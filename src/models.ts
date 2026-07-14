export type SkillStatus = "ready" | "needsRepair";

export interface SkillInstance {
  id: string;
  name: string;
  description: string;
  relativePath: string;
  skillFilePath: string;
  status: SkillStatus;
  error: string | null;
}

export interface WorkspaceSnapshot {
  authorizedRoot: string | null;
  instances: SkillInstance[];
}
