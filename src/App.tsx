import { useEffect, useState } from "react";

import type {
  DuplicateCheckStatus,
  SkillClient,
  SkillFilters,
  SkillInstance,
  SkillQuery,
  SkillRoot,
  SkillSearchResult,
  SkillSort,
  SkillWorkspaceViewPreferences,
  WorkspaceSnapshot,
} from "./models";
import "./styles.css";

export interface SkillGateway {
  loadSnapshot(): Promise<WorkspaceSnapshot>;
  chooseAndAuthorizeRoot(): Promise<WorkspaceSnapshot | null>;
  rescanRoot(rootId: number): Promise<WorkspaceSnapshot>;
  removeRoot(rootId: number): Promise<WorkspaceSnapshot>;
  searchSkills(query: SkillQuery): Promise<SkillSearchResult>;
  loadViewPreferences(): Promise<SkillWorkspaceViewPreferences>;
  saveViewPreferences(preferences: SkillWorkspaceViewPreferences): Promise<void>;
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
  const [view, setView] = useState<"library" | "roots">("library");

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
        ) : (
          <SkillLibrary
            gateway={gateway}
            snapshot={snapshot}
            onManageRoots={() => setView("roots")}
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
}: {
  gateway: SkillGateway;
  snapshot: WorkspaceSnapshot;
  onManageRoots(): void;
}) {
  const [queryText, setQueryText] = useState("");
  const [preferences, setPreferences] = useState(DEFAULT_VIEW_PREFERENCES);
  const [preferencesReady, setPreferencesReady] = useState(false);
  const [instances, setInstances] = useState(snapshot.instances);
  const [total, setTotal] = useState(snapshot.instances.length);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const repairCount = snapshot.instances.filter(
    (skill) => skill.status === "needsRepair",
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
          <span className="scan-status" aria-live="polite">
            <i /> {resultStatus}
          </span>
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
        <div className={`table-shell ${preferences.density}`}>
          <div className="table-header" aria-hidden="true">
            <span>Skill</span>
            <span>相对路径</span>
            <span>状态</span>
          </div>
          {instances.length ? (
            <ul className="skill-list" aria-label="本地 Skill">
              {instances.map((skill) => (
                <SkillRow key={skill.id} skill={skill} root={rootsById.get(skill.rootId)} />
              ))}
            </ul>
          ) : (
            <div className="no-results">
              <strong>没有匹配的 Skill</strong>
              <span>尝试清空检索词或调整筛选条件。</span>
            </div>
          )}
        </div>
      </section>
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

function SkillRow({ skill, root }: { skill: SkillInstance; root?: SkillRoot }) {
  const needsRepair = skill.status === "needsRepair";
  return (
    <li
      className={needsRepair ? "skill-row repair" : "skill-row"}
      aria-label={`${skill.name}，${needsRepair ? "需要修复" : "正常"}`}
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
