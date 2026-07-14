import { useEffect, useState } from "react";

import type { SkillInstance, SkillRoot, WorkspaceSnapshot } from "./models";
import "./styles.css";

export interface SkillGateway {
  loadSnapshot(): Promise<WorkspaceSnapshot>;
  chooseAndAuthorizeRoot(): Promise<WorkspaceSnapshot | null>;
  rescanRoot(rootId: number): Promise<WorkspaceSnapshot>;
  removeRoot(rootId: number): Promise<WorkspaceSnapshot>;
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
          <SkillLibrary snapshot={snapshot} onManageRoots={() => setView("roots")} />
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

function SkillLibrary({
  snapshot,
  onManageRoots,
}: {
  snapshot: WorkspaceSnapshot;
  onManageRoots(): void;
}) {
  const repairCount = snapshot.instances.filter(
    (skill) => skill.status === "needsRepair",
  ).length;
  const rootsById = new Map(snapshot.roots.map((root) => [root.id, root]));
  return (
    <main className="library-page">
      <aside className="library-sidebar">
        <p className="eyebrow">资料库</p>
        <button className="nav-item active">
          <span>全部 Skill</span>
          <b>{snapshot.instances.length}</b>
        </button>
        <button className="nav-item">
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
          <span className="scan-status">
            <i /> 已完成扫描
          </span>
        </div>
        <div className="table-shell">
          <div className="table-header" aria-hidden="true">
            <span>Skill</span>
            <span>相对路径</span>
            <span>状态</span>
          </div>
          <ul className="skill-list" aria-label="本地 Skill">
            {snapshot.instances.map((skill) => (
              <SkillRow key={skill.id} skill={skill} root={rootsById.get(skill.rootId)} />
            ))}
          </ul>
        </div>
      </section>
    </main>
  );
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
