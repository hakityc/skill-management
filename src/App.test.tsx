import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test } from "vitest";

import { SkillManagerApp } from "./App";
import type { WorkspaceSnapshot } from "./models";

afterEach(cleanup);

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
        },
      ],
    };
    const gateway = {
      loadSnapshot: async () => emptySnapshot,
      chooseAndAuthorizeRoot: async () => scannedSnapshot,
      rescanRoot: async () => scannedSnapshot,
      removeRoot: async () => emptySnapshot,
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
});
