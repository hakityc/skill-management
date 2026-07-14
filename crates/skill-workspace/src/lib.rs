//! 本地 Skill 管理的最高层应用接缝。

mod detail;
mod duplicate;
mod edit;
mod file_operations;
mod organization;

pub use detail::{SkillDetail, SkillFileEntry, SkillFileKind, SkillFilePreview};
pub use duplicate::{
    DUPLICATE_SIMILARITY_THRESHOLD, DuplicateComparison, DuplicateDecisionKind,
    DuplicateDecisionRecord, DuplicateFileDifference, DuplicateFileDifferenceStatus,
    DuplicateFileKind, DuplicateFileNodeKind, DuplicateGroup, DuplicateHitRule, DuplicateReview,
    DuplicateReviewInstance, DuplicateTextDiffLine, DuplicateTextDiffLineKind,
};
pub use edit::{
    SkillChangeKind, SkillChangeOutcome, SkillChangePlan, SkillChangeRecord, SkillDraft,
    SkillDraftTarget, SkillDraftValidation, SkillFileDraftChange, SkillFileDraftOperation,
    SkillPlannedChange, SkillValidationIssue,
};
pub use file_operations::{
    FileConflictPolicy, FileOperationBatchOutcome, FileOperationItemResult, FileOperationKind,
    FileOperationPlan, FileOperationRecord, FileOperationRequest, FileOperationResultStatus,
    PlannedFileOperationItem, ZipImportRequest,
};
pub use organization::{
    OrganizationSkillGroup, SkillInstanceOrganization, SkillOrganizationChange,
    SkillOrganizationSnapshot,
};

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("无法访问 Skill 根目录：{0}")]
    InvalidRoot(String),
    #[error("读取本地文件失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("初始化本地索引失败：{0}")]
    Database(#[from] rusqlite::Error),
    #[error("读取视图偏好失败：{0}")]
    Preferences(#[from] serde_json::Error),
    #[error("生成 SKILL.md 元数据失败：{0}")]
    DraftMetadata(#[from] serde_yaml::Error),
    #[error("找不到 Skill 实例：{0}")]
    UnknownInstance(String),
    #[error("找不到 Skill 组：{0}")]
    UnknownSkillGroup(i64),
    #[error("Skill 整理操作无效：{0}")]
    InvalidOrganization(String),
    #[error("无法访问 Skill 文件：{0}")]
    InvalidSkillPath(String),
    #[error("Skill 草稿未通过校验：{0}")]
    InvalidDraft(String),
    #[error("找不到变化计划：{0}")]
    UnknownChangePlan(i64),
    #[error("变化计划已过期：真实文件在预览后发生了变化，请重新预览")]
    StaleChangePlan,
    #[error("找不到编辑操作记录：{0}")]
    UnknownChangeOperation(i64),
    #[error("该编辑操作已经撤销")]
    ChangeAlreadyUndone,
    #[error("文件操作无效：{0}")]
    InvalidFileOperation(String),
    #[error("ZIP 导入无效：{0}")]
    InvalidArchive(String),
    #[error("找不到文件操作计划：{0}")]
    UnknownFileOperationPlan(i64),
    #[error("找不到文件操作记录：{0}")]
    UnknownFileOperationBatch(i64),
    #[error("文件操作计划已过期：{0}")]
    StaleFileOperationPlan(String),
    #[error("该文件操作已经撤销")]
    FileOperationAlreadyUndone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillStatus {
    Ready,
    NeedsRepair,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillClient {
    Claude,
    Codex,
    Gemini,
    OpenCode,
    Hermes,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateCheckStatus {
    None,
    Exact,
    Suspected,
    NameConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillRootStatus {
    Ready,
    PartialFailure,
    Missing,
    PermissionDenied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstance {
    pub id: String,
    pub root_id: i64,
    pub name: String,
    pub description: String,
    pub relative_path: String,
    pub skill_file_path: String,
    pub link_path: Option<String>,
    pub real_path: String,
    pub status: SkillStatus,
    pub error: Option<String>,
    pub client: SkillClient,
    pub duplicate_check_status: DuplicateCheckStatus,
    pub created_at: i64,
    pub modified_at: i64,
    #[serde(skip)]
    pub search_document: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillQuery {
    pub text: String,
    pub filters: SkillFilters,
    pub sort: SkillSort,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFilters {
    pub clients: Vec<SkillClient>,
    pub root_ids: Vec<i64>,
    pub repair_status: SkillRepairFilter,
    pub duplicate_check_statuses: Vec<DuplicateCheckStatus>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillRepairFilter {
    #[default]
    Any,
    Ready,
    NeedsRepair,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillSortField {
    #[default]
    Name,
    ModifiedAt,
    CreatedAt,
    Root,
    DuplicateCheckStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillSortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSort {
    pub field: SkillSortField,
    pub direction: SkillSortDirection,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillListDensity {
    #[default]
    Compact,
    Comfortable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillWorkspaceViewPreferences {
    pub filters: SkillFilters,
    pub sort: SkillSort,
    pub density: SkillListDensity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSearchResult {
    pub instances: Vec<SkillInstance>,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCheckStatusUpdate {
    pub instance_id: String,
    pub status: DuplicateCheckStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillTagsAndGroupsUpdate {
    pub instance_id: String,
    pub tags: Vec<String>,
    pub skill_groups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRoot {
    pub id: i64,
    pub path: String,
    pub status: SkillRootStatus,
    pub error: Option<String>,
    pub recovery_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub authorized_root: Option<String>,
    pub roots: Vec<SkillRoot>,
    pub instances: Vec<SkillInstance>,
}

#[derive(Debug, Clone)]
pub struct SkillWorkspace {
    database_path: PathBuf,
    trash_directory: Option<PathBuf>,
}

impl SkillWorkspace {
    pub fn open(database_path: impl Into<PathBuf>) -> Result<Self, WorkspaceError> {
        let database_path = database_path.into();
        if let Some(parent) = database_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(&database_path)?;
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS workspace_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skill_roots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL DEFAULT 'ready',
                error TEXT,
                recovery_hint TEXT
            );
            CREATE TABLE IF NOT EXISTS skill_instances (
                id TEXT PRIMARY KEY,
                root_id INTEGER NOT NULL DEFAULT 0,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                skill_file_path TEXT NOT NULL,
                link_path TEXT,
                real_path TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL,
                error TEXT,
                client TEXT NOT NULL DEFAULT 'other',
                duplicate_check_status TEXT NOT NULL DEFAULT 'none',
                created_at INTEGER NOT NULL DEFAULT 0,
                modified_at INTEGER NOT NULL DEFAULT 0,
                search_document TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS skill_tags_and_groups (
                instance_id TEXT PRIMARY KEY,
                tags TEXT NOT NULL DEFAULT '',
                skill_groups TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS skill_change_plans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                payload TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS skill_change_operations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                plan_id INTEGER NOT NULL,
                root_id INTEGER NOT NULL,
                target_directory TEXT NOT NULL,
                backup_directory TEXT,
                was_new INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                undone INTEGER NOT NULL DEFAULT 0,
                completed INTEGER NOT NULL DEFAULT 0,
                undoing INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS duplicate_decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_key TEXT NOT NULL UNIQUE,
                instance_ids TEXT NOT NULL,
                kind TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            ",
        )?;
        migrate_workspace_index(&connection)?;
        organization::initialize_organization(&connection)?;
        file_operations::initialize_file_operations(&connection)?;
        drop(connection);
        let workspace = Self {
            database_path,
            trash_directory: None,
        };
        workspace.recover_interrupted_changes()?;
        workspace.cleanup_abandoned_file_operation_plans()?;
        workspace.recover_interrupted_file_operations()?;
        workspace.cleanup_orphan_file_operation_backups()?;
        Ok(workspace)
    }

    pub fn open_with_trash_directory(
        database_path: impl Into<PathBuf>,
        trash_directory: impl Into<PathBuf>,
    ) -> Result<Self, WorkspaceError> {
        let mut workspace = Self::open(database_path)?;
        workspace.trash_directory = Some(trash_directory.into());
        Ok(workspace)
    }

    pub fn add_root(&self, root: impl AsRef<Path>) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let requested_root = root.as_ref();
        let root = match requested_root.canonicalize() {
            Ok(root) => root,
            Err(_) if requested_root.is_absolute() => requested_root.to_path_buf(),
            Err(_) => std::env::current_dir()?.join(requested_root),
        };

        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO skill_roots (path) VALUES (?1) ON CONFLICT(path) DO NOTHING",
            [root.to_string_lossy().as_ref()],
        )?;
        let root_id = transaction.query_row(
            "SELECT id FROM skill_roots WHERE path = ?1",
            [root.to_string_lossy().as_ref()],
            |row| row.get(0),
        )?;
        transaction.execute(
            "
            INSERT INTO workspace_settings (key, value)
            VALUES ('authorized_root', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            ",
            [root.to_string_lossy().as_ref()],
        )?;
        transaction.commit()?;
        self.rescan_root(root_id)
    }

    pub fn remove_root(&self, root_id: i64) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "
            DELETE FROM skill_tags_and_groups
            WHERE instance_id IN (SELECT id FROM skill_instances WHERE root_id = ?1)
            ",
            [root_id],
        )?;
        delete_search_documents_for_root(&transaction, root_id)?;
        transaction.execute("DELETE FROM skill_instances WHERE root_id = ?1", [root_id])?;
        organization::prune_orphaned_organization_records(&transaction)?;
        transaction.execute("DELETE FROM skill_roots WHERE id = ?1", [root_id])?;
        let replacement_root = transaction
            .query_row(
                "SELECT path FROM skill_roots ORDER BY id LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(path) = replacement_root {
            transaction.execute(
                "UPDATE workspace_settings SET value = ?1 WHERE key = 'authorized_root'",
                [path],
            )?;
        } else {
            transaction.execute(
                "DELETE FROM workspace_settings WHERE key = 'authorized_root'",
                [],
            )?;
        }
        transaction.commit()?;
        self.snapshot()
    }

    pub fn rescan_root(&self, root_id: i64) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let path = connection
            .query_row(
                "SELECT path FROM skill_roots WHERE id = ?1",
                [root_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| WorkspaceError::InvalidRoot("找不到要重新扫描的根目录".to_owned()))?;
        drop(connection);

        let outcome = scan_root(root_id, Path::new(&path));
        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction()?;
        if matches!(
            outcome.status,
            SkillRootStatus::Ready | SkillRootStatus::PartialFailure
        ) {
            delete_search_documents_for_root(&transaction, root_id)?;
            transaction.execute("DELETE FROM skill_instances WHERE root_id = ?1", [root_id])?;
            persist_instances(&transaction, &outcome.instances)?;
            organization::prune_orphaned_organization_records(&transaction)?;
        }
        transaction.execute(
            "
            UPDATE skill_roots
            SET status = ?1, error = ?2, recovery_hint = ?3
            WHERE id = ?4
            ",
            params![
                outcome.status.as_database(),
                outcome.error,
                outcome.recovery_hint,
                root_id,
            ],
        )?;
        transaction.commit()?;
        self.snapshot()
    }

    pub fn rescan_all_roots(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let root_ids = {
            let mut statement = connection.prepare("SELECT id FROM skill_roots ORDER BY id")?;
            statement
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        drop(connection);
        for root_id in root_ids {
            self.rescan_root(root_id)?;
        }
        self.snapshot()
    }

    pub fn snapshot(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let authorized_root = connection
            .query_row(
                "SELECT value FROM workspace_settings WHERE key = 'authorized_root'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let mut root_statement = connection.prepare(
            "SELECT id, path, status, error, recovery_hint FROM skill_roots ORDER BY id",
        )?;
        let roots = root_statement
            .query_map([], |row| {
                let status: String = row.get(2)?;
                Ok(SkillRoot {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    status: SkillRootStatus::from_database(&status),
                    error: row.get(3)?,
                    recovery_hint: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let instances = query_instances(
            &connection,
            "
            SELECT * FROM skill_instance_catalog
            ORDER BY root_id, relative_path
            ",
            [],
        )?;

        Ok(WorkspaceSnapshot {
            authorized_root,
            roots,
            instances,
        })
    }

    pub fn search_skills(&self, query: &SkillQuery) -> Result<SkillSearchResult, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let text = query.text.trim();
        let mut instances = if text.is_empty() {
            query_instances(
                &connection,
                "
                SELECT * FROM skill_instance_catalog
                ORDER BY name COLLATE NOCASE, relative_path
                ",
                [],
            )?
        } else if text.chars().count() < 3 {
            let pattern = literal_like_pattern(text);
            query_instances(
                &connection,
                "
                SELECT * FROM skill_instance_catalog
                WHERE name LIKE ?1 ESCAPE '\\'
                   OR description LIKE ?1 ESCAPE '\\'
                   OR search_document LIKE ?1 ESCAPE '\\'
                   OR relative_path LIKE ?1 ESCAPE '\\'
                   OR skill_file_path LIKE ?1 ESCAPE '\\'
                   OR real_path LIKE ?1 ESCAPE '\\'
                   OR id IN (
                       SELECT instance_id FROM skill_tags_and_groups
                       WHERE tags LIKE ?1 ESCAPE '\\'
                          OR skill_groups LIKE ?1 ESCAPE '\\'
                   )
                ",
                [pattern],
            )?
        } else {
            let search_expression = quoted_search_expression(text);
            query_instances(
                &connection,
                "
                SELECT skill_instance_catalog.*
                FROM skill_instance_catalog
                JOIN skill_search ON skill_search.instance_id = skill_instance_catalog.id
                WHERE skill_search MATCH ?1
                ORDER BY rank, skill_instance_catalog.name COLLATE NOCASE,
                         skill_instance_catalog.relative_path
                ",
                [search_expression],
            )?
        };
        instances.retain(|skill| query.filters.matches(skill));
        let root_paths = root_paths(&connection)?;
        instances.sort_by(|left, right| query.sort.compare(left, right, &root_paths));
        let total = instances.len();
        instances.shrink_to_fit();
        Ok(SkillSearchResult { instances, total })
    }

    pub fn load_view_preferences(&self) -> Result<SkillWorkspaceViewPreferences, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let serialized = connection
            .query_row(
                "SELECT value FROM workspace_settings WHERE key = 'view_preferences'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        serialized
            .map(|serialized| serde_json::from_str(&serialized))
            .transpose()
            .map(|preferences| preferences.unwrap_or_default())
            .map_err(WorkspaceError::from)
    }

    pub fn save_view_preferences(
        &self,
        preferences: &SkillWorkspaceViewPreferences,
    ) -> Result<(), WorkspaceError> {
        let serialized = serde_json::to_string(preferences)?;
        let connection = Connection::open(&self.database_path)?;
        connection.execute(
            "
            INSERT INTO workspace_settings (key, value)
            VALUES ('view_preferences', ?1)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            ",
            [serialized],
        )?;
        Ok(())
    }

    pub fn save_duplicate_check_statuses(
        &self,
        updates: &[DuplicateCheckStatusUpdate],
    ) -> Result<(), WorkspaceError> {
        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction()?;
        for update in updates {
            let changed = transaction.execute(
                "
                UPDATE skill_instances
                SET duplicate_check_status = ?1
                WHERE id = ?2
                ",
                params![update.status.as_database(), update.instance_id],
            )?;
            if changed == 0 {
                return Err(WorkspaceError::UnknownInstance(update.instance_id.clone()));
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn save_skill_tags_and_groups(
        &self,
        updates: &[SkillTagsAndGroupsUpdate],
    ) -> Result<(), WorkspaceError> {
        organization::replace_legacy_organization(self, updates)
    }
}

impl SkillStatus {
    fn as_database(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NeedsRepair => "needs_repair",
        }
    }

    fn from_database(value: &str) -> Self {
        match value {
            "ready" => Self::Ready,
            _ => Self::NeedsRepair,
        }
    }
}

impl SkillClient {
    fn as_database(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::OpenCode => "open_code",
            Self::Hermes => "hermes",
            Self::Other => "other",
        }
    }

    fn from_database(value: &str) -> Self {
        match value {
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            "gemini" => Self::Gemini,
            "open_code" => Self::OpenCode,
            "hermes" => Self::Hermes,
            _ => Self::Other,
        }
    }
}

impl DuplicateCheckStatus {
    fn as_database(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Exact => "exact",
            Self::Suspected => "suspected",
            Self::NameConflict => "name_conflict",
        }
    }

    fn from_database(value: &str) -> Self {
        match value {
            "exact" => Self::Exact,
            "suspected" => Self::Suspected,
            "name_conflict" => Self::NameConflict,
            _ => Self::None,
        }
    }
}

impl SkillFilters {
    fn matches(&self, skill: &SkillInstance) -> bool {
        (self.clients.is_empty() || self.clients.contains(&skill.client))
            && (self.root_ids.is_empty() || self.root_ids.contains(&skill.root_id))
            && match self.repair_status {
                SkillRepairFilter::Any => true,
                SkillRepairFilter::Ready => matches!(skill.status, SkillStatus::Ready),
                SkillRepairFilter::NeedsRepair => {
                    matches!(skill.status, SkillStatus::NeedsRepair)
                }
            }
            && (self.duplicate_check_statuses.is_empty()
                || self
                    .duplicate_check_statuses
                    .contains(&skill.duplicate_check_status))
    }
}

impl SkillSort {
    fn compare(
        &self,
        left: &SkillInstance,
        right: &SkillInstance,
        root_paths: &HashMap<i64, String>,
    ) -> Ordering {
        let ordering = match self.field {
            SkillSortField::Name => normalized_name(left).cmp(&normalized_name(right)),
            SkillSortField::ModifiedAt => left.modified_at.cmp(&right.modified_at),
            SkillSortField::CreatedAt => left.created_at.cmp(&right.created_at),
            SkillSortField::Root => root_paths
                .get(&left.root_id)
                .cmp(&root_paths.get(&right.root_id)),
            SkillSortField::DuplicateCheckStatus => duplicate_rank(&left.duplicate_check_status)
                .cmp(&duplicate_rank(&right.duplicate_check_status)),
        };
        let ordering = match self.direction {
            SkillSortDirection::Asc => ordering,
            SkillSortDirection::Desc => ordering.reverse(),
        };
        ordering
            .then_with(|| normalized_name(left).cmp(&normalized_name(right)))
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    }
}

fn normalized_name(skill: &SkillInstance) -> String {
    skill.name.to_lowercase()
}

fn duplicate_rank(status: &DuplicateCheckStatus) -> u8 {
    match status {
        DuplicateCheckStatus::Exact => 0,
        DuplicateCheckStatus::Suspected => 1,
        DuplicateCheckStatus::NameConflict => 2,
        DuplicateCheckStatus::None => 3,
    }
}

impl SkillRootStatus {
    fn as_database(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::PartialFailure => "partial_failure",
            Self::Missing => "missing",
            Self::PermissionDenied => "permission_denied",
        }
    }

    fn from_database(value: &str) -> Self {
        match value {
            "partial_failure" => Self::PartialFailure,
            "missing" => Self::Missing,
            "permission_denied" => Self::PermissionDenied,
            _ => Self::Ready,
        }
    }
}

fn migrate_workspace_index(connection: &Connection) -> Result<(), WorkspaceError> {
    let columns = {
        let mut statement = connection.prepare("PRAGMA table_info(skill_instances)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
    };
    if !columns.iter().any(|column| column == "root_id") {
        connection.execute(
            "ALTER TABLE skill_instances ADD COLUMN root_id INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !columns.iter().any(|column| column == "link_path") {
        connection.execute("ALTER TABLE skill_instances ADD COLUMN link_path TEXT", [])?;
    }
    if !columns.iter().any(|column| column == "real_path") {
        connection.execute(
            "ALTER TABLE skill_instances ADD COLUMN real_path TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    if !columns.iter().any(|column| column == "search_document") {
        connection.execute(
            "ALTER TABLE skill_instances ADD COLUMN search_document TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    if !columns.iter().any(|column| column == "client") {
        connection.execute(
            "ALTER TABLE skill_instances ADD COLUMN client TEXT NOT NULL DEFAULT 'other'",
            [],
        )?;
    }
    if !columns
        .iter()
        .any(|column| column == "duplicate_check_status")
    {
        connection.execute(
            "ALTER TABLE skill_instances ADD COLUMN duplicate_check_status TEXT NOT NULL DEFAULT 'none'",
            [],
        )?;
    }
    if !columns.iter().any(|column| column == "created_at") {
        connection.execute(
            "ALTER TABLE skill_instances ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !columns.iter().any(|column| column == "modified_at") {
        connection.execute(
            "ALTER TABLE skill_instances ADD COLUMN modified_at INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    connection.execute_batch(
        "
        DROP VIEW IF EXISTS skill_instance_catalog;
        CREATE VIEW skill_instance_catalog AS
        SELECT id, root_id, name, description, relative_path, skill_file_path,
               link_path, real_path, status, error, client, duplicate_check_status,
               created_at, modified_at, search_document
        FROM skill_instances;
        ",
    )?;

    let root_columns = {
        let mut statement = connection.prepare("PRAGMA table_info(skill_roots)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
    };
    if !root_columns.iter().any(|column| column == "status") {
        connection.execute(
            "ALTER TABLE skill_roots ADD COLUMN status TEXT NOT NULL DEFAULT 'ready'",
            [],
        )?;
    }
    if !root_columns.iter().any(|column| column == "error") {
        connection.execute("ALTER TABLE skill_roots ADD COLUMN error TEXT", [])?;
    }
    if !root_columns.iter().any(|column| column == "recovery_hint") {
        connection.execute("ALTER TABLE skill_roots ADD COLUMN recovery_hint TEXT", [])?;
    }

    let operation_columns = {
        let mut statement = connection.prepare("PRAGMA table_info(skill_change_operations)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
    };
    if !operation_columns.iter().any(|column| column == "completed") {
        connection.execute(
            "ALTER TABLE skill_change_operations ADD COLUMN completed INTEGER NOT NULL DEFAULT 1",
            [],
        )?;
    }
    if !operation_columns.iter().any(|column| column == "undoing") {
        connection.execute(
            "ALTER TABLE skill_change_operations ADD COLUMN undoing INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    let legacy_root = connection
        .query_row(
            "SELECT value FROM workspace_settings WHERE key = 'authorized_root'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if let Some(path) = legacy_root {
        connection.execute(
            "INSERT INTO skill_roots (path) VALUES (?1) ON CONFLICT(path) DO NOTHING",
            [&path],
        )?;
        let root_id = connection.query_row(
            "SELECT id FROM skill_roots WHERE path = ?1",
            [&path],
            |row| row.get::<_, i64>(0),
        )?;
        connection.execute(
            "
            UPDATE skill_instances
            SET root_id = ?1,
                id = CAST(?1 AS TEXT) || ':' || relative_path
            WHERE root_id = 0
            ",
            [root_id],
        )?;
    }

    let search_columns = {
        let mut statement = connection.prepare("PRAGMA table_info(skill_search)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
    };
    if !search_columns.is_empty()
        && (!search_columns.iter().any(|column| column == "tags")
            || !search_columns.iter().any(|column| column == "skill_groups"))
    {
        connection.execute("DROP TABLE skill_search", [])?;
    }

    connection.execute_batch(
        "
        CREATE VIRTUAL TABLE IF NOT EXISTS skill_search USING fts5(
            instance_id UNINDEXED,
            name,
            description,
            body,
            path,
            tags,
            skill_groups,
            tokenize = 'trigram'
        );
        DELETE FROM skill_search;
        INSERT INTO skill_search (
            instance_id, name, description, body, path, tags, skill_groups
        )
        SELECT skill_instances.id, name, description, search_document,
               relative_path || ' ' || skill_file_path || ' ' || real_path,
               COALESCE(skill_tags_and_groups.tags, ''),
               COALESCE(skill_tags_and_groups.skill_groups, '')
        FROM skill_instances
        LEFT JOIN skill_tags_and_groups
          ON skill_tags_and_groups.instance_id = skill_instances.id;
        ",
    )?;
    Ok(())
}

fn persist_instances(
    transaction: &rusqlite::Transaction<'_>,
    instances: &[SkillInstance],
) -> Result<(), WorkspaceError> {
    for skill in instances {
        transaction.execute(
            "
            INSERT INTO skill_instances (
                id, root_id, name, description, relative_path, skill_file_path,
                link_path, real_path, status, error, client, duplicate_check_status,
                created_at, modified_at, search_document
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
            )
            ",
            params![
                skill.id,
                skill.root_id,
                skill.name,
                skill.description,
                skill.relative_path,
                skill.skill_file_path,
                skill.link_path,
                skill.real_path,
                skill.status.as_database(),
                skill.error,
                skill.client.as_database(),
                skill.duplicate_check_status.as_database(),
                skill.created_at,
                skill.modified_at,
                skill.search_document,
            ],
        )?;
        transaction.execute(
            "
            INSERT INTO skill_search (
                instance_id, name, description, body, path, tags, skill_groups
            )
            VALUES (
                ?1, ?2, ?3, ?4, ?5,
                COALESCE((
                    SELECT tags FROM skill_tags_and_groups WHERE instance_id = ?1
                ), ''),
                COALESCE((
                    SELECT skill_groups FROM skill_tags_and_groups WHERE instance_id = ?1
                ), '')
            )
            ",
            params![
                skill.id,
                skill.name,
                skill.description,
                skill.search_document,
                format!(
                    "{} {} {}",
                    skill.relative_path, skill.skill_file_path, skill.real_path
                ),
            ],
        )?;
    }
    Ok(())
}

fn delete_search_documents_for_root(
    transaction: &rusqlite::Transaction<'_>,
    root_id: i64,
) -> Result<(), WorkspaceError> {
    transaction.execute(
        "
        DELETE FROM skill_search
        WHERE instance_id IN (SELECT id FROM skill_instances WHERE root_id = ?1)
        ",
        [root_id],
    )?;
    Ok(())
}

fn rebuild_search_document(
    transaction: &rusqlite::Transaction<'_>,
    instance_id: &str,
) -> Result<(), WorkspaceError> {
    transaction.execute(
        "DELETE FROM skill_search WHERE instance_id = ?1",
        [instance_id],
    )?;
    transaction.execute(
        "
        INSERT INTO skill_search (
            instance_id, name, description, body, path, tags, skill_groups
        )
        SELECT skill_instances.id, name, description, search_document,
               relative_path || ' ' || skill_file_path || ' ' || real_path,
               COALESCE(skill_tags_and_groups.tags, ''),
               COALESCE(skill_tags_and_groups.skill_groups, '')
        FROM skill_instances
        LEFT JOIN skill_tags_and_groups
          ON skill_tags_and_groups.instance_id = skill_instances.id
        WHERE skill_instances.id = ?1
        ",
        [instance_id],
    )?;
    Ok(())
}

fn quoted_search_expression(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

fn literal_like_pattern(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn query_instances<P>(
    connection: &Connection,
    sql: &str,
    parameters: P,
) -> Result<Vec<SkillInstance>, WorkspaceError>
where
    P: rusqlite::Params,
{
    let mut statement = connection.prepare(sql)?;
    let instances = statement
        .query_map(parameters, |row| {
            let status: String = row.get(8)?;
            Ok(SkillInstance {
                id: row.get(0)?,
                root_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                relative_path: row.get(4)?,
                skill_file_path: row.get(5)?,
                link_path: row.get(6)?,
                real_path: row.get(7)?,
                status: SkillStatus::from_database(&status),
                error: row.get(9)?,
                client: SkillClient::from_database(&row.get::<_, String>(10)?),
                duplicate_check_status: DuplicateCheckStatus::from_database(
                    &row.get::<_, String>(11)?,
                ),
                created_at: row.get(12)?,
                modified_at: row.get(13)?,
                search_document: row.get(14)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(instances)
}

fn root_paths(connection: &Connection) -> Result<HashMap<i64, String>, WorkspaceError> {
    let mut statement = connection.prepare("SELECT id, path FROM skill_roots")?;
    let roots = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(roots)
}

struct ScanOutcome {
    instances: Vec<SkillInstance>,
    status: SkillRootStatus,
    error: Option<String>,
    recovery_hint: Option<String>,
}

fn scan_root(root_id: i64, root: &Path) -> ScanOutcome {
    match fs::metadata(root) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return unavailable_root(
                SkillRootStatus::Missing,
                format!("Skill 根目录已不是文件夹：{}", root.display()),
                "重新选择一个有效的 Skill 根目录。",
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return unavailable_root(
                SkillRootStatus::Missing,
                format!("Skill 根目录不存在：{}", root.display()),
                "确认目录未被移动或删除，然后重新添加该目录。",
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return unavailable_root(
                SkillRootStatus::PermissionDenied,
                format!("没有权限访问 Skill 根目录：{}", root.display()),
                "在系统设置中恢复文件访问权限后重新扫描。",
            );
        }
        Err(error) => {
            return unavailable_root(
                SkillRootStatus::PartialFailure,
                format!("无法访问 Skill 根目录 {}：{error}", root.display()),
                "检查磁盘和目录状态后重新扫描。",
            );
        }
    }
    if let Err(error) = fs::read_dir(root)
        && error.kind() == std::io::ErrorKind::PermissionDenied
    {
        return unavailable_root(
            SkillRootStatus::PermissionDenied,
            format!("没有权限读取 Skill 根目录：{}", root.display()),
            "在系统设置或文件权限中恢复访问后重新扫描。",
        );
    }

    let mut instances = Vec::new();
    let mut warnings = Vec::new();
    discover_directory(
        root_id,
        root,
        root,
        &mut instances,
        &mut HashSet::new(),
        &mut warnings,
    );
    instances.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    if warnings.is_empty() {
        ScanOutcome {
            instances,
            status: SkillRootStatus::Ready,
            error: None,
            recovery_hint: None,
        }
    } else {
        ScanOutcome {
            instances,
            status: SkillRootStatus::PartialFailure,
            error: Some(warnings.join("；")),
            recovery_hint: Some("检查提示中的目录或符号链接后重新扫描。".to_owned()),
        }
    }
}

fn unavailable_root(status: SkillRootStatus, error: String, recovery_hint: &str) -> ScanOutcome {
    ScanOutcome {
        instances: Vec::new(),
        status,
        error: Some(error),
        recovery_hint: Some(recovery_hint.to_owned()),
    }
}

fn discover_directory(
    root_id: i64,
    root: &Path,
    directory: &Path,
    instances: &mut Vec<SkillInstance>,
    ancestors: &mut HashSet<PathBuf>,
    warnings: &mut Vec<String>,
) {
    let real_directory = match directory.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            warnings.push(format!("无法访问目录 {}：{error}", directory.display()));
            return;
        }
    };
    if !ancestors.insert(real_directory.clone()) {
        return;
    }

    let skill_file = directory.join("SKILL.md");
    if skill_file
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_file())
    {
        instances.push(read_skill_instance(root_id, root, directory, &skill_file));
        ancestors.remove(&real_directory);
        return;
    }

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(format!("无法读取目录 {}：{error}", directory.display()));
            ancestors.remove(&real_directory);
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(format!("读取目录项失败：{error}"));
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                warnings.push(format!("无法读取 {}：{error}", entry.path().display()));
                continue;
            }
        };
        if (file_type.is_dir() || file_type.is_symlink())
            && !is_ignored_directory(&entry.file_name())
        {
            match entry.path().canonicalize() {
                Ok(path) if path.is_dir() => {
                    discover_directory(root_id, root, &entry.path(), instances, ancestors, warnings)
                }
                Ok(_) => {}
                Err(error) if file_type.is_symlink() => warnings.push(format!(
                    "符号链接 {} 的目标不可访问：{error}",
                    entry.path().display()
                )),
                Err(error) => {
                    warnings.push(format!("目录 {} 不可访问：{error}", entry.path().display()))
                }
            }
        }
    }

    ancestors.remove(&real_directory);
}

fn is_ignored_directory(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    matches!(
        name,
        ".git"
            | ".hg"
            | ".svn"
            | "node_modules"
            | ".cache"
            | "__pycache__"
            | ".npm"
            | ".yarn"
            | ".pnpm-store"
            | ".Trash"
            | ".Trashes"
            | ".Spotlight-V100"
            | ".fseventsd"
            | ".TemporaryItems"
            | "Caches"
            | "target"
            | "$RECYCLE.BIN"
            | "System Volume Information"
            | ".skill-management-backups"
    ) || name.ends_with("_cache")
}

fn read_skill_instance(
    root_id: i64,
    root: &Path,
    directory: &Path,
    skill_file: &Path,
) -> SkillInstance {
    let metadata = fs::metadata(skill_file).ok();
    let modified_at = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .map(unix_millis)
        .unwrap_or_default();
    let created_at = metadata
        .as_ref()
        .and_then(|metadata| metadata.created().ok())
        .map(unix_millis)
        .unwrap_or(modified_at);
    let (parsed, search_document) = match fs::read(skill_file) {
        Ok(content) => match String::from_utf8(content) {
            Ok(content) => (parse_skill_document(&content), content),
            Err(_) => (
                ParsedSkillDocument {
                    error: Some("SKILL.md 不是有效的 UTF-8 文本".to_owned()),
                    ..ParsedSkillDocument::default()
                },
                String::new(),
            ),
        },
        Err(error) => (
            ParsedSkillDocument {
                error: Some(format!("无法读取 SKILL.md：{error}")),
                ..ParsedSkillDocument::default()
            },
            String::new(),
        ),
    };
    let fallback_name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名 Skill")
        .to_owned();
    let relative_path = normalized_relative_path(root, directory);
    let real_directory = directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_path_buf());
    let link_path = (real_directory != directory).then(|| directory.to_string_lossy().into_owned());
    let status = if parsed.error.is_some() {
        SkillStatus::NeedsRepair
    } else {
        SkillStatus::Ready
    };

    SkillInstance {
        id: format!("{root_id}:{relative_path}"),
        root_id,
        name: parsed.name.unwrap_or(fallback_name),
        description: parsed.description.unwrap_or_default(),
        relative_path,
        skill_file_path: skill_file.to_string_lossy().into_owned(),
        link_path,
        real_path: real_directory.to_string_lossy().into_owned(),
        status,
        error: parsed.error,
        client: detect_client(root),
        duplicate_check_status: DuplicateCheckStatus::None,
        created_at,
        modified_at,
        search_document,
    }
}

fn unix_millis(time: std::time::SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

fn detect_client(root: &Path) -> SkillClient {
    let normalized = root.to_string_lossy().to_ascii_lowercase();
    if normalized.contains("/.claude/") || normalized.ends_with("/.claude") {
        SkillClient::Claude
    } else if normalized.contains("/.codex/") || normalized.ends_with("/.codex") {
        SkillClient::Codex
    } else if normalized.contains("/.gemini/") || normalized.ends_with("/.gemini") {
        SkillClient::Gemini
    } else if normalized.contains("/.opencode/")
        || normalized.contains("/opencode/")
        || normalized.ends_with("/.opencode")
        || normalized.ends_with("/opencode")
    {
        SkillClient::OpenCode
    } else if normalized.contains("/.hermes/")
        || normalized.contains("/hermes/")
        || normalized.ends_with("/.hermes")
        || normalized.ends_with("/hermes")
    {
        SkillClient::Hermes
    } else {
        SkillClient::Other
    }
}

fn normalized_relative_path(root: &Path, directory: &Path) -> String {
    directory
        .strip_prefix(root)
        .unwrap_or(directory)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[derive(Debug, Default)]
struct ParsedSkillDocument {
    name: Option<String>,
    description: Option<String>,
    error: Option<String>,
}

fn parse_skill_document(content: &str) -> ParsedSkillDocument {
    let Some(frontmatter) = frontmatter(content) else {
        return ParsedSkillDocument {
            error: Some("SKILL.md 缺少 YAML frontmatter".to_owned()),
            ..ParsedSkillDocument::default()
        };
    };

    let metadata = match serde_yaml::from_str::<SkillMetadata>(frontmatter) {
        Ok(metadata) => metadata,
        Err(error) => {
            return ParsedSkillDocument {
                error: Some(format!("YAML frontmatter 无法解析：{error}")),
                ..ParsedSkillDocument::default()
            };
        }
    };

    let name = non_empty(metadata.name);
    let description = non_empty(metadata.description);
    let error = if name.is_none() {
        Some("YAML frontmatter 缺少 name".to_owned())
    } else if description.is_none() {
        Some("YAML frontmatter 缺少 description".to_owned())
    } else {
        None
    };

    ParsedSkillDocument {
        name,
        description,
        error,
    }
}

fn frontmatter(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

#[derive(Debug, Deserialize)]
struct SkillMetadata {
    name: Option<String>,
    description: Option<String>,
}
