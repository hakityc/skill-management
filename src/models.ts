export type SkillStatus = "ready" | "needsRepair";

export interface SkillInstance {
  id: string;
  rootId: number;
  name: string;
  description: string;
  relativePath: string;
  skillFilePath: string;
  linkPath: string | null;
  realPath: string;
  status: SkillStatus;
  error: string | null;
}

export type SkillRootStatus =
  | "ready"
  | "partialFailure"
  | "missing"
  | "permissionDenied";

export interface SkillRoot {
  id: number;
  path: string;
  status: SkillRootStatus;
  error: string | null;
  recoveryHint: string | null;
}

export interface WorkspaceSnapshot {
  authorizedRoot: string | null;
  roots: SkillRoot[];
  instances: SkillInstance[];
}
