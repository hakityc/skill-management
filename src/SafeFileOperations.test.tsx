import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test } from "vitest";

import type { SkillGateway } from "./App";
import { SafeFileOperations } from "./SafeFileOperations";
import type {
  FileOperationPlan,
  SkillInstance,
  WorkspaceSnapshot,
  ZipImportRequest,
} from "./models";

afterEach(cleanup);

const snapshot: WorkspaceSnapshot = {
  authorizedRoot: "/skills/source",
  roots: [
    { id: 1, path: "/skills/source", status: "ready", error: null, recoveryHint: null },
    { id: 2, path: "/skills/target", status: "ready", error: null, recoveryHint: null },
  ],
  instances: [instance("alpha"), instance("beta")],
};

const movePlan: FileOperationPlan = {
  id: 41,
  kind: "move",
  undoable: true,
  items: [
    {
      instanceId: "alpha",
      source: "/skills/source/alpha",
      target: "/skills/target/alpha",
      conflict: false,
      willOverwrite: false,
      willRemoveSource: true,
      fileCount: 2,
      totalSize: 2048,
    },
    {
      instanceId: "beta",
      source: "/skills/source/beta",
      target: "/skills/target/beta",
      conflict: true,
      willOverwrite: true,
      willRemoveSource: true,
      fileCount: 1,
      totalSize: 512,
    },
  ],
};

describe("安全文件操作", () => {
  test("批量移动先展示冲突与删除影响，再逐项报告成功和失败", async () => {
    let plannedRequest: unknown = null;
    let changedSnapshot: WorkspaceSnapshot | null = null;
    const cancelledPlans: number[] = [];
    const gateway = operationGateway({
      async planFileOperations(request) {
        plannedRequest = request;
        return movePlan;
      },
      async executeFileOperationPlan() {
        return {
          batchId: 51,
          snapshot,
          results: [
            {
              instanceId: "alpha",
              source: "/skills/source/alpha",
              target: "/skills/target/alpha",
              status: "success",
              message: "操作完成。",
              backupCreated: false,
            },
            {
              instanceId: "beta",
              source: "/skills/source/beta",
              target: "/skills/target/beta",
              status: "failed",
              message: "目标不可写，来源保持不变。",
              backupCreated: false,
            },
          ],
        };
      },
      async cancelFileOperationPlan(planId) {
        cancelledPlans.push(planId);
      },
    });

    render(
      <SafeFileOperations
        gateway={gateway}
        snapshot={snapshot}
        selectedInstances={snapshot.instances}
        initialMode="move"
        onClose={() => {}}
        onSnapshotChange={(next) => {
          changedSnapshot = next;
        }}
      />,
    );

    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "目标 Skill 根目录" }),
      "2",
    );
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "冲突处理" }),
      "overwrite",
    );
    await userEvent.click(screen.getByRole("button", { name: "预览影响" }));

    await waitFor(() =>
      expect(plannedRequest).toEqual({
        instanceIds: ["alpha", "beta"],
        kind: "move",
        targetRootId: 2,
        conflictPolicy: "overwrite",
      }),
    );
    expect(await screen.findByText("将覆盖目标并创建备份")).toBeTruthy();
    expect(screen.getAllByText("将移除来源")).toHaveLength(2);
    expect(screen.getByText("共 2 项 · 3 个文件 · 2.5 KB")).toBeTruthy();

    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "冲突处理" }),
      "skip",
    );
    expect(screen.queryByRole("button", { name: "确认执行" })).toBeNull();
    await waitFor(() => expect(cancelledPlans).toEqual([41]));
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "冲突处理" }),
      "overwrite",
    );
    await userEvent.click(screen.getByRole("button", { name: "预览影响" }));

    await userEvent.click(screen.getByRole("button", { name: "确认执行" }));
    expect(await screen.findByText("1 项成功，1 项失败，0 项跳过")).toBeTruthy();
    expect(screen.getByText("目标不可写，来源保持不变。")).toBeTruthy();
    expect(changedSnapshot).toEqual(snapshot);
  });

  test("ZIP 只在选择文件并确认目标路径后生成导入计划", async () => {
    let request: ZipImportRequest | null = null;
    let cancelledPlanId: number | null = null;
    const importPlan: FileOperationPlan = {
      id: 71,
      kind: "import",
      undoable: true,
      items: [
        {
          instanceId: null,
          source: "/tmp/import-staging/skill",
          target: "/skills/target/imported",
          conflict: false,
          willOverwrite: false,
          willRemoveSource: false,
          fileCount: 2,
          totalSize: 1024,
        },
      ],
    };
    const gateway = operationGateway({
      chooseZipFile: async () => "/Downloads/imported.zip",
      async previewZipImport(nextRequest) {
        request = nextRequest;
        return importPlan;
      },
      async cancelFileOperationPlan(planId) {
        cancelledPlanId = planId;
      },
    });

    render(
      <SafeFileOperations
        gateway={gateway}
        snapshot={snapshot}
        selectedInstances={[]}
        initialMode="import"
        onClose={() => {}}
        onSnapshotChange={() => {}}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "选择 ZIP 文件" }));
    expect(screen.getByText("/Downloads/imported.zip")).toBeTruthy();
    await userEvent.selectOptions(
      screen.getByRole("combobox", { name: "目标 Skill 根目录" }),
      "2",
    );
    await userEvent.clear(screen.getByRole("textbox", { name: "目标相对路径" }));
    await userEvent.type(
      screen.getByRole("textbox", { name: "目标相对路径" }),
      "imported",
    );
    await userEvent.click(screen.getByRole("button", { name: "预览导入" }));

    await waitFor(() =>
      expect(request).toEqual({
        zipPath: "/Downloads/imported.zip",
        targetRootId: 2,
        relativePath: "imported",
        conflictPolicy: "skip",
      }),
    );
    expect(await screen.findByText("确认执行前，不会写入 Skill 根目录")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "关闭" }));
    await waitFor(() => expect(cancelledPlanId).toBe(71));
  });
});

type OperationGateway = Pick<
  SkillGateway,
  | "chooseZipFile"
  | "planFileOperations"
  | "previewZipImport"
  | "executeFileOperationPlan"
  | "cancelFileOperationPlan"
  | "fileOperationHistory"
  | "latestUndoableFileOperation"
  | "undoFileOperationBatch"
>;

function operationGateway(overrides: Partial<OperationGateway>): OperationGateway {
  return {
    chooseZipFile: async () => null,
    planFileOperations: async () => movePlan,
    previewZipImport: async () => movePlan,
    executeFileOperationPlan: async () => ({ batchId: 1, results: [], snapshot }),
    cancelFileOperationPlan: async () => {},
    fileOperationHistory: async () => [],
    latestUndoableFileOperation: async () => null,
    undoFileOperationBatch: async () => snapshot,
    ...overrides,
  };
}

function instance(id: string): SkillInstance {
  return {
    id,
    rootId: 1,
    name: id,
    description: `${id} skill`,
    relativePath: id,
    skillFilePath: `/skills/source/${id}/SKILL.md`,
    linkPath: null,
    realPath: `/skills/source/${id}`,
    status: "ready",
    error: null,
    client: "codex",
    duplicateCheckStatus: "none",
    createdAt: 1,
    modifiedAt: 1,
  };
}
