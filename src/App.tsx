import { useEffect, useState } from "react";

import type {
  DuplicateDecisionKind,
  DuplicateDecisionRecord,
  DuplicateCheckStatus,
  DuplicateReview,
  SkillClient,
  SkillChangeOutcome,
  SkillChangePlan,
  SkillChangeRecord,
  SkillDetail,
  SkillDraft,
  SkillDraftValidation,
  SkillFilters,
  SkillFilePreview,
  SkillInstance,
  SkillQuery,
  SkillRoot,
  SkillSearchResult,
  SkillSort,
  SkillWorkspaceViewPreferences,
  WorkspaceSnapshot,
} from "./models";
import { DuplicateGovernance } from "./DuplicateGovernance";
import "./styles.css";

export interface SkillGateway {
  loadSnapshot(): Promise<WorkspaceSnapshot>;
  chooseAndAuthorizeRoot(): Promise<WorkspaceSnapshot | null>;
  rescanRoot(rootId: number): Promise<WorkspaceSnapshot>;
  removeRoot(rootId: number): Promise<WorkspaceSnapshot>;
  searchSkills(query: SkillQuery): Promise<SkillSearchResult>;
  loadViewPreferences(): Promise<SkillWorkspaceViewPreferences>;
  saveViewPreferences(preferences: SkillWorkspaceViewPreferences): Promise<void>;
  skillDetail(instanceId: string): Promise<SkillDetail>;
  readSkillFile(instanceId: string, relativePath: string): Promise<SkillFilePreview>;
  validateSkillDraft(draft: SkillDraft): Promise<SkillDraftValidation>;
  planSkillChange(draft: SkillDraft): Promise<SkillChangePlan>;
  executeSkillChange(planId: number): Promise<SkillChangeOutcome>;
  undoSkillChange(operationId: number): Promise<SkillChangeOutcome>;
  latestUndoableSkillChange(): Promise<SkillChangeRecord | null>;
  reviewDuplicateGroups(): Promise<DuplicateReview>;
  saveDuplicateDecision(instanceIds: string[], kind: DuplicateDecisionKind): Promise<void>;
  duplicateDecisions(): Promise<DuplicateDecisionRecord[]>;
  restoreDuplicateDecision(decisionId: number): Promise<void>;
}

interface SkillManagerAppProps {
  gateway: SkillGateway;
}

const EMPTY_SNAPSHOT: WorkspaceSnapshot = {
  authorizedRoot: null,
  roots: [],
  instances: [],
};

