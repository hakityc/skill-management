export type SkillStatus = "ready" | "needsRepair";
export type SkillClient =
  | "claude"
  | "codex"
  | "gemini"
  | "openCode"
  | "hermes"
  | "other";
export type DuplicateCheckStatus = "none" | "exact" | "suspected" | "nameConflict";

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
  duplicateCheckStatus: DuplicateCheckStatus;
  createdAt: number;
  modifiedAt: number;
}

export interface SkillFilters {
  clients: SkillClient[];
  rootIds: number[];
  repairStatus: "any" | "ready" | "needsRepair";
  duplicateCheckStatuses: DuplicateCheckStatus[];
}

export type SkillSortField =
  | "name"
  | "modifiedAt"
  | "createdAt"
  | "root"
  | "duplicateCheckStatus";
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

export interface OrganizationSkillGroup {
  id: number;
  name: string;
  instanceIds: string[];
}

export interface SkillInstanceOrganization {
  instanceId: string;
  tags: string[];
  groupIds: number[];
}

export interface SkillOrganizationSnapshot {
  groups: OrganizationSkillGroup[];
  instances: SkillInstanceOrganization[];
}

export interface SkillOrganizationChange {
  instanceIds: string[];
  addTags: string[];
  removeTags: string[];
  addGroupIds: number[];
  removeGroupIds: number[];
}

export type SkillFileKind = "directory" | "text" | "binary" | "symbolicLink";

export interface SkillFileEntry {
  relativePath: string;
  kind: SkillFileKind;
  size: number;
  modifiedAt: number;
}

export interface SkillDetail {
  instance: SkillInstance;
  root: SkillRoot;
  tags: string[];
  skillGroups: string[];
  files: SkillFileEntry[];
  fileCount: number;
}

export type SkillFilePreview =
  | { kind: "text"; content: string }
  | {
      kind: "binary";
      size: number;
      mediaType: string | null;
      previewContent: number[] | null;
    };

export type SkillDraftTarget =
  | { kind: "existing"; instanceId: string }
  | { kind: "new"; rootId: number; relativePath: string };

export type SkillFileDraftOperation =
  | { kind: "writeText"; content: string }
  | { kind: "replaceBinary"; content: number[] }
  | { kind: "delete" };

export interface SkillFileDraftChange {
  relativePath: string;
  operation: SkillFileDraftOperation;
}

export interface SkillDraft {
  target: SkillDraftTarget;
  name: string;
  description: string;
  markdownBody: string;
  fileChanges: SkillFileDraftChange[];
}

export interface SkillValidationIssue {
  field: string;
  message: string;
}

export interface SkillDraftValidation {
  valid: boolean;
  issues: SkillValidationIssue[];
}

export type SkillChangeKind = "create" | "overwrite" | "delete";

export interface SkillPlannedChange {
  relativePath: string;
  kind: SkillChangeKind;
  binary: boolean;
  size: number;
}

export interface SkillChangePlan {
  id: number;
  changes: SkillPlannedChange[];
}

export interface SkillChangeOutcome {
  operationId: number;
  snapshot: WorkspaceSnapshot;
}

export interface SkillChangeRecord {
  operationId: number;
  targetDirectory: string;
  createdAt: number;
}

export type FileOperationKind = "import" | "copy" | "move" | "trash" | "merge";
export type FileConflictPolicy = "skip" | "overwrite";

export interface FileOperationRequest {
  instanceIds: string[];
  kind: Exclude<FileOperationKind, "import" | "merge">;
  targetRootId: number | null;
  conflictPolicy: FileConflictPolicy;
}

export interface ZipImportRequest {
  zipPath: string;
  targetRootId: number;
  relativePath: string;
  conflictPolicy: FileConflictPolicy;
}

export interface PlannedFileOperationItem {
  instanceId: string | null;
  source: string;
  target: string | null;
  conflict: boolean;
  willOverwrite: boolean;
  willRemoveSource: boolean;
  fileCount: number;
  totalSize: number;
  changes?: DuplicateFileDifference[];
}

export interface FileOperationPlan {
  id: number;
  kind: FileOperationKind;
  items: PlannedFileOperationItem[];
  undoable: boolean;
}

export type FileOperationResultStatus = "success" | "failed" | "skipped";

export interface FileOperationItemResult {
  instanceId: string | null;
  source: string;
  target: string | null;
  status: FileOperationResultStatus;
  message: string;
  backupCreated: boolean;
}

export interface FileOperationBatchOutcome {
  batchId: number;
  results: FileOperationItemResult[];
  snapshot: WorkspaceSnapshot;
}

export interface FileOperationRecord {
  batchId: number;
  planId: number;
  kind: FileOperationKind;
  createdAt: number;
  undoable: boolean;
  undone: boolean;
  plan: FileOperationPlan;
  results: FileOperationItemResult[];
}

export type DuplicateHitRule =
  | "exactContent"
  | "normalizedName"
  | "contentSimilarity";

export type DuplicateFileDifferenceStatus =
  | "identical"
  | "modified"
  | "onlyLeft"
  | "onlyRight";

export type DuplicateFileKind = "text" | "binary";
export type DuplicateFileNodeKind = "file" | "symbolicLink";
export type DuplicateTextDiffLineKind = "equal" | "modified" | "onlyLeft" | "onlyRight";
export type DuplicateDecisionKind = "notDuplicate" | "ignored";

export interface DuplicateTextDiffLine {
  kind: DuplicateTextDiffLineKind;
  leftLineNumber: number | null;
  rightLineNumber: number | null;
  left: string | null;
  right: string | null;
}

export interface DuplicateFileDifference {
  relativePath: string;
  status: DuplicateFileDifferenceStatus;
  kind: DuplicateFileKind;
  leftNodeKind?: DuplicateFileNodeKind | null;
  rightNodeKind?: DuplicateFileNodeKind | null;
  leftSize: number | null;
  rightSize: number | null;
  leftFingerprint: string | null;
  rightFingerprint: string | null;
  textDiff: DuplicateTextDiffLine[] | null;
  textDiffTruncated: boolean;
}

export interface DuplicateComparison {
  leftInstanceId: string;
  rightInstanceId: string;
  status: DuplicateCheckStatus;
  similarity: number;
  hitRules: DuplicateHitRule[];
  files: DuplicateFileDifference[];
}

export interface DuplicateReviewInstance {
  id: string;
  name: string;
  description: string;
  path: string;
  client: SkillClient;
}

export interface DuplicateGroup {
  id: string;
  name: string;
  status: DuplicateCheckStatus;
  similarity: number;
  hitRules: DuplicateHitRule[];
  fingerprintFiles: string[];
  instances: DuplicateReviewInstance[];
  comparisons: DuplicateComparison[];
}

export interface DuplicateReview {
  groups: DuplicateGroup[];
  suppressedCount: number;
}

export interface DuplicateDecisionRecord {
  id: number;
  instanceIds: string[];
  kind: DuplicateDecisionKind;
  createdAt: number;
}
