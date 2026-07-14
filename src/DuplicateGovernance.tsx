import { useCallback, useEffect, useMemo, useState } from "react";

import type { SkillGateway } from "./App";
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
  }, [loadReview]);

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
    setSelectedComparisonIndex(0);
    setSelectedFilePath(null);
  }, [selectedGroup?.id]);

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
        <Metric label="待检查" count={review.groups.length} active={filter === "all"} onClick={() => setFilter("all")} />
        <Metric label="完全重复" count={counts.exact} tone="exact" active={filter === "exact"} onClick={() => setFilter("exact")} />
        <Metric label="疑似重复" count={counts.suspected} tone="suspected" active={filter === "suspected"} onClick={() => setFilter("suspected")} />
        <Metric label="同名冲突" count={counts.nameConflict} tone="conflict" active={filter === "nameConflict"} onClick={() => setFilter("nameConflict")} />
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
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>
          <div className="duplicate-queue-list">
            {filteredGroups.map((group, index) => (
              <button
                className={group.id === selectedGroup?.id ? "duplicate-queue-item active" : "duplicate-queue-item"}
                key={group.id}
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
    </section>
  );
}

function FileDifference({ file }: { file: DuplicateFileDifference }) {
  if (file.kind === "binary") {
    return (
      <div className="binary-difference">
        <div><span>实例 A</span><b>{formatBytes(file.leftSize)}</b><code>{file.leftFingerprint ?? "仅另一侧存在"}</code></div>
        <div><span>实例 B</span><b>{formatBytes(file.rightSize)}</b><code>{file.rightFingerprint ?? "仅另一侧存在"}</code></div>
      </div>
    );
  }
  if (!file.textDiff?.length) {
    return <div className="identical-file">文件内容一致，指纹相同。</div>;
  }
  return (
    <div className="text-difference">
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
    </div>
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

function Metric({ label, count, tone = "all", active, onClick }: { label: string; count: number; tone?: string; active: boolean; onClick(): void }) {
  return <button className={`duplicate-metric ${tone}${active ? " active" : ""}`} onClick={onClick}><span>{label}</span><b>{count}</b><small>{metricHint(tone)}</small></button>;
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

function readableError(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason);
}
