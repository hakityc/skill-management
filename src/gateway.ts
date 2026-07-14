import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type { SkillGateway } from "./App";
import type {
  DuplicateDecisionKind,
  DuplicateDecisionRecord,
  DuplicateReview,
  FileOperationBatchOutcome,
  FileOperationPlan,
  FileOperationRecord,
  FileOperationRequest,
  SkillQuery,
  SkillChangeOutcome,
  SkillChangePlan,
  SkillChangeRecord,
  SkillDetail,
  SkillDraft,
  SkillDraftValidation,
  SkillFilePreview,
  SkillOrganizationChange,
  SkillOrganizationSnapshot,
  SkillSearchResult,
  SkillWorkspaceViewPreferences,
  WorkspaceSnapshot,
  ZipImportRequest,
} from "./models";

export function createTauriSkillGateway(): SkillGateway {
  return {
    loadSnapshot: () => invoke<WorkspaceSnapshot>("workspace_snapshot"),
    async chooseAndAuthorizeRoot() {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择 Skill 根目录",
      });
      if (!selected || Array.isArray(selected)) return null;
      return invoke<WorkspaceSnapshot>("authorize_skill_root", {
        path: selected,
      });
    },
    rescanRoot: (rootId) =>
      invoke<WorkspaceSnapshot>("rescan_skill_root", { rootId }),
    removeRoot: (rootId) =>
      invoke<WorkspaceSnapshot>("remove_skill_root", { rootId }),
    searchSkills: (query: SkillQuery) =>
      invoke<SkillSearchResult>("search_skills", { query }),
    loadViewPreferences: () =>
      invoke<SkillWorkspaceViewPreferences>("load_view_preferences"),
    saveViewPreferences: (preferences: SkillWorkspaceViewPreferences) =>
      invoke<void>("save_view_preferences", { preferences }),
    skillDetail: (instanceId) => invoke<SkillDetail>("skill_detail", { instanceId }),
    readSkillFile: (instanceId, relativePath) =>
      invoke<SkillFilePreview>("read_skill_file", { instanceId, relativePath }),
    validateSkillDraft: (draft: SkillDraft) =>
      invoke<SkillDraftValidation>("validate_skill_draft", { draft }),
    planSkillChange: (draft: SkillDraft) =>
      invoke<SkillChangePlan>("plan_skill_change", { draft }),
    executeSkillChange: (planId) =>
      invoke<SkillChangeOutcome>("execute_skill_change", { planId }),
    undoSkillChange: (operationId) =>
      invoke<SkillChangeOutcome>("undo_skill_change", { operationId }),
    latestUndoableSkillChange: () =>
      invoke<SkillChangeRecord | null>("latest_undoable_skill_change"),
    reviewDuplicateGroups: () => invoke<DuplicateReview>("review_duplicate_groups"),
    saveDuplicateDecision: (instanceIds, kind: DuplicateDecisionKind) =>
      invoke<void>("save_duplicate_decision", { instanceIds, kind }),
    duplicateDecisions: () =>
      invoke<DuplicateDecisionRecord[]>("duplicate_decisions"),
    restoreDuplicateDecision: (decisionId) =>
      invoke<void>("restore_duplicate_decision", { decisionId }),
    planDuplicateMerge: (masterInstanceId, targetInstanceIds) =>
      invoke<FileOperationPlan>("plan_duplicate_merge", {
        masterInstanceId,
        targetInstanceIds,
      }),
    skillOrganization: () =>
      invoke<SkillOrganizationSnapshot>("skill_organization"),
    createSkillGroup: (name) =>
      invoke<SkillOrganizationSnapshot>("create_skill_group", { name }),
    renameSkillGroup: (groupId, name) =>
      invoke<SkillOrganizationSnapshot>("rename_skill_group", { groupId, name }),
    deleteSkillGroup: (groupId) =>
      invoke<SkillOrganizationSnapshot>("delete_skill_group", { groupId }),
    applySkillOrganizationChange: (change: SkillOrganizationChange) =>
      invoke<SkillOrganizationSnapshot>("apply_skill_organization_change", { change }),
    reorderSkillGroup: (groupId, orderedInstanceIds) =>
      invoke<SkillOrganizationSnapshot>("reorder_skill_group", {
        groupId,
        orderedInstanceIds,
      }),
    async chooseZipFile() {
      const selected = await open({
        multiple: false,
        title: "选择要导入的 Skill ZIP",
        filters: [{ name: "ZIP 压缩包", extensions: ["zip"] }],
      });
      return !selected || Array.isArray(selected) ? null : selected;
    },
    planFileOperations: (request: FileOperationRequest) =>
      invoke<FileOperationPlan>("plan_file_operations", { request }),
    previewZipImport: (request: ZipImportRequest) =>
      invoke<FileOperationPlan>("preview_zip_import", { request }),
    executeFileOperationPlan: (planId) =>
      invoke<FileOperationBatchOutcome>("execute_file_operation_plan", { planId }),
    cancelFileOperationPlan: (planId) =>
      invoke<void>("cancel_file_operation_plan", { planId }),
    fileOperationHistory: () =>
      invoke<FileOperationRecord[]>("file_operation_history"),
    latestUndoableFileOperation: () =>
      invoke<FileOperationRecord | null>("latest_undoable_file_operation"),
    undoFileOperationBatch: (batchId) =>
      invoke<WorkspaceSnapshot>("undo_file_operation_batch", { batchId }),
  };
}