export function SkillManagerApp({ gateway }: SkillManagerAppProps) {
  const [snapshot, setSnapshot] = useState(EMPTY_SNAPSHOT);
  const [loading, setLoading] = useState(true);
  const [selecting, setSelecting] = useState(false);
  const [busyRootId, setBusyRootId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<"library" | "roots" | "duplicates">("library");

  useEffect(() => {
    let isEffectActive = true;
    gateway
      .loadSnapshot()
      .then((nextSnapshot) => {
        if (isEffectActive) setSnapshot(nextSnapshot);
      })
      .catch((reason: unknown) => {
        if (isEffectActive) setError(readableError(reason));
      })
      .finally(() => {
        if (isEffectActive) setLoading(false);
      });
    return () => {
      isEffectActive = false;
    };
  }, [gateway]);

  async function chooseRoot() {
    setSelecting(true);
    setError(null);
    try {
      const nextSnapshot = await gateway.chooseAndAuthorizeRoot();
      if (nextSnapshot) {
        setSnapshot(nextSnapshot);
        setView("library");
      }
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSelecting(false);
    }
  }

  async function updateRoot(rootId: number, action: "rescan" | "remove") {
    setBusyRootId(rootId);
    setError(null);
    try {
      const nextSnapshot =
        action === "rescan"
          ? await gateway.rescanRoot(rootId)
          : await gateway.removeRoot(rootId);
      setSnapshot(nextSnapshot);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setBusyRootId(null);
    }
  }

  return (
    <div className="app-shell">
      <AppHeader onChooseRoot={chooseRoot} disabled={selecting} />
      {error ? <ErrorBanner message={error} /> : null}
      {loading ? (
        <main className="loading-state" aria-busy="true">
          <span className="loading-dot" />
          正在读取本地索引…
        </main>
      ) : snapshot.roots.length > 0 || snapshot.authorizedRoot ? (
        view === "roots" ? (
          <RootManagement
            snapshot={snapshot}
            busyRootId={busyRootId}
            onBack={() => setView("library")}
            onChooseRoot={chooseRoot}
            onRescan={(rootId) => updateRoot(rootId, "rescan")}
            onRemove={(rootId) => updateRoot(rootId, "remove")}
          />
        ) : view === "duplicates" ? (
          <DuplicateGovernance
            gateway={gateway}
            onBack={() => setView("library")}
            onSnapshotChange={setSnapshot}
          />
        ) : (
          <SkillLibrary
            gateway={gateway}
            snapshot={snapshot}
            onManageRoots={() => setView("roots")}
            onReviewDuplicates={() => setView("duplicates")}
            onSnapshotChange={setSnapshot}
          />
        )
      ) : (
        <EmptyState onChooseRoot={chooseRoot} disabled={selecting} />
      )}
    </div>
  );
}

function AppHeader({
  onChooseRoot,
  disabled,
}: {
  onChooseRoot(): void;
  disabled: boolean;
}) {
  return (
    <header className="app-header">
      <div className="brand">
        <span className="brand-mark" aria-hidden="true">
          SM<i />
        </span>
        <span>
          <strong>Skill 管理器</strong>
          <small>本地档案</small>
        </span>
      </div>
      <button className="secondary-button" onClick={onChooseRoot} disabled={disabled}>
        {disabled ? "正在读取…" : "选择其他根目录"}
      </button>
    </header>
  );
}

function EmptyState({
  onChooseRoot,
  disabled,
}: {
  onChooseRoot(): void;
  disabled: boolean;
}) {
  return (
    <main className="empty-page">
      <section className="empty-card">
        <span className="folder-illustration" aria-hidden="true">
          <i />
          <b>SKILL.md</b>
        </span>
        <p className="eyebrow">开始建立本地档案</p>
        <h1>管理你的本地 Skill</h1>
        <p className="empty-copy">
          选择一个你信任的目录。应用只会扫描这个目录，并把包含 SKILL.md
          的文件夹整理成清晰列表。
        </p>
        <button className="primary-button" onClick={onChooseRoot} disabled={disabled}>
          {disabled ? "正在扫描…" : "选择 Skill 根目录"}
        </button>
        <p className="privacy-note">
          <span aria-hidden="true">●</span> 所有索引都保存在本机
        </p>
      </section>
    </main>
  );
}

const EMPTY_FILTERS: SkillFilters = {
  clients: [],
  rootIds: [],
  repairStatus: "any",
  duplicateCheckStatuses: [],
};

const DEFAULT_VIEW_PREFERENCES: SkillWorkspaceViewPreferences = {
  filters: EMPTY_FILTERS,
  sort: { field: "name", direction: "asc" },
  density: "compact",
};

function SkillLibrary({
  gateway,
  snapshot,
  onManageRoots,
  onReviewDuplicates,
  onSnapshotChange,
}: {
  gateway: SkillGateway;
  snapshot: WorkspaceSnapshot;
  onManageRoots(): void;
  onReviewDuplicates(): void;
  onSnapshotChange(snapshot: WorkspaceSnapshot): void;
}) {
  const [queryText, setQueryText] = useState("");
  const [preferences, setPreferences] = useState(DEFAULT_VIEW_PREFERENCES);
  const [preferencesReady, setPreferencesReady] = useState(false);
  const [instances, setInstances] = useState(snapshot.instances);
  const [total, setTotal] = useState(snapshot.instances.length);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(
    snapshot.instances[0]?.id ?? null,
  );
  const [detail, setDetail] = useState<SkillDetail | null>(null);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [preview, setPreview] = useState<SkillFilePreview | null>(null);
  const [previewPath, setPreviewPath] = useState<string | null>(null);
  const [previewInstanceId, setPreviewInstanceId] = useState<string | null>(null);
  const [editorDraft, setEditorDraft] = useState<SkillDraft | null>(null);
  const [validation, setValidation] = useState<SkillDraftValidation | null>(null);
  const [changePlan, setChangePlan] = useState<SkillChangePlan | null>(null);
  const [editBusy, setEditBusy] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);
  const [lastOperationId, setLastOperationId] = useState<number | null>(null);
  const repairCount = snapshot.instances.filter(
    (skill) => skill.status === "needsRepair",
  ).length;
  const duplicateCount = snapshot.instances.filter(
    (skill) => skill.duplicateCheckStatus !== "none",
  ).length;
  const rootsById = new Map(snapshot.roots.map((root) => [root.id, root]));

  useEffect(() => {
    let active = true;
    gateway
      .loadViewPreferences()
      .then((saved) => {
        if (active) setPreferences(saved);
      })
      .catch((reason: unknown) => {
        if (active) setSearchError(readableError(reason));
      })
      .finally(() => {
        if (active) setPreferencesReady(true);
      });
    return () => {
      active = false;
    };
  }, [gateway]);

  useEffect(() => {
    if (!preferencesReady) return;
    void gateway.saveViewPreferences(preferences).catch((reason: unknown) => {
      setSearchError(readableError(reason));
    });
  }, [gateway, preferences, preferencesReady]);

  useEffect(() => {
    if (!preferencesReady) return;
    let active = true;
    setSearching(true);
    const timer = window.setTimeout(() => {
      gateway
        .searchSkills({
          text: queryText,
          filters: preferences.filters,
          sort: preferences.sort,
        })
        .then((result) => {
          if (!active) return;
          setInstances(result.instances);
          setTotal(result.total);
          setSearchError(null);
        })
        .catch((reason: unknown) => {
          if (active) setSearchError(readableError(reason));
        })
        .finally(() => {
          if (active) setSearching(false);
        });
    }, 120);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [gateway, preferences.filters, preferences.sort, preferencesReady, queryText, snapshot]);

  useEffect(() => {
    if (selectedInstanceId && snapshot.instances.some((skill) => skill.id === selectedInstanceId)) {
      return;
    }
    setSelectedInstanceId(snapshot.instances[0]?.id ?? null);
  }, [selectedInstanceId, snapshot.instances]);

  useEffect(() => {
    let active = true;
    gateway
      .latestUndoableSkillChange()
      .then((record) => {
        if (active) setLastOperationId(record?.operationId ?? null);
      })
      .catch((reason: unknown) => {
        if (active) setEditError(readableError(reason));
      });
    return () => {
      active = false;
    };
  }, [gateway]);

  useEffect(() => {
    if (!selectedInstanceId) {
      setDetail(null);
      setPreview(null);
      setPreviewPath(null);
      setPreviewInstanceId(null);
      return;
    }
    let active = true;
    setDetail(null);
    setDetailError(null);
    setPreview(null);
    setPreviewPath(null);
    setPreviewInstanceId(null);
    gateway
      .skillDetail(selectedInstanceId)
      .then((nextDetail) => {
        if (active) setDetail(nextDetail);
      })
      .catch((reason: unknown) => {
        if (active) setDetailError(readableError(reason));
      });
    return () => {
      active = false;
    };
  }, [gateway, selectedInstanceId, snapshot]);

  async function previewFile(relativePath: string) {
    if (!selectedInstanceId) return;
    setDetailError(null);
    try {
      const nextPreview = await gateway.readSkillFile(selectedInstanceId, relativePath);
      setPreviewInstanceId(selectedInstanceId);
      setPreviewPath(relativePath);
      setPreview(nextPreview);
    } catch (reason) {
      setDetailError(readableError(reason));
    }
  }

  async function openExistingEditor(fileChange?: SkillDraft["fileChanges"][number]) {
    if (!detail) return;
    setEditBusy(true);
    setEditError(null);
    try {
      const skillFile = await gateway.readSkillFile(detail.instance.id, "SKILL.md");
      if (skillFile.kind !== "text") throw new Error("SKILL.md 不是可编辑的文本文件。");
      setEditorDraft({
        target: { kind: "existing", instanceId: detail.instance.id },
        name: detail.instance.name,
        description: detail.instance.description,
        markdownBody: stripFrontmatter(skillFile.content),
        fileChanges: fileChange ? [fileChange] : [],
      });
      setValidation(
        detail.instance.status === "needsRepair"
          ? {
              valid: false,
              issues: [
                {
                  field: "frontmatter",
                  message: `SKILL.md 元数据需要修复：${detail.instance.error ?? "frontmatter 结构错误"}。保存时会按表单重建元数据。`,
                },
              ],
            }
          : null,
      );
      setChangePlan(null);
    } catch (reason) {
      setEditError(readableError(reason));
    } finally {
      setEditBusy(false);
    }
  }

  function openNewEditor() {
    const root = snapshot.roots[0];
    if (!root) return;
    setEditorDraft({
      target: { kind: "new", rootId: root.id, relativePath: "new-skill" },
      name: "new-skill",
      description: "",
      markdownBody: "# 新 Skill\n\n在这里描述 Skill 的使用方式。\n",
      fileChanges: [],
    });
    setValidation(null);
    setChangePlan(null);
    setEditError(null);
  }

  async function previewChanges() {
    if (!editorDraft) return;
    setEditBusy(true);
    setEditError(null);
    setChangePlan(null);
    try {
      const nextValidation = await gateway.validateSkillDraft(editorDraft);
      setValidation(nextValidation);
      if (nextValidation.valid) {
        setChangePlan(await gateway.planSkillChange(editorDraft));
      }
    } catch (reason) {
      setEditError(readableError(reason));
    } finally {
      setEditBusy(false);
    }
  }

  async function confirmChanges() {
    if (!changePlan) return;
    setEditBusy(true);
    setEditError(null);
    try {
      const outcome = await gateway.executeSkillChange(changePlan.id);
      onSnapshotChange(outcome.snapshot);
      setLastOperationId(outcome.operationId);
      setEditorDraft(null);
      setChangePlan(null);
      setValidation(null);
    } catch (reason) {
      setEditError(readableError(reason));
    } finally {
      setEditBusy(false);
    }
  }

  async function undoLastChange() {
    if (lastOperationId === null) return;
    setEditBusy(true);
    setEditError(null);
    try {
      const outcome = await gateway.undoSkillChange(lastOperationId);
      onSnapshotChange(outcome.snapshot);
      setLastOperationId(null);
    } catch (reason) {
      setEditError(readableError(reason));
    } finally {
      setEditBusy(false);
    }
  }

  function updateFilters(filters: SkillFilters) {
    setPreferences((current) => ({ ...current, filters }));
  }

  function updateSingleFilter<Key extends keyof SkillFilters>(
    key: Key,
    value: SkillFilters[Key],
  ) {
    updateFilters({ ...preferences.filters, [key]: value });
  }

  function updateSort(value: string) {
    const [field, direction] = value.split(":") as [
      SkillSort["field"],
      SkillSort["direction"],
    ];
    setPreferences((current) => ({ ...current, sort: { field, direction } }));
  }

  const hasConditions =
    queryText.length > 0 ||
    preferences.filters.clients.length > 0 ||
    preferences.filters.rootIds.length > 0 ||
    preferences.filters.repairStatus !== "any" ||
    preferences.filters.duplicateCheckStatuses.length > 0;
  const resultStatus = searching
    ? "正在检索…"
    : queryText
      ? `检索“${queryText}” · ${total} 个结果`
      : `${total} 个 Skill 实例`;

  return (
    <main className="library-page">
      <aside className="library-sidebar">
        <p className="eyebrow">资料库</p>
        <button
          className={preferences.filters.repairStatus === "any" ? "nav-item active" : "nav-item"}
          onClick={() => updateSingleFilter("repairStatus", "any")}
        >
          <span>全部 Skill</span>
          <b>{snapshot.instances.length}</b>
        </button>
        <button
          className={
            preferences.filters.repairStatus === "needsRepair" ? "nav-item active" : "nav-item"
          }
          onClick={() => updateSingleFilter("repairStatus", "needsRepair")}
        >
          <span>需要修复</span>
          <b>{repairCount}</b>
        </button>
        <button className="nav-item" aria-label="管理根目录" onClick={onManageRoots}>
          <span>管理根目录</span>
          <b>{snapshot.roots.length}</b>
        </button>
        <button className="nav-item" aria-label="重复检查" onClick={onReviewDuplicates}>
          <span>重复检查</span>
          <b>{duplicateCount}</b>
        </button>
        <div className="root-card">
          <span>当前根目录</span>
          <code>{snapshot.authorizedRoot}</code>
          <small>索引已保存在本机</small>
        </div>
      </aside>
      <section className="library-content">
        <div className="library-title">
          <div>
            <p className="eyebrow">本地资料库</p>
            <h1>全部 Skill</h1>
          </div>
          <div className="library-title-actions">
            <span className="scan-status" aria-live="polite">
              <i /> {resultStatus}
            </span>
            {lastOperationId !== null ? (
              <button className="secondary-button compact" onClick={undoLastChange} disabled={editBusy}>
                撤销最近编辑
              </button>
            ) : null}
            <button className="primary-button compact" onClick={openNewEditor}>
              新建 Skill
            </button>
          </div>
        </div>
        <div className="workspace-toolbar">
          <label className={queryText ? "search-control active" : "search-control"}>
            <span aria-hidden="true">⌕</span>
            <input
              type="search"
              aria-label="搜索 Skill"
              placeholder="搜索名称、描述、正文或路径"
              value={queryText}
              onChange={(event) => setQueryText(event.target.value)}
            />
          </label>
          <FilterSelect
            label="Skill 客户端筛选"
            value={preferences.filters.clients[0] ?? ""}
            onChange={(value) =>
              updateSingleFilter("clients", value ? [value as SkillClient] : [])
            }
            options={[
              ["", "全部客户端"],
              ["claude", "Claude"],
              ["codex", "Codex"],
              ["gemini", "Gemini"],
              ["openCode", "OpenCode"],
              ["hermes", "Hermes"],
              ["other", "自定义"],
            ]}
          />
          <FilterSelect
            label="根目录筛选"
            value={preferences.filters.rootIds[0]?.toString() ?? ""}
            onChange={(value) => updateSingleFilter("rootIds", value ? [Number(value)] : [])}
            options={[
              ["", "全部根目录"],
              ...snapshot.roots.map((root) => [root.id.toString(), shortRoot(root.path)]),
            ]}
          />
          <FilterSelect
            label="状态筛选"
            value={preferences.filters.repairStatus}
            onChange={(value) =>
              updateSingleFilter("repairStatus", value as SkillFilters["repairStatus"])
            }
            options={[
              ["any", "全部状态"],
              ["ready", "正常"],
              ["needsRepair", "需要修复"],
            ]}
          />
          <FilterSelect
            label="重复检查状态筛选"
            value={preferences.filters.duplicateCheckStatuses[0] ?? ""}
            onChange={(value) =>
              updateSingleFilter(
                "duplicateCheckStatuses",
                value ? [value as DuplicateCheckStatus] : [],
              )
            }
            options={[
              ["", "全部检查状态"],
              ["none", "未发现相关实例"],
              ["exact", "完全重复"],
              ["suspected", "疑似重复"],
              ["nameConflict", "同名冲突"],
            ]}
          />
          <FilterSelect
            label="排序方式"
            value={`${preferences.sort.field}:${preferences.sort.direction}`}
            onChange={updateSort}
            options={[
              ["name:asc", "名称 A–Z"],
              ["name:desc", "名称 Z–A"],
              ["modifiedAt:desc", "最近修改"],
              ["createdAt:desc", "最近创建"],
              ["root:asc", "根目录"],
              ["duplicateCheckStatus:asc", "重复检查状态"],
            ]}
          />
          <button
            className="density-button"
            aria-label="切换列表密度"
            onClick={() =>
              setPreferences((current) => ({
                ...current,
                density: current.density === "compact" ? "comfortable" : "compact",
              }))
            }
          >
            {preferences.density === "compact" ? "紧凑" : "舒适"}
          </button>
          {hasConditions ? (
            <button
              className="clear-filters"
              aria-label="清空检索与筛选"
              onClick={() => {
                setQueryText("");
                updateFilters(EMPTY_FILTERS);
              }}
            >
              清空
            </button>
          ) : null}
        </div>
        {searchError ? <p className="search-error">检索失败：{searchError}</p> : null}
        <div className="catalog-workspace">
          <div className={`table-shell ${preferences.density}`}>
            <div className="table-header" aria-hidden="true">
              <span>Skill</span>
              <span>相对路径</span>
              <span>状态</span>
            </div>
            {instances.length ? (
              <ul className="skill-list" aria-label="本地 Skill">
                {instances.map((skill) => (
                  <SkillRow
                    key={skill.id}
                    skill={skill}
                    root={rootsById.get(skill.rootId)}
                    selected={skill.id === selectedInstanceId}
                    onSelect={() => setSelectedInstanceId(skill.id)}
                  />
                ))}
              </ul>
            ) : (
              <div className="no-results">
                <strong>没有匹配的 Skill</strong>
                <span>尝试清空检索词或调整筛选条件。</span>
              </div>
            )}
          </div>
          <SkillDetailPanel
            detail={detail}
            error={detailError}
            preview={previewInstanceId === selectedInstanceId ? preview : null}
            previewPath={previewInstanceId === selectedInstanceId ? previewPath : null}
            busy={editBusy}
            onPreview={previewFile}
            onEdit={() => openExistingEditor()}
            onEditText={(path, content) =>
              openExistingEditor({
                relativePath: path,
                operation: { kind: "writeText", content },
              })
            }
            onDelete={(path) =>
              openExistingEditor({ relativePath: path, operation: { kind: "delete" } })
            }
            onReplaceBinary={(path, content) =>
              openExistingEditor({
                relativePath: path,
                operation: { kind: "replaceBinary", content },
              })
            }
          />
        </div>
      </section>
      {editorDraft ? (
        <SkillEditor
          draft={editorDraft}
          roots={snapshot.roots}
          validation={validation}
          plan={changePlan}
          busy={editBusy}
          error={editError}
          onChange={(nextDraft) => {
            setEditorDraft(nextDraft);
            setValidation(null);
            setChangePlan(null);
          }}
          onClose={() => setEditorDraft(null)}
          onPreviewChanges={previewChanges}
          onConfirm={confirmChanges}
        />
      ) : null}
    </main>
  );
}

function FilterSelect({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: string[][];
  onChange(value: string): void;
}) {
  return (
    <label className="filter-control">
      <span>{label.replace("筛选", "")}</span>
      <select
        aria-label={label}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        {options.map(([optionValue, optionLabel]) => (
          <option key={optionValue} value={optionValue}>
            {optionLabel}
          </option>
        ))}
      </select>
    </label>
  );
}

function shortRoot(path: string) {
  const parts = path.split("/").filter(Boolean);
  return parts.length > 2 ? `…/${parts.slice(-2).join("/")}` : path;
}

const PRESET_ROOTS = [
  { client: "Codex", path: "~/.codex/skills", matchToken: "/.codex/" },
  { client: "Claude", path: "~/.claude/skills", matchToken: "/.claude/" },
  { client: "Gemini", path: "~/.gemini/skills", matchToken: "/.gemini/" },
  {
    client: "OpenCode",
    path: "~/.config/opencode/skills",
    matchToken: "/opencode/",
  },
  { client: "Hermes", path: "~/.hermes/skills", matchToken: "/.hermes/" },
];

function RootManagement({
  snapshot,
  busyRootId,
  onBack,
  onChooseRoot,
  onRescan,
  onRemove,
}: {
  snapshot: WorkspaceSnapshot;
  busyRootId: number | null;
  onBack(): void;
  onChooseRoot(): void;
  onRescan(rootId: number): void;
  onRemove(rootId: number): void;
}) {
  return (
    <main className="root-management-page">
      <div className="root-management-title">
        <div>
          <button className="text-button" onClick={onBack}>
            ← 返回 Skill 列表
          </button>
          <p className="eyebrow">本地访问范围</p>
          <h1>Skill 根目录</h1>
          <p>每个目录独立授权、扫描和报告状态。</p>
        </div>
        <button className="primary-button" onClick={onChooseRoot}>
          添加根目录
        </button>
      </div>

      <section className="preset-section" aria-labelledby="preset-title">
        <div className="section-heading">
          <div>
            <h2 id="preset-title">常见客户端目录</h2>
            <p>可按以下位置快速找到各客户端通常使用的 Skill。</p>
          </div>
        </div>
        <div className="preset-grid">
          {PRESET_ROOTS.map((preset) => (
            <article className="preset-card" key={preset.client}>
              <strong>{preset.client}</strong>
              <code>{preset.path}</code>
            </article>
          ))}
        </div>
      </section>

      <section className="managed-roots" aria-labelledby="managed-title">
        <div className="section-heading">
          <div>
            <h2 id="managed-title">已纳管目录</h2>
            <p>{snapshot.roots.length} 个目录，移除只会取消纳管。</p>
          </div>
        </div>
        <ul className="root-list">
          {snapshot.roots.map((root) => (
            <RootRow
              key={root.id}
              root={root}
              busy={busyRootId === root.id}
              onRescan={() => onRescan(root.id)}
              onRemove={() => onRemove(root.id)}
            />
          ))}
        </ul>
        <p className="safe-removal-note">
          移除根目录不会删除目录内的任何 Skill 文件，也不会修改原始内容。
        </p>
      </section>
    </main>
  );
}

function RootRow({
  root,
  busy,
  onRescan,
  onRemove,
}: {
  root: SkillRoot;
  busy: boolean;
  onRescan(): void;
  onRemove(): void;
}) {
  const status = rootStatus(root);
  return (
    <li className={`managed-root ${status.tone}`}>
      <span className="root-health" aria-hidden="true" />
      <span className="managed-root-copy">
        <code>{root.path}</code>
        <span className="root-status">{status.label}</span>
        {root.error ? <small className="root-error">{root.error}</small> : null}
        {root.recoveryHint ? <small>{root.recoveryHint}</small> : null}
      </span>
      <span className="root-actions">
        <button
          className="secondary-button compact"
          aria-label={`重新扫描 ${root.path}`}
          onClick={onRescan}
          disabled={busy}
        >
          {busy ? "处理中…" : "重新扫描"}
        </button>
        <button
          className="danger-button"
          aria-label={`移除 ${root.path}`}
          onClick={onRemove}
          disabled={busy}
        >
          移除
        </button>
      </span>
    </li>
  );
}

function rootStatus(root: SkillRoot) {
  switch (root.status) {
    case "partialFailure":
      return { label: "部分目录读取失败", tone: "warning" };
    case "missing":
      return { label: "路径不存在", tone: "danger" };
    case "permissionDenied":
      return { label: "无访问权限", tone: "danger" };
    default:
      return { label: "可访问", tone: "ready" };
  }
}

function SkillRow({
  skill,
  root,
  selected,
  onSelect,
}: {
  skill: SkillInstance;
  root?: SkillRoot;
  selected: boolean;
  onSelect(): void;
}) {
  const needsRepair = skill.status === "needsRepair";
  return (
    <li
      className={`skill-row${needsRepair ? " repair" : ""}${selected ? " selected" : ""}`}
      aria-label={`${skill.name}，${needsRepair ? "需要修复" : "正常"}`}
      aria-current={selected ? "true" : undefined}
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") onSelect();
      }}
    >
      <span className="file-spine" aria-hidden="true" />
      <span className="skill-copy">
        <strong>{skill.name}</strong>
        {skill.description ? <small>{skill.description}</small> : null}
        {skill.error ? <small className="skill-error">{skill.error}</small> : null}
        {skill.linkPath ? (
          <small className="skill-link">
            链接 {skill.linkPath} → {skill.realPath}
          </small>
        ) : null}
      </span>
      <code title={root?.path}>
        {root ? `${clientName(root.path)} · ${skill.relativePath}` : skill.relativePath}
      </code>
      <span className={needsRepair ? "status-badge repair" : "status-badge"}>
        {needsRepair ? "需要修复" : "正常"}
      </span>
    </li>
  );
}

