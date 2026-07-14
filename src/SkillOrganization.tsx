import { useMemo, useState } from "react";

import type {
  OrganizationSkillGroup,
  SkillInstance,
  SkillOrganizationChange,
  SkillOrganizationSnapshot,
} from "./models";
import "./skill-organization.css";

interface DialogBaseProps {
  organization: SkillOrganizationSnapshot;
  busy: boolean;
  error: string | null;
  onClose(): void;
}

export function OrganizationChangeDialog({
  organization,
  selectedInstances,
  busy,
  error,
  onClose,
  onApply,
}: DialogBaseProps & {
  selectedInstances: SkillInstance[];
  onApply(change: SkillOrganizationChange): unknown;
}) {
  const selectedIds = selectedInstances.map((instance) => instance.id);
  const selectedEntries = selectedIds.map(
    (id) =>
      organization.instances.find((entry) => entry.instanceId === id) ?? {
        instanceId: id,
        tags: [],
        groupIds: [],
      },
  );
  const allGroupIds = organization.groups
    .filter((group) => selectedIds.every((id) => group.instanceIds.includes(id)))
    .map((group) => group.id);
  const anyGroupIds = organization.groups
    .filter((group) => selectedIds.some((id) => group.instanceIds.includes(id)))
    .map((group) => group.id);
  const [targetGroupIds, setTargetGroupIds] = useState(allGroupIds);
  const [touchedGroupIds, setTouchedGroupIds] = useState<number[]>([]);
  const [addTagsText, setAddTagsText] = useState("");
  const [removeTags, setRemoveTags] = useState<string[]>([]);
  const existingTags = useMemo(
    () => [...new Set(selectedEntries.flatMap((entry) => entry.tags))].sort(localeCompare),
    [selectedEntries],
  );

  function toggleGroup(groupId: number) {
    setTouchedGroupIds((current) =>
      current.includes(groupId) ? current : [...current, groupId],
    );
    setTargetGroupIds((current) =>
      current.includes(groupId)
        ? current.filter((id) => id !== groupId)
        : [...current, groupId],
    );
  }

  function submit() {
    const addTags = [...new Set(
      addTagsText
        .split(/[,，\n]+/)
        .map((tag) => tag.trim())
        .filter(Boolean),
    )];
    void onApply({
      instanceIds: selectedIds,
      addTags,
      removeTags,
      addGroupIds: targetGroupIds.filter(
        (id) => touchedGroupIds.includes(id) && !allGroupIds.includes(id),
      ),
      removeGroupIds: anyGroupIds.filter(
        (id) => touchedGroupIds.includes(id) && !targetGroupIds.includes(id),
      ),
    });
  }

  return (
    <DialogShell title={`整理 ${selectedIds.length} 个 Skill 实例`} onClose={onClose}>
      <p className="organization-dialog-copy">
        Skill 组、Skill 标签和顺序只保存在本机，不会移动或修改真实 Skill 文件。
      </p>
      {error ? <p className="organization-error" role="alert">整理失败：{error}</p> : null}
      <section className="organization-form-section">
        <h3>添加多个 Skill 标签</h3>
        <label>
          <span>添加 Skill 标签</span>
          <textarea
            aria-label="添加 Skill 标签"
            placeholder="用逗号分隔，例如：API，常用，安全审计"
            value={addTagsText}
            onChange={(event) => setAddTagsText(event.target.value)}
          />
        </label>
      </section>
      <section className="organization-form-section">
        <h3>移除已有 Skill 标签</h3>
        <div className="organization-check-grid">
          {existingTags.map((tag) => (
            <label key={tag}>
              <input
                type="checkbox"
                aria-label={`移除 Skill 标签 ${tag}`}
                checked={removeTags.includes(tag)}
                onChange={() =>
                  setRemoveTags((current) =>
                    current.includes(tag)
                      ? current.filter((value) => value !== tag)
                      : [...current, tag],
                  )
                }
              />
              <span>#{tag}</span>
            </label>
          ))}
          {existingTags.length === 0 ? <small>所选实例还没有 Skill 标签。</small> : null}
        </div>
      </section>
      <section className="organization-form-section">
        <h3>加入或移出 Skill 组</h3>
        <div className="organization-check-grid groups">
          {organization.groups.map((group) => (
            <label key={group.id}>
              <input
                type="checkbox"
                aria-label={`Skill 组 ${group.name}`}
                ref={(input) => {
                  if (input) {
                    input.indeterminate =
                      anyGroupIds.includes(group.id) &&
                      !allGroupIds.includes(group.id) &&
                      !touchedGroupIds.includes(group.id);
                  }
                }}
                checked={targetGroupIds.includes(group.id)}
                onChange={() => toggleGroup(group.id)}
              />
              <span>{group.name}</span>
              <small>
                {anyGroupIds.includes(group.id) && !allGroupIds.includes(group.id)
                  ? "部分已加入"
                  : `${group.instanceIds.length}`}
              </small>
            </label>
          ))}
          {organization.groups.length === 0 ? <small>请先创建一个 Skill 组。</small> : null}
        </div>
      </section>
      <footer className="organization-dialog-actions">
        <button className="secondary-button" onClick={onClose}>取消</button>
        <button className="primary-button" onClick={submit} disabled={busy || selectedIds.length === 0}>
          {busy ? "正在应用…" : "应用整理"}
        </button>
      </footer>
    </DialogShell>
  );
}

