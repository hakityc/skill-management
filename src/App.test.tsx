import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test } from "vitest";

import { SkillManagerApp, type SkillGateway } from "./App";
import type {
  SkillDetail,
  SkillDraft,
  SkillOrganizationChange,
  SkillQuery,
  SkillWorkspaceViewPreferences,
  WorkspaceSnapshot,
} from "./models";

afterEach(cleanup);

const defaultViewPreferences: SkillWorkspaceViewPreferences = {
  filters: {
    clients: [],
    rootIds: [],
    repairStatus: "any",
    duplicateCheckStatuses: [],
  },
  sort: { field: "name", direction: "asc" },
  density: "compact",
};

const unavailableEditingMethods: Pick<
  SkillGateway,
  | "skillDetail"
  | "readSkillFile"
  | "validateSkillDraft"
  | "planSkillChange"
  | "executeSkillChange"
  | "undoSkillChange"
  | "latestUndoableSkillChange"
  | "reviewDuplicateGroups"
  | "saveDuplicateDecision"
  | "duplicateDecisions"
  | "restoreDuplicateDecision"
  | "skillOrganization"
  | "createSkillGroup"
  | "renameSkillGroup"
  | "deleteSkillGroup"
  | "applySkillOrganizationChange"
  | "reorderSkillGroup"
> = {
  skillDetail: async () => {
    throw new Error("当前测试不读取详情");
  },
  readSkillFile: async () => {
    throw new Error("当前测试不读取文件");
  },
  validateSkillDraft: async () => ({ valid: true, issues: [] }),
  planSkillChange: async () => ({ id: 1, changes: [] }),
  executeSkillChange: async () => {
    throw new Error("当前测试不执行编辑");
  },
  undoSkillChange: async () => {
    throw new Error("当前测试不执行撤销");
  },
  latestUndoableSkillChange: async () => null,
  reviewDuplicateGroups: async () => ({ groups: [], suppressedCount: 0 }),
  saveDuplicateDecision: async () => {},
  duplicateDecisions: async () => [],
  restoreDuplicateDecision: async () => {},
  skillOrganization: async () => ({ groups: [], instances: [] }),
  createSkillGroup: async () => ({ groups: [], instances: [] }),
  renameSkillGroup: async () => ({ groups: [], instances: [] }),
  deleteSkillGroup: async () => ({ groups: [], instances: [] }),
  applySkillOrganizationChange: async () => ({ groups: [], instances: [] }),
  reorderSkillGroup: async () => ({ groups: [], instances: [] }),
};