function SkillDetailPanel({
  detail,
  error,
  preview,
  previewPath,
  busy,
  onPreview,
  onEdit,
  onEditText,
  onDelete,
  onReplaceBinary,
}: {
  detail: SkillDetail | null;
  error: string | null;
  preview: SkillFilePreview | null;
  previewPath: string | null;
  busy: boolean;
  onPreview(relativePath: string): void;
  onEdit(): void;
  onEditText(relativePath: string, content: string): void;
  onDelete(relativePath: string): void;
  onReplaceBinary(relativePath: string, content: number[]): void;
}) {
  if (error) {
    return <aside className="skill-detail-panel detail-empty">详情读取失败：{error}</aside>;
  }
  if (!detail) {
    return <aside className="skill-detail-panel detail-empty">选择一个 Skill 查看详情。</aside>;
  }

  return (
    <aside className="skill-detail-panel" aria-label="Skill 详情">
      <div className="detail-heading">
        <div>
          <p className="eyebrow">Skill 详情</p>
          <h2>{detail.instance.name}</h2>
        </div>
        <button className="primary-button compact" onClick={onEdit} disabled={busy}>
          编辑 Skill
        </button>
      </div>
      <p className="detail-description">{detail.instance.description || "暂无描述"}</p>
      <dl className="detail-metadata">
        <div>
          <dt>客户端</dt>
          <dd>{skillClientName(detail.instance.client)}</dd>
        </div>
        <div>
          <dt>根目录</dt>
          <dd title={detail.root.path}>{shortRoot(detail.root.path)}</dd>
        </div>
        <div>
          <dt>相对路径</dt>
          <dd>{detail.instance.relativePath}</dd>
        </div>
        <div>
          <dt>修改时间</dt>
          <dd>{formatTimestamp(detail.instance.modifiedAt)}</dd>
        </div>
      </dl>
      {detail.tags.length || detail.skillGroups.length ? (
        <div className="detail-labels">
          {detail.tags.map((tag) => (
            <span className="tag-chip" key={tag}>#{tag}</span>
          ))}
          {detail.skillGroups.map((group) => (
            <span className="group-chip" key={group}>{group}</span>
          ))}
        </div>
      ) : null}
      <section className="file-section" aria-labelledby="file-section-title">
        <div className="file-section-heading">
          <h3 id="file-section-title">目录文件</h3>
          <span>{detail.fileCount} 个文件</span>
        </div>
        <ul className="file-tree">
          {detail.files.map((file) => (
            <li key={file.relativePath}>
              <button
                className={previewPath === file.relativePath ? "file-button active" : "file-button"}
                aria-label={`预览 ${file.relativePath}`}
                onClick={() => onPreview(file.relativePath)}
                disabled={file.kind === "directory" || file.kind === "symbolicLink"}
              >
                <span aria-hidden="true">{fileKindIcon(file.kind)}</span>
                <b>{file.relativePath}</b>
                <small>{file.kind === "binary" ? formatBytes(file.size) : fileKindName(file.kind)}</small>
              </button>
            </li>
          ))}
        </ul>
      </section>
      {preview && previewPath ? (
        <section className="file-preview" aria-label={`${previewPath} 预览`}>
          <div className="preview-heading">
            <strong>{previewPath}</strong>
            {previewPath !== "SKILL.md" ? (
              <span className="preview-actions">
                {preview.kind === "text" ? (
                  <button className="text-button inline" onClick={() => onEditText(previewPath, preview.content)}>
                    编辑此文本文件
                  </button>
                ) : (
                  <label className="replace-binary-button">
                    替换附件
                    <input
                      type="file"
                      aria-label={`替换 ${previewPath}`}
                      onChange={async (event) => {
                        const file = event.target.files?.[0];
                        if (!file) return;
                        onReplaceBinary(previewPath, Array.from(new Uint8Array(await file.arrayBuffer())));
                      }}
                    />
                  </label>
                )}
                <button className="text-button inline danger-text" onClick={() => onDelete(previewPath)}>
                  删除文件
                </button>
              </span>
            ) : null}
          </div>
          {preview.kind === "text" ? (
            <pre>{preview.content}</pre>
          ) : (
            <div className="binary-preview">
              {preview.mediaType && preview.previewContent ? (
                <img
                  src={binaryPreviewDataUrl(preview.mediaType, preview.previewContent)}
                  alt={`附件预览 ${previewPath}`}
                />
              ) : null}
              <strong>二进制附件</strong>
              <span>{formatBytes(preview.size)}，不会以文本方式打开。</span>
            </div>
          )}
        </section>
      ) : null}
    </aside>
  );
}

