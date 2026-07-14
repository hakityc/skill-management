import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import type { SkillGateway } from "./App";
import type { WorkspaceSnapshot } from "./models";

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
  };
}