describe("Skill 管理器", () => {
  test("个人用户从中文空状态选择根目录后看到合法与需要修复的 Skill", async () => {
    const emptySnapshot: WorkspaceSnapshot = {
      authorizedRoot: null,
      roots: [],
      instances: [],
    };
    const scannedSnapshot: WorkspaceSnapshot = {
      authorizedRoot: "/Users/example/.codex/skills",
      roots: [
        {
          id: 1,
          path: "/Users/example/.codex/skills",
          status: "ready",
          error: null,
          recoveryHint: null,
        },
      ],
      instances: [
        {
          id: "api-review",
          rootId: 1,
          name: "api-review",
          description: "审查 API 设计与接口边界。",
          relativePath: "api-review",
          skillFilePath: "/Users/example/.codex/skills/api-review/SKILL.md",
          linkPath: "/Users/example/.codex/skills/api-review",
          realPath: "/Users/shared-skills/api-review",
          status: "ready",
          error: null,
          client: "codex",
          duplicateCheckStatus: "none",
          createdAt: 1,
          modifiedAt: 1,
        },
        {
          id: "broken-skill",
          rootId: 1,
          name: "broken-skill",
          description: "",
          relativePath: "broken-skill",
          skillFilePath: "/Users/example/.codex/skills/broken-skill/SKILL.md",
          linkPath: null,
          realPath: "/Users/example/.codex/skills/broken-skill",
          status: "needsRepair",
          error: "YAML frontmatter 缺少 description",
          client: "codex",
          duplicateCheckStatus: "none",
          createdAt: 1,
          modifiedAt: 1,
        },
      ],
    };
    let currentSnapshot = emptySnapshot;
    const gateway = {
      ...unavailableEditingMethods,
      loadSnapshot: async () => currentSnapshot,
      async chooseAndAuthorizeRoot() {
        currentSnapshot = scannedSnapshot;
        return currentSnapshot;
      },
      rescanRoot: async () => scannedSnapshot,
      removeRoot: async () => emptySnapshot,
      searchSkills: async () => ({
        instances: scannedSnapshot.instances,
        total: scannedSnapshot.instances.length,
      }),
      loadViewPreferences: async () => defaultViewPreferences,
      saveViewPreferences: async () => {},
    };

    render(<SkillManagerApp gateway={gateway} />);

    expect(
      await screen.findByRole("heading", { name: "管理你的本地 Skill" }),
    ).toBeTruthy();

    await userEvent.click(
      screen.getByRole("button", { name: "选择 Skill 根目录" }),
    );

    expect(
      await screen.findByRole("heading", { name: "全部 Skill" }),
    ).toBeTruthy();
    const readyRow = screen.getByRole("listitem", { name: "api-review，正常" });
    expect(within(readyRow).getByText("审查 API 设计与接口边界。")).toBeTruthy();
    expect(within(readyRow).getByText("Codex · api-review")).toBeTruthy();
    expect(
      within(readyRow).getByText(
        "链接 /Users/example/.codex/skills/api-review → /Users/shared-skills/api-review",
      ),
    ).toBeTruthy();
    const repairRow = screen.getByRole("listitem", {
      name: "broken-skill，需要修复",
    });
    expect(within(repairRow).getByText("需要修复")).toBeTruthy();
    expect(
      within(repairRow).getByText("YAML frontmatter 缺少 description"),
    ).toBeTruthy();
    expect(screen.getByText("/Users/example/.codex/skills")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "重复检查" }));
    expect(
      await screen.findByRole("heading", { name: "把相似，变成确定。" }),
    ).toBeTruthy();
  });

  test("个人用户管理常见客户端根目录并可单独重扫和安全移除", async () => {
    const initial: WorkspaceSnapshot = {
      authorizedRoot: "/Users/example/.codex/skills",
      roots: [
        {
          id: 1,
          path: "/Users/example/.codex/skills",
          status: "partialFailure",
          error: "符号链接 broken-skill 的目标不可访问",
          recoveryHint: "检查提示中的符号链接后重新扫描。",
        },
        {
          id: 2,
          path: "/Users/example/.claude/skills",
          status: "missing",
          error: "Skill 根目录不存在",
          recoveryHint: "确认目录未被移动或删除。",
        },
      ],
      instances: [],
    };
    let rescanned = false;
    let removed = false;
    const gateway = {
      ...unavailableEditingMethods,
      loadSnapshot: async () => initial,
      chooseAndAuthorizeRoot: async () => null,
      async rescanRoot(rootId: number) {
        rescanned = rootId === 1;
        return {
          ...initial,
          roots: initial.roots.map((root) =>
            root.id === rootId
              ? { ...root, status: "ready" as const, error: null, recoveryHint: null }
              : root,
          ),
        };
      },
      async removeRoot(rootId: number) {
        removed = rootId === 2;
        return {
          ...initial,
          roots: initial.roots.filter((root) => root.id !== rootId),
        };
      },
      searchSkills: async () => ({ instances: [], total: 0 }),
      loadViewPreferences: async () => defaultViewPreferences,
      saveViewPreferences: async () => {},
    };

    render(<SkillManagerApp gateway={gateway} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "管理根目录" }),
    );

    expect(screen.getByRole("heading", { name: "Skill 根目录" })).toBeTruthy();
    for (const client of ["Codex", "Claude", "Gemini", "OpenCode", "Hermes"]) {
      expect(screen.getByText(client)).toBeTruthy();
    }
    expect(screen.getByText("部分目录读取失败")).toBeTruthy();
    expect(screen.getByText("路径不存在")).toBeTruthy();
    expect(screen.getByText("确认目录未被移动或删除。")).toBeTruthy();

    await userEvent.click(
      screen.getByRole("button", { name: "重新扫描 /Users/example/.codex/skills" }),
    );
    expect(rescanned).toBe(true);
    expect(await screen.findByText("可访问")).toBeTruthy();

    await userEvent.click(
      screen.getByRole("button", { name: "移除 /Users/example/.claude/skills" }),
    );
    expect(removed).toBe(true);
    expect(screen.queryByText("/Users/example/.claude/skills")).toBeNull();
  });

  test("个人用户在中文工作台检索并组合筛选排序后可一键清空", async () => {
    const snapshot: WorkspaceSnapshot = {
      authorizedRoot: "/Users/example/.codex/skills",
      roots: [
        {
          id: 1,
          path: "/Users/example/.codex/skills",
          status: "ready",
          error: null,
          recoveryHint: null,
        },
      ],
      instances: [
        {
          id: "api-review",
          rootId: 1,
          name: "api-review",
          description: "审查接口边界。",
          relativePath: "api-review",
          skillFilePath: "/Users/example/.codex/skills/api-review/SKILL.md",
          linkPath: null,
          realPath: "/Users/example/.codex/skills/api-review",
          status: "needsRepair",
          error: "缺少字段",
          client: "codex",
          duplicateCheckStatus: "none",
          createdAt: 1,
          modifiedAt: 2,
        },
      ],
    };
    const queries: SkillQuery[] = [];
    const savedPreferences: SkillWorkspaceViewPreferences[] = [];
    const gateway = {
      ...unavailableEditingMethods,
      loadSnapshot: async () => snapshot,
      chooseAndAuthorizeRoot: async () => null,
      rescanRoot: async () => snapshot,
      removeRoot: async () => snapshot,
      loadViewPreferences: async () => defaultViewPreferences,
      async saveViewPreferences(preferences: SkillWorkspaceViewPreferences) {
        savedPreferences.push(preferences);
      },
      async searchSkills(query: SkillQuery) {
        queries.push(query);
        return { instances: snapshot.instances, total: snapshot.instances.length };
      },
    };

    render(<SkillManagerApp gateway={gateway} />);

    const search = await screen.findByRole("searchbox", { name: "搜索 Skill" });
    await userEvent.type(search, "replay");
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "Skill 客户端筛选" }),
      "codex",
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "根目录筛选" }),
      "1",
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "状态筛选" }),
      "needsRepair",
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "重复检查状态筛选" }),
      "none",
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "排序方式" }),
      "modifiedAt:desc",
    );

    await waitFor(() => {
      const lastQuery = queries.at(-1);
      expect(lastQuery?.text).toBe("replay");
      expect(lastQuery?.filters.clients).toEqual(["codex"]);
      expect(lastQuery?.filters.rootIds).toEqual([1]);
      expect(lastQuery?.filters.repairStatus).toBe("needsRepair");
      expect(lastQuery?.filters.duplicateCheckStatuses).toEqual(["none"]);
      expect(lastQuery?.sort).toEqual({ field: "modifiedAt", direction: "desc" });
    });
    expect(screen.getByText("检索“replay” · 1 个结果")).toBeTruthy();
    expect(savedPreferences.at(-1)?.sort.field).toBe("modifiedAt");

    await userEvent.click(
      screen.getByRole("button", { name: "清空检索与筛选" }),
    );
    await waitFor(() => {
      const lastQuery = queries.at(-1);
      expect(lastQuery?.text).toBe("");
      expect(lastQuery?.filters.clients).toEqual([]);
      expect(lastQuery?.filters.rootIds).toEqual([]);
      expect(lastQuery?.filters.repairStatus).toBe("any");
      expect(lastQuery?.filters.duplicateCheckStatuses).toEqual([]);
    });
  });

  test("个人用户查看文件详情、校验编辑、确认变化计划并撤销", async () => {
    const instance = {
      id: "api-review",
      rootId: 1,
      name: "api-review",
      description: "旧描述。",
      relativePath: "api-review",
      skillFilePath: "/Users/example/.codex/skills/api-review/SKILL.md",
      linkPath: null,
      realPath: "/Users/example/.codex/skills/api-review",
      status: "ready" as const,
      error: null,
      client: "codex" as const,
      duplicateCheckStatus: "none" as const,
      createdAt: 1,
      modifiedAt: 2,
    };
    const secondInstance = {
      ...instance,
      id: "release-notes",
      name: "release-notes",
      description: "整理发布说明。",
      relativePath: "release-notes",
      skillFilePath: "/Users/example/.codex/skills/release-notes/SKILL.md",
      realPath: "/Users/example/.codex/skills/release-notes",
      status: "needsRepair" as const,
      error: "frontmatter 缺少结束分隔线",
    };
    const snapshot: WorkspaceSnapshot = {
      authorizedRoot: "/Users/example/.codex/skills",
      roots: [
        {
          id: 1,
          path: "/Users/example/.codex/skills",
          status: "ready",
          error: null,
          recoveryHint: null,
        },
      ],
      instances: [instance, secondInstance],
    };
    const detail: SkillDetail = {
      instance,
      root: snapshot.roots[0],
      tags: ["API", "安全审计"],
      skillGroups: ["支付项目"],
      fileCount: 3,
      files: [
        { relativePath: "SKILL.md", kind: "text", size: 80, modifiedAt: 2 },
        {
          relativePath: "references/guide.md",
          kind: "text",
          size: 18,
          modifiedAt: 2,
        },
        { relativePath: "preview.png", kind: "binary", size: 120, modifiedAt: 2 },
      ],
    };
    const secondDetail: SkillDetail = {
      ...detail,
      instance: secondInstance,
      fileCount: 1,
      tags: [],
      skillGroups: [],
      files: [{ relativePath: "SKILL.md", kind: "text", size: 60, modifiedAt: 2 }],
    };
    let plannedDescription: string | null = null;
    let executed = false;
    let undone = false;
    const gateway = {
      loadSnapshot: async () => snapshot,
      chooseAndAuthorizeRoot: async () => null,
      rescanRoot: async () => snapshot,
      removeRoot: async () => snapshot,
      searchSkills: async () => ({ instances: snapshot.instances, total: 1 }),
      loadViewPreferences: async () => defaultViewPreferences,
      saveViewPreferences: async () => {},
      skillDetail: async (instanceId: string) =>
        instanceId === secondInstance.id ? secondDetail : detail,
      async readSkillFile(instanceId: string, relativePath: string) {
        if (relativePath === "preview.png") {
          return {
            kind: "binary" as const,
            size: 120,
            mediaType: "image/png",
            previewContent: [0x89, 0x50, 0x4e, 0x47],
          };
        }
        return {
          kind: "text" as const,
          content:
            relativePath === "SKILL.md"
              ? instanceId === secondInstance.id
                ? "---\nname: release-notes\ndescription: 整理发布说明。\n\n# 发布说明\n"
                : "---\nname: api-review\ndescription: 旧描述。\n---\n\n# API Review\n"
              : "检查幂等性。\n",
        };
      },
      async validateSkillDraft(draft: SkillDraft) {
        return draft.description
          ? { valid: true, issues: [] }
          : {
              valid: false,
              issues: [{ field: "description", message: "Skill 描述不能为空。" }],
            };
      },
      async planSkillChange(draft: SkillDraft) {
        plannedDescription = draft.description;
        return {
          id: 7,
          changes: [
            { relativePath: "SKILL.md", kind: "overwrite" as const, binary: false, size: 99 },
          ],
        };
      },
      async executeSkillChange() {
        executed = true;
        return { operationId: 9, snapshot };
      },
      async undoSkillChange() {
        undone = true;
        return { operationId: 9, snapshot };
      },
      latestUndoableSkillChange: async () => null,
      reviewDuplicateGroups: async () => ({ groups: [], suppressedCount: 0 }),
      saveDuplicateDecision: async () => {},
      duplicateDecisions: async () => [],
      restoreDuplicateDecision: async () => {},
      skillOrganization: async () => ({ groups: [], instances: [] }),
      createSkillGroup: async () => ({ groups: [], instances: [] }),
      renameSkillGroup: async () => ({ groups: [], instances: [] }),
      deleteSkillGroup: async () => ({ groups: [], instances: [] }),
      applySkillOrganizationChange: async () => ({ groups: [], instances: [] }),
      reorderSkillGroup: async () => ({ groups: [], instances: [] }),
    };

    render(<SkillManagerApp gateway={gateway} />);

    expect(await screen.findByRole("heading", { name: "api-review" })).toBeTruthy();
    expect(screen.getByText("3 个文件")).toBeTruthy();
    expect(screen.getByText("#安全审计")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "预览 references/guide.md" }));
    expect(await screen.findByText("检查幂等性。")).toBeTruthy();
    await userEvent.click(screen.getByRole("listitem", { name: "release-notes，需要修复" }));
    expect(await screen.findByRole("heading", { name: "release-notes" })).toBeTruthy();
    expect(screen.queryByText("检查幂等性。")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "编辑 Skill" }));
    expect(
      await screen.findByText(/SKILL\.md 元数据需要修复：frontmatter 缺少结束分隔线/),
    ).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "关闭" }));
    await userEvent.click(screen.getByRole("listitem", { name: "api-review，正常" }));
    expect(await screen.findByRole("heading", { name: "api-review" })).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "预览 preview.png" }));
    expect(await screen.findByText("二进制附件")).toBeTruthy();
    expect(screen.getByRole("img", { name: "附件预览 preview.png" })).toBeTruthy();
    expect(screen.getByLabelText("替换 preview.png")).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "编辑 Skill" }));
    const description = await screen.findByRole("textbox", { name: "Skill 描述" });
    await userEvent.clear(description);
    await userEvent.click(screen.getByRole("button", { name: "预览变化" }));
    expect(await screen.findByText("Skill 描述不能为空。")).toBeTruthy();
    await userEvent.type(description, "审查 API 与安全边界。");
    await userEvent.click(screen.getByRole("button", { name: "预览变化" }));
    expect(await screen.findByText("覆盖 SKILL.md")).toBeTruthy();
    expect(plannedDescription).toBe("审查 API 与安全边界。");
    await userEvent.click(screen.getByRole("button", { name: "确认保存" }));
    expect(executed).toBe(true);
    await userEvent.click(await screen.findByRole("button", { name: "撤销最近编辑" }));
    expect(undone).toBe(true);
  });

  test("个人用户通过自动视图、Skill 组顺序和多选批量整理日常工作台", async () => {
    const snapshot: WorkspaceSnapshot = {
      authorizedRoot: "/Users/me/.codex/skills",
      roots: [
        { id: 1, path: "/Users/me/.codex/skills", status: "ready", error: null, recoveryHint: null },
        { id: 2, path: "/Users/me/.claude/skills", status: "ready", error: null, recoveryHint: null },
      ],
      instances: [
        testInstance("alpha", "alpha", 1, "codex", "exact"),
        testInstance("beta", "beta", 2, "claude", "suspected"),
        { ...testInstance("repair", "repair", 1, "codex", "none"), status: "needsRepair", error: "缺少描述" },
      ],
    };
    const organization = {
      groups: [{ id: 7, name: "发布流程", instanceIds: ["beta", "alpha"] }],
      instances: [
        { instanceId: "alpha", tags: ["常用"], groupIds: [7] },
        { instanceId: "beta", tags: ["常用"], groupIds: [7] },
      ],
    };
    let applied: SkillOrganizationChange | null = null;
    const gateway: SkillGateway = {
      ...unavailableEditingMethods,
      loadSnapshot: async () => snapshot,
      chooseAndAuthorizeRoot: async () => null,
      rescanRoot: async () => snapshot,
      removeRoot: async () => snapshot,
      searchSkills: async () => ({ instances: snapshot.instances, total: 3 }),
      loadViewPreferences: async () => defaultViewPreferences,
      saveViewPreferences: async () => {},
      skillOrganization: async () => organization,
      async applySkillOrganizationChange(change) {
        applied = change;
        return organization;
      },
    };

    render(<SkillManagerApp gateway={gateway} />);

    expect(await screen.findByRole("button", { name: /Codex.*2/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Claude.*1/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /完全重复.*1/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /疑似重复.*1/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /需要修复.*1/ })).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: /发布流程.*2/ }));
    expect(await screen.findByRole("heading", { name: "发布流程" })).toBeTruthy();
    await waitFor(() => {
      const rows = within(screen.getByRole("list", { name: "本地 Skill" })).getAllByRole("listitem");
      expect(rows.map((row) => row.getAttribute("aria-label"))).toEqual([
        "beta，正常",
        "alpha，正常",
      ]);
    });

    await userEvent.click(screen.getByRole("checkbox", { name: "选择 alpha" }));
    await userEvent.click(screen.getByRole("checkbox", { name: "选择 beta" }));
    await userEvent.click(screen.getByRole("button", { name: "批量整理" }));
    await userEvent.type(screen.getByRole("textbox", { name: "添加标签" }), "API，安全审计");
    await userEvent.click(screen.getByRole("button", { name: "应用整理" }));
    expect(applied).toEqual({
      instanceIds: ["alpha", "beta"],
      addTags: ["API", "安全审计"],
      removeTags: [],
      addGroupIds: [],
      removeGroupIds: [],
    });
  });
});

function testInstance(
  id: string,
  name: string,
  rootId: number,
  client: "codex" | "claude",
  duplicateCheckStatus: "none" | "exact" | "suspected",
) {
  return {
    id,
    rootId,
    name,
    description: `${name} description`,
    relativePath: name,
    skillFilePath: `/skills/${name}/SKILL.md`,
    linkPath: null,
    realPath: `/skills/${name}`,
    status: "ready" as const,
    error: null,
    client,
    duplicateCheckStatus,
    createdAt: 1,
    modifiedAt: 1,
  };
}