function SkillEditor({
  draft,
  roots,
  validation,
  plan,
  busy,
  error,
  onChange,
  onClose,
  onPreviewChanges,
  onConfirm,
}: {
  draft: SkillDraft;
  roots: SkillRoot[];
  validation: SkillDraftValidation | null;
  plan: SkillChangePlan | null;
  busy: boolean;
  error: string | null;
  onChange(draft: SkillDraft): void;
  onClose(): void;
  onPreviewChanges(): void;
  onConfirm(): void;
}) {
  const newTarget = draft.target.kind === "new" ? draft.target : null;
  return (
    <div className="editor-backdrop" role="presentation">
      <section className="skill-editor" role="dialog" aria-modal="true" aria-labelledby="editor-title">
        <header className="editor-header">
          <div>
            <p className="eyebrow">安全编辑</p>
            <h2 id="editor-title">{draft.target.kind === "new" ? "新建 Skill" : `编辑 ${draft.name}`}</h2>
          </div>
          <button className="secondary-button compact" onClick={onClose}>关闭</button>
        </header>
        <div className="editor-grid">
          <div className="editor-fields">
            {newTarget ? (
              <>
                <label>
                  <span>根目录</span>
                  <select
                    aria-label="新 Skill 根目录"
                    value={newTarget.rootId}
                    onChange={(event) =>
                      onChange({
                        ...draft,
                        target: {
                          ...newTarget,
                          rootId: Number(event.target.value),
                        },
                      })
                    }
                  >
                    {roots.map((root) => <option key={root.id} value={root.id}>{root.path}</option>)}
                  </select>
                </label>
                <label>
                  <span>目录名称</span>
                  <input
                    aria-label="Skill 目录名称"
                    value={newTarget.relativePath}
                    onChange={(event) =>
                      onChange({
                        ...draft,
                        target: { ...newTarget, relativePath: event.target.value },
                      })
                    }
                  />
                </label>
              </>
            ) : null}
            <label>
              <span>Skill 名称</span>
              <input
                aria-label="Skill 名称"
                value={draft.name}
                onChange={(event) => onChange({ ...draft, name: event.target.value })}
              />
            </label>
            <label>
              <span>Skill 描述</span>
              <textarea
                aria-label="Skill 描述"
                rows={3}
                value={draft.description}
                onChange={(event) => onChange({ ...draft, description: event.target.value })}
              />
            </label>
            <label className="markdown-field">
              <span>SKILL.md 正文</span>
              <textarea
                aria-label="Markdown 正文"
                value={draft.markdownBody}
                onChange={(event) => onChange({ ...draft, markdownBody: event.target.value })}
              />
            </label>
            {draft.fileChanges.map((change, index) =>
              change.operation.kind === "writeText" ? (
                <label className="markdown-field" key={`${change.relativePath}-${index}`}>
                  <span>文本文件 · {change.relativePath}</span>
                  <textarea
                    aria-label={`编辑 ${change.relativePath}`}
                    value={change.operation.content}
                    onChange={(event) => {
                      const fileChanges = [...draft.fileChanges];
                      fileChanges[index] = {
                        ...change,
                        operation: { kind: "writeText", content: event.target.value },
                      };
                      onChange({ ...draft, fileChanges });
                    }}
                  />
                </label>
              ) : change.operation.kind === "replaceBinary" ? (
                <div className="pending-file-change binary" key={`${change.relativePath}-${index}`}>
                  将替换附件 <code>{change.relativePath}</code>（{formatBytes(change.operation.content.length)}）
                </div>
              ) : (
                <div className="pending-file-change" key={`${change.relativePath}-${index}`}>
                  将删除 <code>{change.relativePath}</code>
                </div>
              ),
            )}
          </div>
          <div className="editor-preview-column">
            <section className="markdown-preview">
              <p className="eyebrow">Markdown 预览</p>
              <h3>{draft.name || "未命名 Skill"}</h3>
              <p>{draft.description || "填写描述后会显示在这里。"}</p>
              <pre>{draft.markdownBody}</pre>
            </section>
            {validation && !validation.valid ? (
              <section className="validation-panel" role="alert">
                <strong>保存前需要修复</strong>
                <ul>{validation.issues.map((issue) => <li key={`${issue.field}-${issue.message}`}>{issue.message}</li>)}</ul>
              </section>
            ) : null}
            {plan ? (
              <section className="change-plan" aria-label="不可变变化计划">
                <div>
                  <p className="eyebrow">变化计划 #{plan.id}</p>
                  <strong>确认后一次性写入</strong>
                </div>
                <ul>
                  {plan.changes.map((change) => (
                    <li key={`${change.kind}-${change.relativePath}`}>
                      <span className={`change-kind ${change.kind}`}>
                        {changeKindName(change.kind)} {change.relativePath}
                      </span>
                    </li>
                  ))}
                </ul>
                <p>写入前会生成本地备份，失败时不会留下部分修改。</p>
              </section>
            ) : null}
            {error ? <p className="editor-error" role="alert">{error}</p> : null}
          </div>
        </div>
        <footer className="editor-footer">
          <button className="secondary-button" onClick={onClose}>取消</button>
          <button className="secondary-button" onClick={onPreviewChanges} disabled={busy}>
            {busy ? "正在检查…" : "预览变化"}
          </button>
          {plan ? (
            <button className="primary-button" onClick={onConfirm} disabled={busy}>
              {busy ? "正在保存…" : "确认保存"}
            </button>
          ) : null}
        </footer>
      </section>
    </div>
  );
}

