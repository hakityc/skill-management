import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { SkillGateway } from "./App";
import { readableError } from "./errors";
import type {
  DuplicateCheckStatus,
  DuplicateComparison,
  DuplicateDecisionKind,
  DuplicateDecisionRecord,
  DuplicateFileDifference,
  DuplicateGroup,
  DuplicateHitRule,
  DuplicateReview,
  DuplicateReviewInstance,
  FileOperationItemResult,
  FileOperationPlan,
  FileOperationRecord,
  SkillClient,
  WorkspaceSnapshot,
} from "./models";
import "./duplicate-governance.css";

interface DuplicateGovernanceProps {
  gateway: SkillGateway;
  onBack(): void;
  onSnapshotChange(snapshot: WorkspaceSnapshot): void;
}

const EMPTY_REVIEW: DuplicateReview = { groups: [], suppressedCount: 0 };

export function DuplicateGovernance({
  gateway,
  onBack,
  onSnapshotChange,
}: DuplicateGovernanceProps) {
  const [review, setReview] = useState(EMPTY_REVIEW);
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);
  const [selectedComparisonIndex, setSelectedComparisonIndex] = useState(0);
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [filter, setFilter] = useState<DuplicateCheckStatus | "all">("all");
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showDecisions, setShowDecisions] = useState(false);
  const [decisions, setDecisions] = useState<DuplicateDecisionRecord[]>([]);
  const [mergeMasterId, setMergeMasterId] = useState<string | null>(null);
  const [mergeTargetIds, setMergeTargetIds] = useState<string[]>([]);
  const [mergePlan, setMergePlan] = useState<FileOperationPlan | null>(null);
  const [mergeResults, setMergeResults] = useState<FileOperationItemResult[] | null>(null);
  const [mergeHistory, setMergeHistory] = useState<FileOperationRecord[]>([]);
  const mergePlanRef = useRef<FileOperationPlan | null>(null);
  const mergeRequestRevision = useRef(0);

  function storeMergePlan(next: FileOperationPlan | null) {
    mergePlanRef.current = next;
    setMergePlan(next);
  }

  const loadReview = useCallback(async () => {
    setError(null);
    try {
      const nextReview = await gateway.reviewDuplicateGroups();
      setReview(nextReview);
      setSelectedGroupId((current) =>
        current && nextReview.groups.some((group) => group.id === current)
          ? current
          : (nextReview.groups[0]?.id ?? null),
      );
      onSnapshotChange(await gateway.loadSnapshot());
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setLoading(false);
    }
  }, [gateway, onSnapshotChange]);

  useEffect(() => {
    void loadReview();
    void gateway.fileOperationHistory().then((records) =>
      setMergeHistory(records.filter((record) => record.kind === "merge")),
    );
  }, [loadReview]);

  useEffect(
    () => () => {
      mergeRequestRevision.current += 1;
      const obsolete = mergePlanRef.current;
      if (obsolete) void gateway.cancelFileOperationPlan(obsolete.id).catch(() => {});
    },
    [gateway],
  );

  const filteredGroups = useMemo(() => {
    const search = query.trim().toLocaleLowerCase("zh-CN");
    return review.groups.filter(
      (group) =>
        (filter === "all" || group.status === filter) &&
        (!search ||
          group.name.toLocaleLowerCase("zh-CN").includes(search) ||
          group.instances.some(
            (instance) =>
              instance.name.toLocaleLowerCase("zh-CN").includes(search) ||
              instance.path.toLocaleLowerCase("zh-CN").includes(search),
          )),
    );
  }, [filter, query, review.groups]);
  const selectedGroup =
    filteredGroups.find((group) => group.id === selectedGroupId) ?? filteredGroups[0] ?? null;
  const comparison = selectedGroup?.comparisons[selectedComparisonIndex] ?? null;
  const selectedFile =
    comparison?.files.find((file) => file.relativePath === selectedFilePath) ??
    comparison?.files.find((file) => file.status !== "identical") ??
    comparison?.files[0] ??
    null;

  useEffect(() => {
    mergeRequestRevision.current += 1;
    const obsolete = mergePlanRef.current;
    if (obsolete) void gateway.cancelFileOperationPlan(obsolete.id).catch(() => {});
    setSelectedComparisonIndex(0);
    setSelectedFilePath(null);
    const instances = selectedGroup?.instances ?? [];
    setMergeMasterId(instances[0]?.id ?? null);
    setMergeTargetIds(instances.slice(1).map((instance) => instance.id));
    storeMergePlan(null);
    setMergeResults(null);
  }, [selectedGroup?.id]);

  function discardMergePlan() {
    mergeRequestRevision.current += 1;
    const obsolete = mergePlanRef.current;
    storeMergePlan(null);
    setMergeResults(null);
    if (obsolete) void gateway.cancelFileOperationPlan(obsolete.id).catch(() => {});
  }

  function chooseMergeMaster(instanceId: string) {
    discardMergePlan();
    setMergeMasterId(instanceId);
    setMergeTargetIds(selectedGroup?.instances.filter((item) => item.id !== instanceId).map((item) => item.id) ?? []);
  }

  function toggleMergeTarget(instanceId: string) {
    discardMergePlan();
    setMergeTargetIds((current) =>
      current.includes(instanceId)
        ? current.filter((id) => id !== instanceId)
        : [...current, instanceId],
    );
  }

  async function previewMerge() {
    if (!mergeMasterId || mergeTargetIds.length === 0) return;
    setBusy(true);
    setError(null);
    const requestRevision = mergeRequestRevision.current + 1;
    mergeRequestRevision.current = requestRevision;
    try {
      if (mergePlan && !mergeResults) {
        storeMergePlan(null);
        await gateway.cancelFileOperationPlan(mergePlan.id);
      }
      const nextPlan = await gateway.planDuplicateMerge(mergeMasterId, mergeTargetIds);
      if (mergeRequestRevision.current !== requestRevision) {
        await gateway.cancelFileOperationPlan(nextPlan.id).catch(() => {});
        return;
      }
      storeMergePlan(nextPlan);
      setMergeResults(null);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function executeMerge() {
    if (!mergePlan) return;
    setBusy(true);
    setError(null);
    try {
      const outcome = await gateway.executeFileOperationPlan(mergePlan.id);
      setMergeResults(outcome.results);
      onSnapshotChange(outcome.snapshot);
      setMergeHistory(
        (await gateway.fileOperationHistory()).filter((record) => record.kind === "merge"),
      );
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function undoMerge(batchId: number) {
    setBusy(true);
    setError(null);
    try {
      onSnapshotChange(await gateway.undoFileOperationBatch(batchId));
      setMergeHistory(
        (await gateway.fileOperationHistory()).filter((record) => record.kind === "merge"),
      );
      storeMergePlan(null);
      setMergeResults(null);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function decide(kind: DuplicateDecisionKind) {
    if (!selectedGroup) return;
    setBusy(true);
    setError(null);
    try {
      await gateway.saveDuplicateDecision(
        selectedGroup.instances.map((instance) => instance.id),
        kind,
      );
      await loadReview();
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function openDecisionSettings() {
    setBusy(true);
    setError(null);
    try {
      setDecisions(await gateway.duplicateDecisions());
      setShowDecisions(true);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(false);
    }
  }

  async function restoreDecision(decisionId: number) {
    setBusy(true);
    setError(null);
    try {
      await gateway.restoreDuplicateDecision(decisionId);
      setDecisions(await gateway.duplicateDecisions());
      await loadReview();
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusy(false);
    }
  }

  const counts = {
    exact: review.groups.filter((group) => group.status === "exact").length,
    suspected: review.groups.filter((group) => group.status === "suspected").length,
    nameConflict: review.groups.filter((group) => group.status === "nameConflict").length,
  };

  return (
    <main className="duplicate-page">
      <header className="duplicate-header">
        <div>
          <button className="duplicate-back" onClick={onBack}>← 返回 Skill 列表</button>
          <p>Skill 管理器 / 重复检查</p>
          <h1>把相似，变成确定。</h1>
        </div>
        <div className="duplicate-header-actions">
          <button className="secondary-button" onClick={openDecisionSettings} disabled={busy}>
            已忽略结果 {review.suppressedCount}
          </button>
          <button className="primary-button" onClick={loadReview} disabled={loading || busy}>
            {loading ? "正在检查…" : "重新检查"}
          </button>
        </div>
      </header>

      <section className="duplicate-metrics" aria-label="重复检查概览">
        <Metric label="待检查" count={review.groups.length} active={filter === "all"} disabled={busy} onClick={() => setFilter("all")} />
        <Metric label="完全重复" count={counts.exact} tone="exact" active={filter === "exact"} disabled={busy} onClick={() => setFilter("exact")} />
        <Metric label="疑似重复" count={counts.suspected} tone="suspected" active={filter === "suspected"} disabled={busy} onClick={() => setFilter("suspected")} />
        <Metric label="同名冲突" count={counts.nameConflict} tone="conflict" active={filter === "nameConflict"} disabled={busy} onClick={() => setFilter("nameConflict")} />
      </section>

      {error ? <div className="duplicate-error" role="alert">重复检查失败：{error}</div> : null}
      <div className="duplicate-workspace">
        <aside className="duplicate-queue">
          <div className="duplicate-queue-heading">
            <div><p>审阅队列</p><h2>{filteredGroups.length} 组待处理</h2></div>
            <span>风险优先</span>
          </div>
          <label className="duplicate-search">
            <span aria-hidden="true">⌕</span>
            <input
              type="search"
              aria-label="搜索 Skill 组"
              placeholder="搜索名称、实例或路径"
              value={query}
              disabled={busy}
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
          <div className="duplicate-queue-list">
            {filteredGroups.map((group, index) => (
              <button
                className={group.id === selectedGroup?.id ? "duplicate-queue-item active" : "duplicate-queue-item"}
                key={group.id}
                disabled={busy}
                onClick={() => setSelectedGroupId(group.id)}
              >
                <span className="queue-number">{String(index + 1).padStart(2, "0")}</span>
                <span className="queue-copy">
                  <b>{group.name}</b>
                  <small>{group.instances.length} 个实例 · {group.instances.map((instance) => clientName(instance.client)).join(" · ")}</small>
                </span>
                <span className="queue-score"><b>{formatPercent(group.similarity)}</b><small>相似度</small></span>
                <StatusBadge status={group.status} />
              </button>
            ))}
            {!loading && filteredGroups.length === 0 ? (
              <div className="duplicate-empty"><strong>没有待检查的 Skill 组</strong><span>可以查看已忽略结果，或重新检查 Skill 根目录。</span></div>
            ) : null}
          </div>
          <footer><span><i /> 本地检查规则 v1</span><button onClick={openDecisionSettings}>查看忽略项</button></footer>
        </aside>

        {selectedGroup && comparison ? (
          <ComparisonStage
            group={selectedGroup}
            comparison={comparison}
            selectedComparisonIndex={selectedComparisonIndex}
            selectedFile={selectedFile}
            busy={busy}
            onSelectComparison={(index) => {
              setSelectedComparisonIndex(index);
              setSelectedFilePath(null);
            }}
            onSelectFile={setSelectedFilePath}
            onNotDuplicate={() => decide("notDuplicate")}
            onIgnore={() => decide("ignored")}
            mergeMasterId={mergeMasterId}
            mergeTargetIds={mergeTargetIds}
            mergePlan={mergePlan}
            mergeResults={mergeResults}
            mergeHistory={mergeHistory}
            onChooseMergeMaster={chooseMergeMaster}
            onToggleMergeTarget={toggleMergeTarget}
            onPreviewMerge={previewMerge}
            onExecuteMerge={executeMerge}
            onUndoMerge={undoMerge}
          />
        ) : (
          <section className="duplicate-stage duplicate-empty"><strong>没有可比较的 Skill 组</strong></section>
        )}
      </div>

      {showDecisions ? (
        <DecisionSettings
          decisions={decisions}
          busy={busy}
          onClose={() => setShowDecisions(false)}
          onRestore={restoreDecision}
        />
      ) : null}
    </main>
  );
}

function ComparisonStage({
  group,
  comparison,
  selectedComparisonIndex,
  selectedFile,
  busy,
  onSelectComparison,
  onSelectFile,
  onNotDuplicate,
  onIgnore,
  mergeMasterId,
  mergeTargetIds,
  mergePlan,
  mergeResults,
  mergeHistory,
  onChooseMergeMaster,
  onToggleMergeTarget,
  onPreviewMerge,
  onExecuteMerge,
  onUndoMerge,
}: {
  group: DuplicateGroup;
  comparison: DuplicateComparison;
  selectedComparisonIndex: number;
  selectedFile: DuplicateFileDifference | null;
  busy: boolean;
  onSelectComparison(index: number): void;
  onSelectFile(path: string): void;
  onNotDuplicate(): void;
  onIgnore(): void;
  mergeMasterId: string | null;
  mergeTargetIds: string[];
  mergePlan: FileOperationPlan | null;
  mergeResults: FileOperationItemResult[] | null;
  mergeHistory: FileOperationRecord[];
  onChooseMergeMaster(id: string): void;
  onToggleMergeTarget(id: string): void;
  onPreviewMerge(): void;
  onExecuteMerge(): void;
  onUndoMerge(batchId: number): void;
}) {
  const left = group.instances.find((instance) => instance.id === comparison.leftInstanceId)!;
  const right = group.instances.find((instance) => instance.id === comparison.rightInstanceId)!;
  return (
    <section className="duplicate-stage">
      <div className="comparison-heading">
        <div>
          <span>重复检查 / {statusName(comparison.status)}</span>
          <h2>{group.name}</h2>
          <p>{statusDescription(comparison.status)}</p>
        </div>
        <div className="comparison-actions">
          <button className="secondary-button" onClick={onIgnore} disabled={busy}>暂时忽略</button>
          <button className="secondary-button" onClick={onNotDuplicate} disabled={busy}>不是重复</button>
        </div>
      </div>

      <div className="comparison-rules">
        <span>命中规则</span>
        {comparison.hitRules.map((rule) => <b key={rule}>{ruleName(rule)}</b>)}
        <span>参与指纹：{comparison.files.map((file) => file.relativePath).join("、")}</span>
      </div>

      {group.comparisons.length > 1 ? (
        <label className="comparison-picker">
          <span>比较实例组合</span>
          <select
            aria-label="比较实例组合"
            value={selectedComparisonIndex}
            onChange={(event) => onSelectComparison(Number(event.target.value))}
          >
            {group.comparisons.map((candidate, index) => (
              <option key={`${candidate.leftInstanceId}-${candidate.rightInstanceId}`} value={index}>
                {comparisonName(group, candidate)}
              </option>
            ))}
          </select>
        </label>
      ) : null}

      <div className="instance-comparison">
        <InstanceCard instance={left} side="A" />
        <div className="comparison-score"><span>内容相似度</span><b>{formatPercent(comparison.similarity)}</b><small>阈值 82%</small></div>
        <InstanceCard instance={right} side="B" />
      </div>

      <section className="duplicate-diff-panel" aria-label="逐文件差异">
        <div className="diff-panel-heading">
          <div><b>逐文件差异</b><span>{comparison.files.filter((file) => file.status !== "identical").length} 处变化</span></div>
          <small>二进制文件只比较指纹和大小</small>
        </div>
        <div className="duplicate-file-tabs">
          {comparison.files.map((file) => (
            <button
              className={file.relativePath === selectedFile?.relativePath ? "active" : ""}
              key={file.relativePath}
              onClick={() => onSelectFile(file.relativePath)}
            >
              <i className={file.status} />
              {file.relativePath}
              <span>{fileStatusName(file.status)}</span>
            </button>
          ))}
        </div>
        {selectedFile ? <FileDifference file={selectedFile} /> : null}
      </section>
      <MergePanel
        group={group}
        masterId={mergeMasterId}
        targetIds={mergeTargetIds}
        plan={mergePlan}
        results={mergeResults}
        history={mergeHistory}
        busy={busy}
        onChooseMaster={onChooseMergeMaster}
        onToggleTarget={onToggleMergeTarget}
        onPreview={onPreviewMerge}
        onExecute={onExecuteMerge}
        onUndo={onUndoMerge}
      />
    </section>
  );
}

function MergePanel({
  group,
  masterId,
  targetIds,
  plan,
  results,
  history,
  busy,
  onChooseMaster,
  onToggleTarget,
  onPreview,
  onExecute,
  onUndo,
}: {
  group: DuplicateGroup;
  masterId: string | null;
  targetIds: string[];
  plan: FileOperationPlan | null;
  results: FileOperationItemResult[] | null;
  history: FileOperationRecord[];
  busy: boolean;
  onChooseMaster(id: string): void;
  onToggleTarget(id: string): void;
  onPreview(): void;
  onExecute(): void;
  onUndo(batchId: number): void;
}) {
  return (
    <section className="merge-panel" aria-label="安全归并">
      <header>
        <div><span>一次性操作</span><h3>安全归并相关 Skill 实例</h3></div>
        <p>选择一个主实例，将其完整目录镜像到目标；不会拼接或生成 SKILL.md，也不会建立永久同步。</p>
      </header>
      <div className="merge-selection">
        <fieldset>
          <legend>1. 选择本次主实例</legend>
          {group.instances.map((instance) => (
            <label key={instance.id}>
              <input
                type="radio"
                name={`merge-master-${group.id}`}
                aria-label={`主实例 ${clientName(instance.client)} · ${instance.name}`}
                checked={instance.id === masterId}
                disabled={busy}
                onChange={() => onChooseMaster(instance.id)}
              />
              <b>{clientName(instance.client)} · {instance.name}</b><small>{instance.path}</small>
            </label>
          ))}
        </fieldset>
        <fieldset>
          <legend>2. 选择归并目标</legend>
          {group.instances.filter((instance) => instance.id !== masterId).map((instance) => (
            <label key={instance.id}>
              <input
                type="checkbox"
                aria-label={`归并目标 ${clientName(instance.client)} · ${instance.name}`}
                checked={targetIds.includes(instance.id)}
                disabled={busy}
                onChange={() => onToggleTarget(instance.id)}
              />
              <b>{clientName(instance.client)} · {instance.name}</b><small>{instance.path}</small>
            </label>
          ))}
        </fieldset>
      </div>
      <button className="primary-button merge-preview-button" onClick={onPreview} disabled={busy || !masterId || targetIds.length === 0}>
        预览安全归并
      </button>

      {plan ? (
        <div className="merge-plan">
          <div className="merge-warning">
            <b>确认后，目标目录将完整采用主实例内容</b>
            <span>目标额外文件只会按下列删除清单处理；每个目标会先独立备份。</span>
          </div>
          {plan.items.map((item, itemIndex) => {
            const changes = item.changes ?? [];
            return (
              <article className="merge-target-plan" key={`${item.instanceId}-${itemIndex}`}>
                <header><div><span>目标 {itemIndex + 1}</span><b>{item.target}</b></div><small>备份后原子替换</small></header>
                <div className="merge-change-counts">
                  <span>新增 {changes.filter((change) => change.status === "onlyLeft").length}</span>
                  <span>覆盖 {changes.filter((change) => change.status === "modified").length}</span>
                  <span className="delete">删除 {changes.filter((change) => change.status === "onlyRight").length}</span>
                </div>
                <div className="merge-change-list">
                  {changes.map((change) => (
                    <details key={change.relativePath}>
                      <summary><b>{change.relativePath}</b><span>{mergeChangeName(change.status)}</span></summary>
                      <FileDifference file={change} />
                    </details>
                  ))}
                  {changes.length === 0 ? <p>目标已经与主实例一致；执行后仍会保持独立目录。</p> : null}
                </div>
              </article>
            );
          })}
          {!results ? (
            <button className="primary-button" onClick={onExecute} disabled={busy}>
              确认归并 {plan.items.length} 个目标
            </button>
          ) : (
            <div className="merge-results">
              {results.map((result, index) => (
                <div className={result.status} key={`${result.instanceId}-${index}`}>
                  <b>{result.status === "success" ? "成功" : result.status === "failed" ? "失败" : "跳过"}</b>
                  <span>{result.message}</span><small>{result.target}</small>
                </div>
              ))}
            </div>
          )}
        </div>
      ) : null}

      {history.length ? (
        <div className="merge-history">
          <h4>归并记录</h4>
          {history.slice(0, 5).map((record) => (
            <div key={record.batchId}>
              <span>#{record.batchId} · {record.results.length || record.plan.items.length} 个目标</span>
              {record.undoable && !record.undone ? (
                <button className="secondary-button compact" onClick={() => onUndo(record.batchId)} disabled={busy}>撤销归并 #{record.batchId}</button>
              ) : <small>已撤销</small>}
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function FileDifference({ file }: { file: DuplicateFileDifference }) {
  const nodeTypeChanged =
    file.leftNodeKind &&
    file.rightNodeKind &&
    file.leftNodeKind !== file.rightNodeKind;
  const nodeTypeNotice = nodeTypeChanged ? (
    <p className="node-type-change" role="note">
      节点类型变化：主实例为{nodeKindName(file.leftNodeKind)}，目标为{nodeKindName(file.rightNodeKind)}。
    </p>
  ) : null;
  if (file.kind === "binary") {
    return (
      <>{nodeTypeNotice}<div className="binary-difference">
        <div><span>实例 A · {nodeKindName(file.leftNodeKind)}</span><b>{formatBytes(file.leftSize)}</b><code>{file.leftFingerprint ?? "仅另一侧存在"}</code></div>
        <div><span>实例 B · {nodeKindName(file.rightNodeKind)}</span><b>{formatBytes(file.rightSize)}</b><code>{file.rightFingerprint ?? "仅另一侧存在"}</code></div>
      </div></>
    );
  }
  if (!file.textDiff?.length) {
    return <>{nodeTypeNotice}<div className="identical-file">文件内容一致，指纹相同。</div></>;
  }
  return (
    <>{nodeTypeNotice}<div className="text-difference">
      {file.textDiffTruncated ? (
        <p className="diff-truncated" role="note">
          差异过长，仅展示双方前 1000 行。
        </p>
      ) : null}
      <div className="diff-column-labels"><span>实例 A</span><span>实例 B</span></div>
      {file.textDiff.map((line, index) => (
        <div className={`text-diff-line ${line.kind}`} key={`${line.leftLineNumber}-${line.rightLineNumber}-${index}`}>
          <span>{line.leftLineNumber ?? ""}</span><code>{line.left ?? ""}</code>
          <span>{line.rightLineNumber ?? ""}</span><code>{line.right ?? ""}</code>
        </div>
      ))}
    </div></>
  );
}

function InstanceCard({ instance, side }: { instance: DuplicateReviewInstance; side: string }) {
  return (
    <article className="duplicate-instance-card">
      <header><span>{side}</span><b>{clientName(instance.client)}</b></header>
      <h3>{instance.name}</h3>
      <p>{instance.description}</p>
      <dl><div><dt>客户端</dt><dd>{clientName(instance.client)}</dd></div><div><dt>实例路径</dt><dd title={instance.path}>{instance.path}</dd></div></dl>
    </article>
  );
}

function DecisionSettings({
  decisions,
  busy,
  onClose,
  onRestore,
}: {
  decisions: DuplicateDecisionRecord[];
  busy: boolean;
  onClose(): void;
  onRestore(id: number): void;
}) {
  return (
    <div className="decision-settings-backdrop">
      <section className="decision-settings" role="dialog" aria-modal="true" aria-labelledby="decision-settings-title">
        <header><div><p>重复检查设置</p><h2 id="decision-settings-title">已忽略结果</h2></div><button className="secondary-button" onClick={onClose}>关闭</button></header>
        <p>恢复后，这些 Skill 实例会重新进入检查队列。</p>
        <ul>
          {decisions.map((decision) => (
            <li key={decision.id}>
              <span><b>{decision.kind === "notDuplicate" ? "不是重复" : "暂时忽略"}</b><small>{decision.instanceIds.join(" ↔ ")}</small></span>
              <button className="secondary-button compact" onClick={() => onRestore(decision.id)} disabled={busy}>恢复检查</button>
            </li>
          ))}
        </ul>
        {decisions.length === 0 ? <div className="duplicate-empty">没有已忽略的结果。</div> : null}
      </section>
    </div>
  );
}

function Metric({ label, count, tone = "all", active, disabled, onClick }: { label: string; count: number; tone?: string; active: boolean; disabled: boolean; onClick(): void }) {
  return <button className={`duplicate-metric ${tone}${active ? " active" : ""}`} onClick={onClick} disabled={disabled}><span>{label}</span><b>{count}</b><small>{metricHint(tone)}</small></button>;
}

function StatusBadge({ status }: { status: DuplicateCheckStatus }) {
  return <span className={`duplicate-status ${status}`}>{statusName(status)}</span>;
}

function statusName(status: DuplicateCheckStatus) {
  return { none: "未发现相关实例", exact: "完全重复", suspected: "疑似重复", nameConflict: "同名冲突" }[status];
}

function statusDescription(status: DuplicateCheckStatus) {
  return {
    none: "没有需要审阅的相关实例。",
    exact: "目录有效内容完全一致，可以确认是否需要整理。",
    suspected: "名称或内容高度相似，建议人工核对差异。",
    nameConflict: "名称一致但内容差异明显，请保持谨慎。",
  }[status];
}

function ruleName(rule: DuplicateHitRule) {
  return { exactContent: "有效内容指纹一致", normalizedName: "规范化名称匹配", contentSimilarity: "内容相似度 ≥ 82%" }[rule];
}

function fileStatusName(status: DuplicateFileDifference["status"]) {
  return { identical: "一致", modified: "已修改", onlyLeft: "仅 A 侧", onlyRight: "仅 B 侧（新增）" }[status];
}

function mergeChangeName(status: DuplicateFileDifference["status"]) {
  return { identical: "不变", modified: "覆盖", onlyLeft: "新增", onlyRight: "删除" }[status];
}

function nodeKindName(kind: DuplicateFileDifference["leftNodeKind"]) {
  return kind === "symbolicLink" ? "符号链接" : kind === "file" ? "普通文件" : "不存在";
}

function comparisonName(group: DuplicateGroup, comparison: DuplicateComparison) {
  const left = group.instances.find((instance) => instance.id === comparison.leftInstanceId);
  const right = group.instances.find((instance) => instance.id === comparison.rightInstanceId);
  return `${clientName(left?.client ?? "other")} · ${left?.name ?? "未知实例"} ↔ ${clientName(right?.client ?? "other")} · ${right?.name ?? "未知实例"}`;
}

function metricHint(tone: string) {
  return { all: "组相关实例", exact: "内容一致", suspected: "建议人工确认", conflict: "内容差异明显" }[tone];
}

function clientName(client: SkillClient) {
  return { claude: "Claude", codex: "Codex", gemini: "Gemini", openCode: "OpenCode", hermes: "Hermes", other: "自定义" }[client];
}

function formatPercent(value: number) {
  return `${new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 }).format(value * 100)}%`;
}

function formatBytes(value: number | null) {
  if (value === null) return "—";
  return value < 1024 ? `${value} B` : `${(value / 1024).toFixed(1)} KB`;
}
