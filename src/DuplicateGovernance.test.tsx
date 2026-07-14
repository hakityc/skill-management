import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test } from "vitest";

import type { SkillGateway } from "./App";
import { DuplicateGovernance } from "./DuplicateGovernance";
import type {
  DuplicateDecisionKind,
  DuplicateGroup,
  DuplicateReview,
  WorkspaceSnapshot,
} from "./models";

afterEach(cleanup);

describe("重复检查中心", () => {
  test("个人用户查看判定依据、逐文件差异并保存和恢复裁决", async () => {
    const snapshot: WorkspaceSnapshot = { authorizedRoot: null, roots: [], instances: [] };
    const suspected = duplicateGroup("suspected", "release-notes", 0.824);
    suspected.instances.push({
      id: "third-suspected",
      name: "release-notes-copy",
      description: "旧版本发布说明。",
      path: "/Users/me/.gemini/skills/release-notes-copy",
      client: "gemini",
    });
    suspected.comparisons.push({
      ...suspected.comparisons[0],
      rightInstanceId: "third-suspected",
      status: "nameConflict",
      similarity: 0.46,
      hitRules: ["normalizedName"],
    });
    const exact = duplicateGroup("exact", "api-review", 1);
    const conflict = duplicateGroup("nameConflict", "auth-helper", 0.46);
    let groups = [suspected, exact, conflict];
    let savedDecision: { ids: string[]; kind: DuplicateDecisionKind } | null = null;
    let restoredDecisionId: number | null = null;
    const review = (): DuplicateReview => ({ groups, suppressedCount: savedDecision ? 1 : 0 });
    const gateway: SkillGateway = {
      loadSnapshot: async () => snapshot,
      chooseAndAuthorizeRoot: async () => null,
      rescanRoot: async () => snapshot,
      removeRoot: async () => snapshot,
      searchSkills: async () => ({ instances: [], total: 0 }),
      loadViewPreferences: async () => ({
        filters: { clients: [], rootIds: [], repairStatus: "any", duplicateCheckStatuses: [] },
        sort: { field: "name", direction: "asc" },
        density: "compact",
      }),
      saveViewPreferences: async () => {},
      skillDetail: async () => { throw new Error("当前测试不读取详情"); },
      readSkillFile: async () => { throw new Error("当前测试不读取文件"); },
      validateSkillDraft: async () => ({ valid: true, issues: [] }),
      planSkillChange: async () => ({ id: 1, changes: [] }),
      executeSkillChange: async () => { throw new Error("当前测试不编辑"); },
      undoSkillChange: async () => { throw new Error("当前测试不撤销编辑"); },
      latestUndoableSkillChange: async () => null,
      reviewDuplicateGroups: async () => review(),
      async saveDuplicateDecision(ids, kind) {
        savedDecision = { ids, kind };
        groups = groups.filter((group) => !group.instances.every((instance) => ids.includes(instance.id)));
      },
      duplicateDecisions: async () => [
        { id: 17, instanceIds: ["old-a", "old-b"], kind: "ignored", createdAt: 10 },
      ],
      async restoreDuplicateDecision(decisionId) {
        restoredDecisionId = decisionId;
      },
      skillOrganization: async () => ({ groups: [], instances: [] }),
      createSkillGroup: async () => ({ groups: [], instances: [] }),
      renameSkillGroup: async () => ({ groups: [], instances: [] }),
      deleteSkillGroup: async () => ({ groups: [], instances: [] }),
      applySkillOrganizationChange: async () => ({ groups: [], instances: [] }),
      reorderSkillGroup: async () => ({ groups: [], instances: [] }),
      chooseZipFile: async () => null,
      planFileOperations: async () => ({ id: 1, kind: "copy", items: [], undoable: true }),
      previewZipImport: async () => ({ id: 1, kind: "import", items: [], undoable: true }),
      executeFileOperationPlan: async () => ({ batchId: 1, results: [], snapshot }),
      cancelFileOperationPlan: async () => {},
      fileOperationHistory: async () => [],
      latestUndoableFileOperation: async () => null,
      undoFileOperationBatch: async () => snapshot,
    };

    render(
      <DuplicateGovernance gateway={gateway} onBack={() => {}} onSnapshotChange={() => {}} />,
    );

    expect(await screen.findByRole("heading", { name: "把相似，变成确定。" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /待检查.*3/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /完全重复.*1/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /疑似重复.*1/ })).toBeTruthy();
    expect(screen.getByRole("button", { name: /同名冲突.*1/ })).toBeTruthy();
    expect(screen.getByText("内容相似度 ≥ 82%")).toBeTruthy();
    expect(screen.getByText("规范化名称匹配")).toBeTruthy();
    expect(screen.getByText("/Users/me/.codex/skills/release-notes")).toBeTruthy();
    expect(screen.getByText("/Users/me/.claude/skills/release-notes")).toBeTruthy();
    expect(screen.getByText("## 修复与升级")).toBeTruthy();
    const comparisonPicker = screen.getByRole("combobox", { name: "比较实例组合" });
    expect(comparisonPicker).toBeTruthy();
    await userEvent.selectOptions(comparisonPicker, "1");
    expect(screen.getByText("重复检查 / 同名冲突")).toBeTruthy();
    expect(screen.queryByText("内容相似度 ≥ 82%")).toBeNull();
    await userEvent.selectOptions(comparisonPicker, "0");
    await userEvent.click(screen.getByRole("button", { name: /preview\.png/ }));
    expect(screen.getByText("111")).toBeTruthy();
    expect(screen.getByText("222")).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "不是重复" }));
    expect(savedDecision).toEqual({
      ids: ["left-suspected", "right-suspected", "third-suspected"],
      kind: "notDuplicate",
    });
    expect(screen.queryByRole("heading", { name: "release-notes" })).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: /已忽略结果/ }));
    expect(await screen.findByRole("heading", { name: "已忽略结果" })).toBeTruthy();
    expect(screen.getByText("old-a ↔ old-b")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "恢复检查" }));
    expect(restoredDecisionId).toBe(17);
  });
});

