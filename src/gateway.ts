import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type { SkillGateway } from "./App";
import type {
  SkillQuery,
  SkillChangeOutcome,
  SkillChangePlan,
  SkillDetail,
  SkillDraft,
  SkillDraftValidation,
  SkillFilePreview,
  SkillSearchResult,
  SkillWorkspaceViewPreferences,
  WorkspaceSnapshot,
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
  };
}
