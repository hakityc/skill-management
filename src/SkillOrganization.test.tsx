import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test } from "vitest";

import {
  GroupManagementDialog,
  OrganizationChangeDialog,
} from "./SkillOrganization";
import type {
  SkillInstance,
  SkillOrganizationChange,
  SkillOrganizationSnapshot,
} from "./models";

afterEach(cleanup);

const organization: SkillOrganizationSnapshot = {
  groups: [
    { id: 1, name: "发布流程", instanceIds: ["alpha", "beta"] },
    { id: 2, name: "安全审查", instanceIds: ["alpha"] },
  ],
  instances: [
    { instanceId: "alpha", tags: ["发布", "常用"], groupIds: [1, 2] },
    { instanceId: "beta", tags: ["发布"], groupIds: [1] },
  ],
};

describe("Skill 组与标签整理", () => {
  test("个人用户批量添加和移除多个标签与 Skill 组", async () => {
    let submitted: SkillOrganizationChange | null = null;
    render(
      <OrganizationChangeDialog
        organization={organization}
        selectedInstances={[skill("alpha", "alpha"), skill("beta", "beta")]}
        busy={false}
        error={null}
        onClose={() => {}}
        onApply={(change) => {
          submitted = change;
        }}
      />,
    );

    expect(screen.getByRole("heading", { name: "整理 2 个 Skill 实例" })).toBeTruthy();
    await userEvent.type(screen.getByRole("textbox", { name: "添加标签" }), "API，安全审计");
    await userEvent.click(screen.getByRole("checkbox", { name: "移除标签 常用" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Skill 组 发布流程" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "Skill 组 安全审查" }));
    await userEvent.click(screen.getByRole("button", { name: "应用整理" }));

    expect(submitted).toEqual({
      instanceIds: ["alpha", "beta"],
      addTags: ["API", "安全审计"],
      removeTags: ["常用"],
      addGroupIds: [2],
      removeGroupIds: [1],
    });
  });

  test("个人用户创建、重命名、删除 Skill 组并调整成员顺序", async () => {
    const calls: string[] = [];
    render(
      <GroupManagementDialog
        organization={organization}
        instances={[skill("alpha", "alpha"), skill("beta", "beta")]}
        busy={false}
        error={null}
        onClose={() => {}}
        onCreate={(name) => calls.push(`create:${name}`)}
        onRename={(id, name) => calls.push(`rename:${id}:${name}`)}
        onDelete={(id) => calls.push(`delete:${id}`)}
        onReorder={(id, ids) => calls.push(`reorder:${id}:${ids.join(",")}`)}
      />,
    );

    await userEvent.type(screen.getByRole("textbox", { name: "新 Skill 组名称" }), "日常工作");
    await userEvent.click(screen.getByRole("button", { name: "创建 Skill 组" }));
    const rename = screen.getByRole("textbox", { name: "Skill 组名称 发布流程" });
    await userEvent.clear(rename);
    await userEvent.type(rename, "发布中心");
    await userEvent.click(screen.getByRole("button", { name: "保存发布流程名称" }));
    await userEvent.click(screen.getByRole("button", { name: "调整发布流程顺序" }));
    await userEvent.click(screen.getByRole("button", { name: "下移 alpha" }));
    await userEvent.click(screen.getByRole("button", { name: "保存自定义顺序" }));
    await userEvent.click(screen.getByRole("button", { name: "删除安全审查" }));

    expect(calls).toEqual([
      "create:日常工作",
      "rename:1:发布中心",
      "reorder:1:beta,alpha",
      "delete:2",
    ]);
  });
});

function skill(id: string, name: string): SkillInstance {
  return {
    id,
    rootId: 1,
    name,
    description: `${name} description`,
    relativePath: name,
    skillFilePath: `/skills/${name}/SKILL.md`,
    linkPath: null,
    realPath: `/skills/${name}`,
    status: "ready",
    error: null,
    client: "codex",
    duplicateCheckStatus: "none",
    createdAt: 1,
    modifiedAt: 1,
  };
}
