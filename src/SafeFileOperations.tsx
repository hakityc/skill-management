import { useEffect, useMemo, useState } from "react";

import type { SkillGateway } from "./App";
import { readableError } from "./errors";
import type {
  FileConflictPolicy,
  FileOperationItemResult,
  FileOperationKind,
  FileOperationPlan,
  FileOperationRecord,
  SkillInstance,
  WorkspaceSnapshot,
} from "./models";
import "./safe-file-operations.css";

type OperationGateway = Pick<
  SkillGateway,
  | "chooseZipFile"
  | "planFileOperations"
  | "previewZipImport"
  | "executeFileOperationPlan"
  | "cancelFileOperationPlan"
  | "fileOperationHistory"
  | "latestUndoableFileOperation"
  | "undoFileOperationBatch"
>;

interface SafeFileOperationsProps {
  gateway: OperationGateway;
  snapshot: WorkspaceSnapshot;
  selectedInstances: SkillInstance[];
  initialMode: Exclude<FileOperationKind, "merge">;
  onClose(): void;
  onSnapshotChange(snapshot: WorkspaceSnapshot): void;
  onCompleted?(): void;
}

export function SafeFileOperations({
  gateway,
  snapshot,
  selectedInstances,
  initialMode,
  onClose,
  onSnapshotChange,
  onCompleted,
}: SafeFileOperationsProps) {
  const [mode, setMode] = useState<Exclude<FileOperationKind, "merge">>(initialMode);
  const [targetRootId, setTargetRootId] = useState<number | null>(
    snapshot.roots.find((root) => root.status === "ready")?.id ?? null,
  );
  const [conflictPolicy, setConflictPolicy] =
    useState<FileConflictPolicy>("skip");
  const [zipPath, setZipPath] = useState<string | null>(null);
  const [relativePath, setRelativePath] = useState("imported-skill");
  const [plan, setPlan] = useState<FileOperationPlan | null>(null);
  const [results, setResults] = useState<FileOperationItemResult[] | null>(null);
  const [history, setHistory] = useState<FileOperationRecord[]>([]);
  const [latestUndoable, setLatestUndoable] = useState<FileOperationRecord | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const availableTargetRoots = useMemo(() => {
    if (mode === "copy" || mode === "move") {
      const selectedRootIds = new Set(selectedInstances.map((instance) => instance.rootId));
      return snapshot.roots.filter(
        (root) => root.status === "ready" && !selectedRootIds.has(root.id),
      );
    }
    return snapshot.roots.filter((root) => root.status === "ready");
  }, [mode, selectedInstances, snapshot.roots]);

  useEffect(() => {
    if (mode !== "copy" && mode !== "move") return;
    if (availableTargetRoots.some((root) => root.id === targetRootId)) return;
    setTargetRootId(availableTargetRoots[0]?.id ?? null);
  }, [availableTargetRoots, mode, targetRootId]);

  useEffect(() => {
    void refreshHistory();
    // 只在打开面板时读取一次，后续由完成与撤销动作刷新。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gateway]);

  function changeMode(nextMode: Exclude<FileOperationKind, "merge">) {
    invalidatePlan();
    setMode(nextMode);
    setResults(null);
    setError(null);
  }

  function invalidatePlan() {
    const obsoletePlan = plan;
    setPlan(null);
    if (obsoletePlan) {
      void gateway.cancelFileOperationPlan(obsoletePlan.id).catch((reason: unknown) => {
        setError(readableError(reason));
      });
    }
  }

  function closeOperations() {
    invalidatePlan();
    onClose();
  }

  async function refreshHistory() {
    try {
      const [records, latest] = await Promise.all([
        gateway.fileOperationHistory(),
        gateway.latestUndoableFileOperation(),
      ]);
      setHistory(records);
      setLatestUndoable(latest);
    } catch (reason) {
      setError(readableError(reason));
    }
  }

  async function chooseZip() {
    setError(null);
    try {
      const selected = await gateway.chooseZipFile();
      if (selected) {
        invalidatePlan();
        setZipPath(selected);
        const inferredName = selected
          .split(/[\\/]/)
          .at(-1)
          ?.replace(/\.zip$/i, "");
        if (inferredName) setRelativePath(inferredName);
      }
    } catch (reason) {
      setError(readableError(reason));
    }
  }

  async function previewImpact() {
    if (mode === "import" && (!zipPath || targetRootId === null || !relativePath.trim())) {
      setError("请选择 ZIP、目标根目录，并填写目标相对路径。");
      return;
    }
    if (mode !== "import" && selectedInstances.length === 0) {
      setError("请先选择至少一个 Skill 实例。");
      return;
    }
    if ((mode === "copy" || mode === "move") && targetRootId === null) {
      setError("没有可用的目标 Skill 根目录，请先添加另一个根目录。");
      return;
    }
    setBusy(true);
    setError(null);
    setPlan(null);
    setResults(null);
    try {
      const nextPlan =
        mode === "import"
          ? await gateway.previewZipImport({
              zipPath: zipPath!,
              targetRootId: targetRootId!,
              relativePath: relativePath.trim(),
              conflictPolicy,
            })
          : await gateway.planFileOperations({
              instanceIds: selectedInstances.map((instance) => instance.id),
              kind: mode,
              targetRootId: mode === "trash" ? null : targetRootId,
              conflictPolicy,
            });
      setPlan(nextPlan);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function executePlan() {
    if (!plan) return;
    setBusy(true);
    setError(null);
    try {
      const outcome = await gateway.executeFileOperationPlan(plan.id);
      setResults(outcome.results);
      setPlan(null);
      onSnapshotChange(outcome.snapshot);
      onCompleted?.();
      await refreshHistory();
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function undoLatest() {
    if (!latestUndoable) return;
    setBusy(true);
    setError(null);
    try {
      const nextSnapshot = await gateway.undoFileOperationBatch(latestUndoable.batchId);
      onSnapshotChange(nextSnapshot);
      onCompleted?.();
      await refreshHistory();
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(false);
    }
  }

  const impact = plan?.items.reduce(
    (total, item) => ({
      files: total.files + item.fileCount,
      bytes: total.bytes + item.totalSize,
    }),
    { files: 0, bytes: 0 },
  );
  const resultSummary = results
    ? {
        success: results.filter((item) => item.status === "success").length,
        failed: results.filter((item) => item.status === "failed").length,
        skipped: results.filter((item) => item.status === "skipped").length,
      }
    : null;

  return (
    <div className="editor-backdrop" role="presentation">
      <section className="safe-file-operations" role="dialog" aria-modal="true" aria-labelledby="safe-operation-title">
        <header className="editor-header">
          <div>
            <p className="eyebrow">先预览，再写入</p>
            <h2 id="safe-operation-title">安全文件操作</h2>
          </div>
          <button className="secondary-button compact" onClick={closeOperations} disabled={busy}>
            关闭
          </button>
        </header>

        <div className="safe-operation-layout">
          <div className="safe-operation-workflow">
            <nav className="operation-tabs" aria-label="文件操作类型">
              {(["import", "copy", "move", "trash"] as Exclude<FileOperationKind, "merge">[]).map(
                (item) => (
                  <button
                    key={item}
                    className={mode === item ? "active" : ""}
                    aria-pressed={mode === item}
                    onClick={() => changeMode(item)}
                    disabled={item !== "import" && selectedInstances.length === 0}
                  >
                    {operationName(item)}
                  </button>
                ),
              )}
            </nav>

            {mode === "import" ? (
              <section className="operation-setup">
                <div className="zip-picker">
                  <button className="secondary-button" onClick={chooseZip} disabled={busy}>
                    选择 ZIP 文件
                  </button>
                  <code>{zipPath ?? "尚未选择 ZIP"}</code>
                </div>
                <OperationSelect
                  label="目标 Skill 根目录"
                  value={targetRootId?.toString() ?? ""}
                  onChange={(value) => {
                    setTargetRootId(value ? Number(value) : null);
                    invalidatePlan();
                  }}
                  options={availableTargetRoots.map((root) => [String(root.id), root.path])}
                />
                <label className="operation-field">
                  <span>目标相对路径</span>
                  <input
                    aria-label="目标相对路径"
                    value={relativePath}
                    onChange={(event) => {
                      setRelativePath(event.target.value);
                      invalidatePlan();
                    }}
                  />
                </label>
                <ConflictSelect
                  value={conflictPolicy}
                  onChange={(value) => {
                    setConflictPolicy(value);
                    invalidatePlan();
                  }}
                />
                <p className="operation-safety-note">
                  ZIP 会先解包到临时目录并检查路径穿越、绝对路径和越界符号链接。
                </p>
              </section>
            ) : (
              <section className="operation-setup">
                <p className="selection-summary">
                  已选择 <strong>{selectedInstances.length}</strong> 个 Skill 实例
                </p>
                {mode === "copy" || mode === "move" ? (
                  <OperationSelect
                    label="目标 Skill 根目录"
                    value={targetRootId?.toString() ?? ""}
                    onChange={(value) => {
                      setTargetRootId(value ? Number(value) : null);
                      invalidatePlan();
                    }}
                    options={availableTargetRoots.map((root) => [String(root.id), root.path])}
                  />
                ) : null}
                {mode !== "trash" ? (
                  <ConflictSelect
                    value={conflictPolicy}
                    onChange={(value) => {
                      setConflictPolicy(value);
                      invalidatePlan();
                    }}
                  />
                ) : (
                  <p className="trash-note">
                    删除会把每个 Skill 移入 macOS 系统废纸篓。应用内不能撤销，可在访达的废纸篓中恢复。
                  </p>
                )}
              </section>
            )}

            {error ? <p className="operation-error">{error}</p> : null}
            <p className="operation-boundary-note">
              执行时请勿用其他工具并发重排相关根目录。本版会阻止静态软链接越界，但不抵御同权限恶意进程在操作期间替换目录。
            </p>

            {plan ? (
              <section className="operation-plan" aria-label="文件操作影响预览">
                <div className="operation-section-heading">
                  <div>
                    <p className="eyebrow">确认计划</p>
                    <h3>{operationName(plan.kind)}影响</h3>
                  </div>
                  <strong>
                    共 {plan.items.length} 项 · {impact?.files ?? 0} 个文件 · {formatBytes(impact?.bytes ?? 0)}
                  </strong>
                </div>
                {plan.kind === "import" ? (
                  <p className="operation-safety-note confirm-note">
                    确认执行前，不会写入 Skill 根目录
                  </p>
                ) : null}
                <ul className="operation-item-list">
                  {plan.items.map((item, index) => (
                    <li key={`${item.source}-${index}`}>
                      <div>
                        <strong>{lastPathPart(item.source)}</strong>
                        <code>{item.source}</code>
                        {item.target ? <code>→ {item.target}</code> : null}
                      </div>
                      <div className="impact-badges">
                        {item.willOverwrite ? <span className="warning">将覆盖目标并创建备份</span> : null}
                        {item.conflict && !item.willOverwrite ? <span>目标冲突，将跳过</span> : null}
                        {item.willRemoveSource ? <span className="danger">将移除来源</span> : null}
                        <span>{item.fileCount} 个文件 · {formatBytes(item.totalSize)}</span>
                      </div>
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}

            {results ? (
              <section className="operation-results" aria-live="polite">
                <div className="operation-section-heading">
                  <div>
                    <p className="eyebrow">执行结果</p>
                    <h3>
                      {resultSummary?.success} 项成功，{resultSummary?.failed} 项失败，{resultSummary?.skipped} 项跳过
                    </h3>
                  </div>
                </div>
                <ul className="operation-result-list">
                  {results.map((item, index) => (
                    <li className={item.status} key={`${item.source}-${index}`}>
                      <span>{resultStatusName(item.status)}</span>
                      <div>
                        <strong>{lastPathPart(item.source)}</strong>
                        <p>{item.message}</p>
                        {item.backupCreated ? <small>已创建目标备份</small> : null}
                      </div>
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}
          </div>

          <aside className="operation-history">
            <div className="operation-section-heading">
              <div>
                <p className="eyebrow">本机审计记录</p>
                <h3>最近操作</h3>
              </div>
              {latestUndoable ? (
                <button className="secondary-button compact" onClick={undoLatest} disabled={busy}>
                  撤销最近操作
                </button>
              ) : null}
            </div>
            {history.length ? (
              <ol>
                {history.slice(0, 8).map((record) => (
                  <li key={record.batchId}>
                    <strong>{operationName(record.kind)} · {record.plan.items.length} 项</strong>
                    <span>{formatTimestamp(record.createdAt)}</span>
                    <small>
                      {record.undone
                        ? "已撤销"
                        : `${record.results.filter((item) => item.status === "success").length} 项成功`}
                    </small>
                  </li>
                ))}
              </ol>
            ) : (
              <p className="history-empty">还没有文件操作记录。</p>
            )}
          </aside>
        </div>

        <footer className="editor-footer">
          <button className="secondary-button" onClick={closeOperations} disabled={busy}>
            完成
          </button>
          {!results ? (
            plan ? (
              <button className={mode === "trash" ? "danger-button" : "primary-button"} onClick={executePlan} disabled={busy}>
                {busy ? "正在执行…" : "确认执行"}
              </button>
            ) : (
              <button className="primary-button" onClick={previewImpact} disabled={busy}>
                {busy ? "正在检查…" : mode === "import" ? "预览导入" : "预览影响"}
              </button>
            )
          ) : null}
        </footer>
      </section>
    </div>
  );
}

function ConflictSelect({
  value,
  onChange,
}: {
  value: FileConflictPolicy;
  onChange(value: FileConflictPolicy): void;
}) {
  return (
    <OperationSelect
      label="冲突处理"
      value={value}
      onChange={(next) => onChange(next as FileConflictPolicy)}
      options={[
        ["skip", "跳过已有目标（推荐）"],
        ["overwrite", "覆盖目标并创建备份"],
      ]}
    />
  );
}

function OperationSelect({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange(value: string): void;
  options: string[][];
}) {
  return (
    <label className="operation-field">
      <span>{label}</span>
      <select aria-label={label} value={value} onChange={(event) => onChange(event.target.value)}>
        {!options.length ? <option value="">没有可用目录</option> : null}
        {options.map(([optionValue, text]) => (
          <option key={optionValue} value={optionValue}>{text}</option>
        ))}
      </select>
    </label>
  );
}

function operationName(kind: FileOperationKind) {
  return { import: "ZIP 导入", copy: "复制", move: "移动", trash: "移入废纸篓", merge: "重复归并" }[kind];
}

function resultStatusName(status: FileOperationItemResult["status"]) {
  return { success: "成功", failed: "失败", skipped: "跳过" }[status];
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Number((bytes / 1024).toFixed(1))} KB`;
  return `${Number((bytes / 1024 / 1024).toFixed(1))} MB`;
}

function lastPathPart(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

function formatTimestamp(timestamp: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(timestamp));
}
