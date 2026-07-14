export type SkillStatus = "ready" | "needsRepair";
export type SkillClient =
  | "claude"
  | "codex"
  | "gemini"
  | "openCode"
  | "hermes"
  | "other";
export type DuplicateStatus = "none" | "exact" | "suspected" | "nameConflict";

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
  client: SkillClient;
  duplicateStatus: DuplicateStatus;
  createdAt: number;
  modifiedAt: number;
}

export interface SkillFilters {
  clients: SkillClient[];
  rootIds: number[];
  needsRepair: boolean | null;
  duplicateStatuses: DuplicateStatus[];
}

export type SkillSortField =
  | "name"
  | "modifiedAt"
  | "createdAt"
  | "root"
  | "duplicateStatus";
export type SkillSortDirection = "asc" | "desc";

export interface SkillSort {
  field: SkillSortField;
  direction: SkillSortDirection;
}

export interface SkillQuery {
  text: string;
  filters: SkillFilters;
  sort: SkillSort;
}

export interface SkillSearchResult {
  instances: SkillInstance[];
  total: number;
}

export type SkillListDensity = "compact" | "comfortable";

export interface SkillWorkspaceViewPreferences {
  filters: SkillFilters;
  sort: SkillSort;
  density: SkillListDensity;
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
