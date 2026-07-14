import { useEffect, useState } from "react";

import type { SkillInstance, WorkspaceSnapshot } from "./models";
import "./styles.css";

export interface SkillGateway {
  loadSnapshot(): Promise<WorkspaceSnapshot>;
  chooseAndAuthorizeRoot(): Promise<WorkspaceSnapshot | null>;
}

interface SkillManagerAppProps {
  gateway: SkillGateway;
}

const EMPTY_SNAPSHOT: WorkspaceSnapshot = {
  authorizedRoot: null,
  instances: [],
};

export function SkillManagerApp({ gateway }: SkillManagerAppProps) {
  const [snapshot, setSnapshot] = useState(EMPTY_SNAPSHOT);
  const [loading, setLoading] = useState(true);
  const [selecting, setSelecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      if (nextSnapshot) setSnapshot(nextSnapshot);
    } catch (reason) {
      setError(readableError(reason));
    } finally {
      setSelecting(false);
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
      ) : snapshot.authorizedRoot ? (
        <SkillLibrary snapshot={snapshot} />
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

function SkillLibrary({ snapshot }: { snapshot: WorkspaceSnapshot }) {
  const repairCount = snapshot.instances.filter(
    (skill) => skill.status === "needsRepair",
  ).length;
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
              <SkillRow key={skill.id} skill={skill} />
            ))}
          </ul>
        </div>
      </section>
    </main>
  );
}

function SkillRow({ skill }: { skill: SkillInstance }) {
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
      </span>
      <code>{skill.relativePath}</code>
      <span className={needsRepair ? "status-badge repair" : "status-badge"}>
        {needsRepair ? "需要修复" : "正常"}
      </span>
    </li>
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
