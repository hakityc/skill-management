import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test } from "vitest";

import { SkillManagerApp } from "./App";
import type { WorkspaceSnapshot } from "./models";

describe("Skill 管理器", () => {
  test("个人用户从中文空状态选择根目录后看到合法与需要修复的 Skill", async () => {
    const emptySnapshot: WorkspaceSnapshot = {
      authorizedRoot: null,
      instances: [],
    };
    const scannedSnapshot: WorkspaceSnapshot = {
      authorizedRoot: "/Users/example/.agents/skills",
      instances: [
        {
          id: "api-review",
          name: "api-review",
          description: "审查 API 设计与接口边界。",
          relativePath: "api-review",
          skillFilePath: "/Users/example/.agents/skills/api-review/SKILL.md",
          status: "ready",
          error: null,
        },
        {
          id: "broken-skill",
          name: "broken-skill",
          description: "",
          relativePath: "broken-skill",
          skillFilePath: "/Users/example/.agents/skills/broken-skill/SKILL.md",
          status: "needsRepair",
          error: "YAML frontmatter 缺少 description",
        },
      ],
    };
    const gateway = {
      loadSnapshot: async () => emptySnapshot,
      chooseAndAuthorizeRoot: async () => scannedSnapshot,
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
    const repairRow = screen.getByRole("listitem", {
      name: "broken-skill，需要修复",
    });
    expect(within(repairRow).getByText("需要修复")).toBeTruthy();
    expect(
      within(repairRow).getByText("YAML frontmatter 缺少 description"),
    ).toBeTruthy();
    expect(screen.getByText("/Users/example/.agents/skills")).toBeTruthy();
  });
});