function stripFrontmatter(content: string) {
  const complete = content.match(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/);
  if (complete) return content.slice(complete[0].length).replace(/^\r?\n/, "");
  if (content.startsWith("---\n") || content.startsWith("---\r\n")) {
    const bodyStart = content.search(/\r?\n\r?\n/);
    return bodyStart >= 0 ? content.slice(bodyStart).replace(/^\s+/, "") : "";
  }
  return content;
}

function binaryPreviewDataUrl(mediaType: string, content: number[]) {
  let binary = "";
  for (const byte of content) binary += String.fromCharCode(byte);
  return `data:${mediaType};base64,${window.btoa(binary)}`;
}

function skillClientName(client: SkillClient) {
  return {
    claude: "Claude",
    codex: "Codex",
    gemini: "Gemini",
    openCode: "OpenCode",
    hermes: "Hermes",
    other: "自定义",
  }[client];
}

function formatTimestamp(value: number) {
  const milliseconds = value < 1_000_000_000_000 ? value * 1000 : value;
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(milliseconds));
}

function formatBytes(size: number) {
  return size < 1024 ? `${size} B` : `${(size / 1024).toFixed(1)} KB`;
}

function fileKindIcon(kind: SkillDetail["files"][number]["kind"]) {
  return { directory: "▾", text: "¶", binary: "◆", symbolicLink: "↗" }[kind];
}

function fileKindName(kind: SkillDetail["files"][number]["kind"]) {
  return { directory: "目录", text: "文本", binary: "附件", symbolicLink: "链接" }[kind];
}

function changeKindName(kind: SkillChangePlan["changes"][number]["kind"]) {
  return { create: "新增", overwrite: "覆盖", delete: "删除" }[kind];
}

function clientName(path: string) {
  return (
    PRESET_ROOTS.find((preset) => path.includes(preset.matchToken))?.client ?? "自定义"
  );
}

function ErrorBanner({ message }: { message: string }) {
  return (
    <div className="error-banner" role="alert">
      <strong>无法读取 Skill 根目录</strong>
      <span>{message}</span>
    </div>
  );
}

function readableError(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason);
}