export function GroupManagementDialog({
  organization,
  instances,
  busy,
  error,
  onClose,
  onCreate,
  onRename,
  onDelete,
  onReorder,
}: DialogBaseProps & {
  instances: SkillInstance[];
  onCreate(name: string): unknown;
  onRename(id: number, name: string): unknown;
  onDelete(id: number): unknown;
  onReorder(id: number, instanceIds: string[]): unknown;
}) {
  const [newName, setNewName] = useState("");
  const [names, setNames] = useState<Record<number, string>>(() =>
    Object.fromEntries(organization.groups.map((group) => [group.id, group.name])),
  );
  const [orderingGroup, setOrderingGroup] = useState<OrganizationSkillGroup | null>(null);
  const [orderedIds, setOrderedIds] = useState<string[]>([]);

  function beginOrder(group: OrganizationSkillGroup) {
    setOrderingGroup(group);
    setOrderedIds(group.instanceIds);
  }

  function move(index: number, direction: -1 | 1) {
    const target = index + direction;
    if (target < 0 || target >= orderedIds.length) return;
    setOrderedIds((current) => {
      const next = [...current];
      [next[index], next[target]] = [next[target], next[index]];
      return next;
    });
  }

  return (
    <DialogShell title="管理 Skill 组" onClose={onClose}>
      <p className="organization-dialog-copy">创建虚拟 Skill 组，或调整组内实例的展示顺序。</p>
      {error ? <p className="organization-error" role="alert">Skill 组操作失败：{error}</p> : null}
      <form
        className="create-group-form"
        onSubmit={(event) => {
          event.preventDefault();
          if (!newName.trim()) return;
          void onCreate(newName.trim());
          setNewName("");
        }}
      >
        <label><span>新 Skill 组名称</span><input aria-label="新 Skill 组名称" value={newName} onChange={(event) => setNewName(event.target.value)} /></label>
        <button className="primary-button" disabled={busy || !newName.trim()}>创建 Skill 组</button>
      </form>
      <ul className="group-management-list">
        {organization.groups.map((group) => (
          <li key={group.id}>
            <div className="group-edit-row">
              <input
                aria-label={`Skill 组名称 ${group.name}`}
                value={names[group.id] ?? group.name}
                onChange={(event) => setNames((current) => ({ ...current, [group.id]: event.target.value }))}
              />
              <span>{group.instanceIds.length} 个实例</span>
              <button className="secondary-button compact" aria-label={`保存${group.name}名称`} onClick={() => void onRename(group.id, names[group.id] ?? group.name)} disabled={busy}>保存</button>
              <button className="secondary-button compact" aria-label={`调整${group.name}顺序`} onClick={() => beginOrder(group)} disabled={busy || group.instanceIds.length < 2}>调整顺序</button>
              <button className="text-button danger-text" aria-label={`删除${group.name}`} onClick={() => void onDelete(group.id)} disabled={busy}>删除</button>
            </div>
          </li>
        ))}
      </ul>
      {organization.groups.length === 0 ? <div className="organization-empty">还没有 Skill 组。</div> : null}
      {orderingGroup ? (
        <section className="group-order-editor">
          <header><div><small>自定义顺序</small><h3>{orderingGroup.name}</h3></div><button className="text-button" onClick={() => setOrderingGroup(null)}>取消调整</button></header>
          <ol>
            {orderedIds.map((instanceId, index) => {
              const instance = instances.find((candidate) => candidate.id === instanceId);
              const name = instance?.name ?? instanceId;
              return (
                <li key={instanceId}>
                  <span>{index + 1}</span><b>{name}</b>
                  <button aria-label={`上移 ${name}`} onClick={() => move(index, -1)} disabled={index === 0}>↑</button>
                  <button aria-label={`下移 ${name}`} onClick={() => move(index, 1)} disabled={index === orderedIds.length - 1}>↓</button>
                </li>
              );
            })}
          </ol>
          <button className="primary-button" onClick={() => void onReorder(orderingGroup.id, orderedIds)} disabled={busy}>保存自定义顺序</button>
        </section>
      ) : null}
      <footer className="organization-dialog-actions"><button className="secondary-button" onClick={onClose}>关闭</button></footer>
    </DialogShell>
  );
}

function DialogShell({ title, onClose, children }: { title: string; onClose(): void; children: React.ReactNode }) {
  return (
    <div className="organization-dialog-backdrop">
      <section className="organization-dialog" role="dialog" aria-modal="true" aria-label={title}>
        <header className="organization-dialog-heading"><div><p>本地虚拟整理</p><h2>{title}</h2></div><button className="secondary-button compact" onClick={onClose}>关闭</button></header>
        <div className="organization-dialog-body">{children}</div>
      </section>
    </div>
  );
}

function localeCompare(left: string, right: string) {
  return left.localeCompare(right, "zh-CN");
}