function duplicateGroup(
  status: "exact" | "suspected" | "nameConflict",
  name: string,
  similarity: number,
): DuplicateGroup {
  const leftId = `left-${status}`;
  const rightId = `right-${status}`;
  return {
    id: status,
    name,
    status,
    similarity,
    hitRules:
      status === "exact"
        ? ["exactContent", "normalizedName"]
        : status === "suspected"
          ? ["normalizedName", "contentSimilarity"]
          : ["normalizedName"],
    fingerprintFiles: ["SKILL.md", "references/template.md"],
    instances: [
      {
        id: leftId,
        name,
        description: "整理版本发布说明。",
        path: `/Users/me/.codex/skills/${name}`,
        client: "codex",
      },
      {
        id: rightId,
        name,
        description: "整理并核对版本说明。",
        path: `/Users/me/.claude/skills/${name}`,
        client: "claude",
      },
    ],
    comparisons: [
      {
        leftInstanceId: leftId,
        rightInstanceId: rightId,
        status,
        similarity,
        hitRules: status === "suspected" ? ["normalizedName", "contentSimilarity"] : ["normalizedName"],
        files: [
          {
            relativePath: "SKILL.md",
            status: status === "exact" ? "identical" : "modified",
            kind: "text",
            leftSize: 100,
            rightSize: 108,
            leftFingerprint: "aaa",
            rightFingerprint: status === "exact" ? "aaa" : "bbb",
            textDiffTruncated: false,
            textDiff:
              status === "exact"
                ? null
                : [
                    {
                      kind: "modified",
                      leftLineNumber: 8,
                      rightLineNumber: 8,
                      left: "## 修复",
                      right: "## 修复与升级",
                    },
                  ],
          },
          {
            relativePath: "preview.png",
            status: "modified",
            kind: "binary",
            leftSize: 120,
            rightSize: 140,
            leftFingerprint: "111",
            rightFingerprint: "222",
            textDiffTruncated: false,
            textDiff: null,
          },
        ],
      },
    ],
  };
}
