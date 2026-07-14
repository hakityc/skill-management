use std::{
    collections::HashSet,
    fs,
    hash::{DefaultHasher, Hash, Hasher},
    io::Read,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::{
    SkillInstance, SkillWorkspace, WorkspaceError, WorkspaceSnapshot,
    detail::safe_relative_path,
    edit::{
        atomic_replace_directory, copy_directory, directory_fingerprint, remove_path_if_exists,
        sibling_work_path,
    },
    unix_millis,
};

const MAX_ZIP_ENTRIES: usize = 10_000;
const MAX_ZIP_UNCOMPRESSED_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileOperationKind {
    Import,
    Copy,
    Move,
    Trash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileConflictPolicy {
    Skip,
    Overwrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationRequest {
    pub instance_ids: Vec<String>,
    pub kind: FileOperationKind,
    pub target_root_id: Option<i64>,
    pub conflict_policy: FileConflictPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipImportRequest {
    pub zip_path: String,
    pub target_root_id: i64,
    pub relative_path: String,
    pub conflict_policy: FileConflictPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationPlan {
    pub id: i64,
    pub kind: FileOperationKind,
    pub items: Vec<PlannedFileOperationItem>,
    pub undoable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedFileOperationItem {
    pub instance_id: Option<String>,
    pub source: String,
    pub target: Option<String>,
    pub conflict: bool,
    pub will_overwrite: bool,
    pub will_remove_source: bool,
    pub file_count: usize,
    pub total_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileOperationResultStatus {
    Success,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationItemResult {
    pub instance_id: Option<String>,
    pub source: String,
    pub target: Option<String>,
    pub status: FileOperationResultStatus,
    pub message: String,
    pub backup_created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationBatchOutcome {
    pub batch_id: i64,
    pub results: Vec<FileOperationItemResult>,
    pub snapshot: WorkspaceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationRecord {
    pub batch_id: i64,
    pub plan_id: i64,
    pub kind: FileOperationKind,
    pub created_at: i64,
    pub undoable: bool,
    pub undone: bool,
    pub plan: FileOperationPlan,
    pub results: Vec<FileOperationItemResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingFileOperationPlan {
    kind: FileOperationKind,
    conflict_policy: FileConflictPolicy,
    items: Vec<PendingFileOperationItem>,
    staging_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingFileOperationItem {
    instance_id: Option<String>,
    source_root_id: Option<i64>,
    #[serde(default)]
    source_root: Option<String>,
    #[serde(default)]
    source_relative_path: Option<String>,
    target_root_id: Option<i64>,
    #[serde(default)]
    target_root: Option<String>,
    #[serde(default)]
    target_relative_path: Option<String>,
    source: String,
    target: Option<String>,
    source_fingerprint: u64,
    target_fingerprint: Option<u64>,
    conflict: bool,
    file_count: usize,
    total_size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UndoPayload {
    items: Vec<UndoItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UndoItem {
    kind: FileOperationKind,
    source: String,
    target: String,
    target_backup: Option<String>,
    target_was_new: bool,
    source_backup: Option<String>,
    source_root_id: Option<i64>,
    target_root_id: Option<i64>,
    #[serde(default)]
    source_root: Option<String>,
    #[serde(default)]
    source_relative_path: Option<String>,
    #[serde(default)]
    target_root: Option<String>,
    #[serde(default)]
    target_relative_path: Option<String>,
    #[serde(default)]
    applied_target_fingerprint: Option<u64>,
}

impl SkillWorkspace {
    pub fn plan_file_operations(
        &self,
        request: &FileOperationRequest,
    ) -> Result<FileOperationPlan, WorkspaceError> {
        if matches!(request.kind, FileOperationKind::Import) {
            return Err(WorkspaceError::InvalidFileOperation(
                "ZIP 导入必须通过 ZIP 预览创建计划。".to_owned(),
            ));
        }
        if request.instance_ids.is_empty() {
            return Err(WorkspaceError::InvalidFileOperation(
                "至少选择一个 Skill 实例。".to_owned(),
            ));
        }
        let snapshot = self.snapshot()?;
        let target_root = request
            .target_root_id
            .map(|root_id| {
                snapshot
                    .roots
                    .iter()
                    .find(|root| root.id == root_id)
                    .cloned()
                    .ok_or_else(|| {
                        WorkspaceError::InvalidRoot("找不到目标 Skill 根目录。".to_owned())
                    })
            })
            .transpose()?;
        if matches!(
            request.kind,
            FileOperationKind::Copy | FileOperationKind::Move
        ) && target_root.is_none()
        {
            return Err(WorkspaceError::InvalidFileOperation(
                "复制或移动必须选择目标 Skill 根目录。".to_owned(),
            ));
        }
        let mut seen = HashSet::new();
        let mut planned_targets = HashSet::new();
        let mut items = Vec::new();
        for instance_id in &request.instance_ids {
            if !seen.insert(instance_id) {
                continue;
            }
            let instance = snapshot
                .instances
                .iter()
                .find(|instance| instance.id == *instance_id)
                .ok_or_else(|| WorkspaceError::UnknownInstance(instance_id.clone()))?;
            let source_root = snapshot
                .roots
                .iter()
                .find(|root| root.id == instance.root_id)
                .ok_or_else(|| {
                    WorkspaceError::InvalidRoot("找不到来源 Skill 根目录。".to_owned())
                })?;
            if target_root
                .as_ref()
                .is_some_and(|root| root.id == instance.root_id)
            {
                return Err(WorkspaceError::InvalidFileOperation(
                    "来源与目标 Skill 根目录不能相同。".to_owned(),
                ));
            }
            let source =
                operation_source_path(instance, &request.kind, Path::new(&source_root.path))?;
            let target = target_root
                .as_ref()
                .map(|root| PathBuf::from(&root.path).join(&instance.relative_path));
            if let Some(target) = &target {
                validate_target_ancestors(
                    Path::new(&target_root.as_ref().expect("目标根目录必须存在").path),
                    target,
                )?;
                let collision_key = target.to_string_lossy().to_lowercase();
                if !planned_targets.insert(collision_key) {
                    return Err(WorkspaceError::InvalidFileOperation(format!(
                        "多个所选 Skill 会写入同一目标“{}”；请拆分操作或只保留一个来源。",
                        target.to_string_lossy()
                    )));
                }
            }
            let (file_count, total_size) = directory_impact(&source)?;
            items.push(PendingFileOperationItem {
                instance_id: Some(instance.id.clone()),
                source_root_id: Some(instance.root_id),
                source_root: Some(source_root.path.clone()),
                source_relative_path: Some(instance.relative_path.clone()),
                target_root_id: target_root.as_ref().map(|root| root.id),
                target_root: target_root.as_ref().map(|root| root.path.clone()),
                target_relative_path: target_root.as_ref().map(|_| instance.relative_path.clone()),
                source: source.to_string_lossy().into_owned(),
                target: target
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                source_fingerprint: operation_path_fingerprint(&source)?,
                target_fingerprint: target
                    .as_ref()
                    .filter(|path| path_entry_exists(path))
                    .map(|path| operation_path_fingerprint(path))
                    .transpose()?,
                conflict: target.as_ref().is_some_and(|path| path_entry_exists(path)),
                file_count,
                total_size,
            });
        }
        store_plan(
            &self.database_path,
            PendingFileOperationPlan {
                kind: request.kind.clone(),
                conflict_policy: request.conflict_policy.clone(),
                items,
                staging_root: None,
            },
        )
    }

    pub fn preview_zip_import(
        &self,
        request: &ZipImportRequest,
    ) -> Result<FileOperationPlan, WorkspaceError> {
        let relative_path = safe_relative_path(&request.relative_path)?;
        let snapshot = self.snapshot()?;
        let root = snapshot
            .roots
            .iter()
            .find(|root| root.id == request.target_root_id)
            .ok_or_else(|| WorkspaceError::InvalidRoot("找不到目标 Skill 根目录。".to_owned()))?;
        let target = PathBuf::from(&root.path).join(relative_path);
        validate_target_ancestors(Path::new(&root.path), &target)?;
        let staging_root = unique_staging_directory(&self.database_path)?;
        fs::create_dir_all(&staging_root)?;
        let extracted = match extract_zip_safely(Path::new(&request.zip_path), &staging_root) {
            Ok(path) => path,
            Err(error) => {
                let _ = remove_path_if_exists(&staging_root);
                return Err(error);
            }
        };
        let (file_count, total_size) = directory_impact(&extracted)?;
        let pending = PendingFileOperationPlan {
            kind: FileOperationKind::Import,
            conflict_policy: request.conflict_policy.clone(),
            items: vec![PendingFileOperationItem {
                instance_id: None,
                source_root_id: None,
                source_root: None,
                source_relative_path: None,
                target_root_id: Some(root.id),
                target_root: Some(root.path.clone()),
                target_relative_path: Some(request.relative_path.clone()),
                source: extracted.to_string_lossy().into_owned(),
                target: Some(target.to_string_lossy().into_owned()),
                source_fingerprint: operation_path_fingerprint(&extracted)?,
                target_fingerprint: path_entry_exists(&target)
                    .then(|| operation_path_fingerprint(&target))
                    .transpose()?,
                conflict: path_entry_exists(&target),
                file_count,
                total_size,
            }],
            staging_root: Some(staging_root.to_string_lossy().into_owned()),
        };
        match store_plan(&self.database_path, pending) {
            Ok(plan) => Ok(plan),
            Err(error) => {
                let _ = remove_path_if_exists(&staging_root);
                Err(error)
            }
        }
    }

    pub fn execute_file_operation_plan(
        &self,
        plan_id: i64,
    ) -> Result<FileOperationBatchOutcome, WorkspaceError> {
        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let serialized = transaction
            .query_row(
                "SELECT payload FROM file_operation_plans WHERE id = ?1",
                [plan_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(WorkspaceError::UnknownFileOperationPlan(plan_id))?;
        let pending: PendingFileOperationPlan = serde_json::from_str(&serialized)?;
        let undoable = !matches!(pending.kind, FileOperationKind::Trash);
        let public_plan = public_plan(plan_id, &pending);
        transaction.execute(
            "
            INSERT INTO file_operation_batches (
                plan_id, kind, created_at, undoable, undone, completed,
                results_payload, undo_payload, plan_payload, staging_root
            ) VALUES (?1, ?2, ?3, ?4, 0, 0, '[]', '{\"items\":[]}', ?5, ?6)
            ",
            params![
                plan_id,
                operation_kind_database(&pending.kind),
                unix_millis(SystemTime::now()),
                undoable,
                serde_json::to_string(&public_plan)?,
                pending.staging_root,
            ],
        )?;
        let batch_id = transaction.last_insert_rowid();
        let batch_backup_root = undoable.then(|| {
            database_parent(&self.database_path)
                .join("file-operation-backups")
                .join(format!("batch-{batch_id}"))
        });
        if let Some(backup_root) = &batch_backup_root {
            transaction.execute(
                "UPDATE file_operation_batches SET backup_root = ?1 WHERE id = ?2",
                params![backup_root.to_string_lossy(), batch_id],
            )?;
        }
        if transaction.execute("DELETE FROM file_operation_plans WHERE id = ?1", [plan_id])? != 1 {
            return Err(WorkspaceError::UnknownFileOperationPlan(plan_id));
        }
        transaction.commit()?;
        let mut results = Vec::new();
        let mut undo_payload = UndoPayload::default();
        for (index, item) in pending.items.iter().enumerate() {
            if pending.kind == FileOperationKind::Trash {
                let mut durable_results = results.clone();
                durable_results.push(result_for(
                    item,
                    FileOperationResultStatus::Failed,
                    "应用在确认废纸篓结果前中断时，请在访达的废纸篓中核对此项。",
                    false,
                ));
                persist_batch_progress(&connection, batch_id, &durable_results, &undo_payload)?;
            }
            let result = if item.conflict && pending.conflict_policy == FileConflictPolicy::Skip {
                result_for(
                    item,
                    FileOperationResultStatus::Skipped,
                    "目标已存在，已按计划跳过。",
                    false,
                )
            } else {
                match self.execute_operation_item(
                    batch_id,
                    index,
                    &pending.kind,
                    item,
                    &mut undo_payload,
                ) {
                    Ok(backup_created) => result_for(
                        item,
                        FileOperationResultStatus::Success,
                        if pending.kind == FileOperationKind::Trash {
                            "已移入系统废纸篓；可在访达的废纸篓中恢复。"
                        } else {
                            "操作完成。"
                        },
                        backup_created,
                    ),
                    Err(error) => result_for(
                        item,
                        FileOperationResultStatus::Failed,
                        &error.to_string(),
                        false,
                    ),
                }
            };
            results.push(result);
            persist_batch_progress(&connection, batch_id, &results, &undo_payload)?;
        }
        if let Some(staging_root) = &pending.staging_root {
            cleanup_staging_root(&self.database_path, Path::new(staging_root))?;
        }
        if undo_payload.items.is_empty()
            && let Some(backup_root) = &batch_backup_root
        {
            cleanup_backup_root(&self.database_path, backup_root)?;
        }
        connection.execute(
            "
            UPDATE file_operation_batches
            SET completed = 1,
                staging_root = NULL,
                backup_root = CASE WHEN ?2 THEN NULL ELSE backup_root END
            WHERE id = ?1
            ",
            params![batch_id, undo_payload.items.is_empty()],
        )?;
        self.rescan_all_roots()?;
        Ok(FileOperationBatchOutcome {
            batch_id,
            results,
            snapshot: self.snapshot()?,
        })
    }

    pub fn file_operation_history(&self) -> Result<Vec<FileOperationRecord>, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let mut statement = connection.prepare(
            "
            SELECT id, plan_id, kind, created_at, undoable, undone,
                   results_payload, plan_payload
            FROM file_operation_batches
            WHERE completed = 1
            ORDER BY created_at DESC, id DESC
            ",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })?
            .map(|row| {
                let (batch_id, plan_id, kind, created_at, undoable, undone, payload, plan_payload) =
                    row?;
                let kind = operation_kind_from_database(&kind);
                let plan = match plan_payload {
                    Some(payload) => serde_json::from_str(&payload)?,
                    None => FileOperationPlan {
                        id: plan_id,
                        kind: kind.clone(),
                        items: Vec::new(),
                        undoable,
                    },
                };
                Ok(FileOperationRecord {
                    batch_id,
                    plan_id,
                    kind,
                    created_at,
                    undoable,
                    undone,
                    plan,
                    results: serde_json::from_str(&payload)?,
                })
            })
            .collect::<Result<Vec<_>, WorkspaceError>>()
    }

    pub fn latest_undoable_file_operation(
        &self,
    ) -> Result<Option<FileOperationRecord>, WorkspaceError> {
        Ok(self.file_operation_history()?.into_iter().find(|record| {
            record.undoable
                && !record.undone
                && record
                    .results
                    .iter()
                    .any(|result| result.status == FileOperationResultStatus::Success)
        }))
    }

    pub fn cancel_file_operation_plan(&self, plan_id: i64) -> Result<(), WorkspaceError> {
        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let payload = transaction
            .query_row(
                "SELECT payload FROM file_operation_plans WHERE id = ?1",
                [plan_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(WorkspaceError::UnknownFileOperationPlan(plan_id))?;
        let pending: PendingFileOperationPlan = serde_json::from_str(&payload)?;
        if let Some(staging_root) = &pending.staging_root {
            cleanup_staging_root(&self.database_path, Path::new(staging_root))?;
        }
        transaction.execute("DELETE FROM file_operation_plans WHERE id = ?1", [plan_id])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn undo_file_operation_batch(
        &self,
        batch_id: i64,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let row = connection
            .query_row(
                "SELECT undoable, undone, undo_payload, backup_root FROM file_operation_batches WHERE id = ?1 AND completed = 1",
                [batch_id],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?)),
            )
            .optional()?
            .ok_or(WorkspaceError::UnknownFileOperationBatch(batch_id))?;
        if !row.0 {
            return Err(WorkspaceError::InvalidFileOperation(
                "移入系统废纸篓的操作请在访达中恢复。".to_owned(),
            ));
        }
        if row.1 {
            return Err(WorkspaceError::FileOperationAlreadyUndone);
        }
        let undo_payload: UndoPayload = serde_json::from_str(&row.2)?;
        validate_undo_payload(&self.database_path, &undo_payload)?;
        connection.execute(
            "UPDATE file_operation_batches SET undoing = 1 WHERE id = ?1",
            [batch_id],
        )?;
        rollback_undo_payload(&self.database_path, &undo_payload, batch_id)?;
        if let Some(backup_root) = &row.3 {
            cleanup_backup_root(&self.database_path, Path::new(backup_root))?;
        }
        connection.execute(
            "UPDATE file_operation_batches SET undone = 1, undoing = 0, backup_root = NULL WHERE id = ?1",
            [batch_id],
        )?;
        self.rescan_all_roots()?;
        self.snapshot()
    }

    fn execute_operation_item(
        &self,
        batch_id: i64,
        index: usize,
        kind: &FileOperationKind,
        item: &PendingFileOperationItem,
        undo_payload: &mut UndoPayload,
    ) -> Result<bool, WorkspaceError> {
        let source = PathBuf::from(&item.source);
        if matches!(kind, FileOperationKind::Move | FileOperationKind::Trash) {
            let validated_source = validated_recorded_path(
                item.source_root.as_deref(),
                item.source_relative_path.as_deref(),
                &item.source,
            )?;
            validate_target_ancestors(
                Path::new(item.source_root.as_deref().ok_or_else(|| {
                    WorkspaceError::InvalidFileOperation("操作计划缺少来源根目录。".to_owned())
                })?),
                &validated_source,
            )?;
        }
        if !path_entry_exists(&source)
            || operation_path_fingerprint(&source)? != item.source_fingerprint
        {
            return Err(WorkspaceError::StaleFileOperationPlan(item.source.clone()));
        }
        if *kind == FileOperationKind::Trash {
            self.move_to_trash(&source, batch_id, index)?;
            return Ok(false);
        }
        let target = PathBuf::from(item.target.as_deref().ok_or_else(|| {
            WorkspaceError::InvalidFileOperation("操作计划缺少目标路径。".to_owned())
        })?);
        prepare_target_parent(
            Path::new(item.target_root.as_deref().ok_or_else(|| {
                WorkspaceError::InvalidFileOperation("操作计划缺少目标根目录。".to_owned())
            })?),
            &target,
        )?;
        let current_target = path_entry_exists(&target)
            .then(|| operation_path_fingerprint(&target))
            .transpose()?;
        if current_target != item.target_fingerprint {
            return Err(WorkspaceError::StaleFileOperationPlan(
                target.to_string_lossy().into_owned(),
            ));
        }
        let backup_root = self
            .database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("file-operation-backups")
            .join(format!("batch-{batch_id}"))
            .join(format!("item-{index}"));
        let application_data_root = database_parent(&self.database_path);
        prepare_target_parent(&application_data_root, &backup_root.join("entry"))?;
        let target_backup = item
            .target_fingerprint
            .is_some()
            .then(|| backup_root.join("target"));
        let source_backup = (*kind == FileOperationKind::Move).then(|| backup_root.join("source"));
        let snapshot_result = (|| {
            if let Some(backup) = &target_backup {
                remove_path_if_exists(backup)?;
                copy_path_snapshot(&target, backup)?;
            }
            if let Some(backup) = &source_backup {
                remove_path_if_exists(backup)?;
                copy_path_snapshot(&source, backup)?;
            }
            Ok::<(), WorkspaceError>(())
        })();
        if let Err(error) = snapshot_result {
            let _ = cleanup_backup_item(&self.database_path, &backup_root);
            return Err(error);
        }
        let undo_item = UndoItem {
            kind: kind.clone(),
            source: item.source.clone(),
            target: target.to_string_lossy().into_owned(),
            target_backup: target_backup
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            target_was_new: item.target_fingerprint.is_none(),
            source_backup: source_backup
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            source_root_id: item.source_root_id,
            target_root_id: item.target_root_id,
            source_root: item.source_root.clone(),
            source_relative_path: item.source_relative_path.clone(),
            target_root: item.target_root.clone(),
            target_relative_path: item.target_relative_path.clone(),
            applied_target_fingerprint: None,
        };
        undo_payload.items.push(undo_item.clone());
        let connection = Connection::open(&self.database_path)?;
        persist_undo_progress(&connection, batch_id, undo_payload)?;

        let operation = (|| {
            let stage = sibling_work_path(
                &target,
                "file-operation-stage",
                batch_id * 10_000 + index as i64,
            )?;
            remove_path_if_exists(&stage)?;
            copy_directory(&source, &stage)?;
            atomic_replace_directory(&stage, &target, batch_id * 10_000 + index as i64)?;
            if let Some(source_backup) = &source_backup {
                debug_assert!(path_entry_exists(source_backup));
                remove_path_if_exists(&source)?;
            }
            operation_path_fingerprint(&target)
        })();
        let applied_target_fingerprint = match operation {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                let _ = rollback_undo_item(&self.database_path, &undo_item, batch_id);
                undo_payload.items.pop();
                persist_undo_progress(&connection, batch_id, undo_payload)?;
                let _ = cleanup_backup_item(&self.database_path, &backup_root);
                return Err(error);
            }
        };
        undo_payload
            .items
            .last_mut()
            .expect("刚加入的撤销项必须存在")
            .applied_target_fingerprint = Some(applied_target_fingerprint);
        persist_undo_progress(&connection, batch_id, undo_payload)?;
        Ok(target_backup.is_some())
    }

    fn move_to_trash(
        &self,
        source: &Path,
        batch_id: i64,
        index: usize,
    ) -> Result<(), WorkspaceError> {
        if let Some(trash_directory) = &self.trash_directory {
            fs::create_dir_all(trash_directory)?;
            let name = source
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("skill");
            let target = trash_directory.join(format!("{batch_id}-{index}-{name}"));
            fs::rename(source, target)?;
            return Ok(());
        }
        trash::delete(source).map_err(|error| {
            WorkspaceError::InvalidFileOperation(format!("无法移入系统废纸篓：{error}"))
        })
    }

    pub(crate) fn recover_interrupted_file_operations(&self) -> Result<(), WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let mut statement = connection.prepare(
            "
            SELECT id, completed, undoing, kind, undo_payload, staging_root, backup_root
            FROM file_operation_batches
            WHERE completed = 0 OR undoing = 1 OR staging_root IS NOT NULL
            ORDER BY id
            ",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let had_interrupted_batches = !rows.is_empty();
        for (batch_id, completed, undoing, kind, payload, staging_root, backup_root) in rows {
            let undo_payload: UndoPayload = serde_json::from_str(&payload)?;
            if !completed || undoing {
                if kind == "trash" && !completed {
                    connection.execute(
                        "UPDATE file_operation_batches SET completed = 1, undone = 0, undoing = 0 WHERE id = ?1",
                        [batch_id],
                    )?;
                } else {
                    rollback_undo_payload(&self.database_path, &undo_payload, batch_id)?;
                    connection.execute(
                        "UPDATE file_operation_batches SET completed = 1, undone = 1, undoing = 0, backup_root = NULL WHERE id = ?1",
                        [batch_id],
                    )?;
                }
                if let Some(backup_root) = &backup_root {
                    cleanup_backup_root(&self.database_path, Path::new(backup_root))?;
                    connection.execute(
                        "UPDATE file_operation_batches SET backup_root = NULL WHERE id = ?1",
                        [batch_id],
                    )?;
                }
            }
            if let Some(staging_root) = staging_root {
                cleanup_staging_root(&self.database_path, Path::new(&staging_root))?;
                connection.execute(
                    "UPDATE file_operation_batches SET staging_root = NULL WHERE id = ?1",
                    [batch_id],
                )?;
            }
        }
        if had_interrupted_batches {
            self.rescan_all_roots()?;
        }
        Ok(())
    }

    pub(crate) fn cleanup_abandoned_file_operation_plans(&self) -> Result<(), WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let mut statement = connection.prepare("SELECT id, payload FROM file_operation_plans")?;
        let plans = statement
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (plan_id, payload) in plans {
            let pending: PendingFileOperationPlan = serde_json::from_str(&payload)?;
            if let Some(staging_root) = &pending.staging_root {
                cleanup_staging_root(&self.database_path, Path::new(staging_root))?;
            }
            connection.execute("DELETE FROM file_operation_plans WHERE id = ?1", [plan_id])?;
        }
        Ok(())
    }

    pub(crate) fn cleanup_orphan_file_operation_backups(&self) -> Result<(), WorkspaceError> {
        let backup_base = application_backup_base(&self.database_path);
        if !path_entry_exists(&backup_base) {
            return Ok(());
        }
        let application_data_root = database_parent(&self.database_path);
        validate_target_ancestors(&application_data_root, &backup_base.join("entry"))?;
        let connection = Connection::open(&self.database_path)?;
        let mut referenced = HashSet::new();
        let mut statement = connection.prepare(
            "SELECT backup_root, undo_payload FROM file_operation_batches WHERE undone = 0",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        for (backup_root, undo_payload) in rows {
            if let Some(backup_root) = backup_root {
                referenced.insert(PathBuf::from(backup_root));
            }
            let payload: UndoPayload = serde_json::from_str(&undo_payload)?;
            for item in payload.items {
                for backup in [item.target_backup, item.source_backup]
                    .into_iter()
                    .flatten()
                {
                    let path = PathBuf::from(backup);
                    if let Ok(relative) = path.strip_prefix(&backup_base)
                        && let Some(Component::Normal(batch_name)) = relative.components().next()
                    {
                        referenced.insert(backup_base.join(batch_name));
                    }
                }
            }
        }
        for entry in fs::read_dir(&backup_base)? {
            let path = entry?.path();
            let is_batch = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("batch-"));
            if is_batch && !referenced.contains(&path) {
                cleanup_backup_root(&self.database_path, &path)?;
            }
        }
        Ok(())
    }
}

pub(crate) fn initialize_file_operations(connection: &Connection) -> Result<(), WorkspaceError> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS file_operation_plans (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            payload TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS file_operation_batches (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            plan_id INTEGER NOT NULL,
            kind TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            undoable INTEGER NOT NULL,
            undone INTEGER NOT NULL DEFAULT 0,
            completed INTEGER NOT NULL DEFAULT 0,
            undoing INTEGER NOT NULL DEFAULT 0,
            results_payload TEXT NOT NULL,
            undo_payload TEXT NOT NULL,
            plan_payload TEXT,
            staging_root TEXT,
            backup_root TEXT
        );
        ",
    )?;
    let has_plan_payload = {
        let mut statement = connection.prepare("PRAGMA table_info(file_operation_batches)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|column| column == "plan_payload")
    };
    if !has_plan_payload {
        connection.execute(
            "ALTER TABLE file_operation_batches ADD COLUMN plan_payload TEXT",
            [],
        )?;
    }
    let has_undoing = {
        let mut statement = connection.prepare("PRAGMA table_info(file_operation_batches)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|column| column == "undoing")
    };
    if !has_undoing {
        connection.execute(
            "ALTER TABLE file_operation_batches ADD COLUMN undoing INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    let has_staging_root = {
        let mut statement = connection.prepare("PRAGMA table_info(file_operation_batches)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|column| column == "staging_root")
    };
    if !has_staging_root {
        connection.execute(
            "ALTER TABLE file_operation_batches ADD COLUMN staging_root TEXT",
            [],
        )?;
    }
    let has_backup_root = {
        let mut statement = connection.prepare("PRAGMA table_info(file_operation_batches)")?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|column| column == "backup_root")
    };
    if !has_backup_root {
        connection.execute(
            "ALTER TABLE file_operation_batches ADD COLUMN backup_root TEXT",
            [],
        )?;
    }
    Ok(())
}

fn store_plan(
    database_path: &Path,
    pending: PendingFileOperationPlan,
) -> Result<FileOperationPlan, WorkspaceError> {
    let connection = Connection::open(database_path)?;
    connection.execute(
        "INSERT INTO file_operation_plans (payload, created_at) VALUES (?1, ?2)",
        params![
            serde_json::to_string(&pending)?,
            unix_millis(SystemTime::now())
        ],
    )?;
    let id = connection.last_insert_rowid();
    Ok(public_plan(id, &pending))
}

fn public_plan(id: i64, pending: &PendingFileOperationPlan) -> FileOperationPlan {
    FileOperationPlan {
        id,
        kind: pending.kind.clone(),
        undoable: pending.kind != FileOperationKind::Trash,
        items: pending
            .items
            .iter()
            .map(|item| PlannedFileOperationItem {
                instance_id: item.instance_id.clone(),
                source: item.source.clone(),
                target: item.target.clone(),
                conflict: item.conflict,
                will_overwrite: item.conflict
                    && pending.conflict_policy == FileConflictPolicy::Overwrite,
                will_remove_source: matches!(
                    pending.kind,
                    FileOperationKind::Move | FileOperationKind::Trash
                ),
                file_count: item.file_count,
                total_size: item.total_size,
            })
            .collect(),
    }
}

fn result_for(
    item: &PendingFileOperationItem,
    status: FileOperationResultStatus,
    message: &str,
    backup_created: bool,
) -> FileOperationItemResult {
    FileOperationItemResult {
        instance_id: item.instance_id.clone(),
        source: item.source.clone(),
        target: item.target.clone(),
        status,
        message: message.to_owned(),
        backup_created,
    }
}

fn persist_batch_progress(
    connection: &Connection,
    batch_id: i64,
    results: &[FileOperationItemResult],
    undo_payload: &UndoPayload,
) -> Result<(), WorkspaceError> {
    connection.execute(
        "UPDATE file_operation_batches SET results_payload = ?1, undo_payload = ?2 WHERE id = ?3",
        params![
            serde_json::to_string(results)?,
            serde_json::to_string(undo_payload)?,
            batch_id
        ],
    )?;
    Ok(())
}

fn persist_undo_progress(
    connection: &Connection,
    batch_id: i64,
    undo_payload: &UndoPayload,
) -> Result<(), WorkspaceError> {
    connection.execute(
        "UPDATE file_operation_batches SET undo_payload = ?1 WHERE id = ?2",
        params![serde_json::to_string(undo_payload)?, batch_id],
    )?;
    Ok(())
}

fn rollback_undo_payload(
    database_path: &Path,
    payload: &UndoPayload,
    identifier: i64,
) -> Result<(), WorkspaceError> {
    validate_undo_paths(database_path, payload)?;
    for item in payload.items.iter().rev() {
        rollback_undo_item(database_path, item, identifier)?;
    }
    Ok(())
}

fn validate_undo_payload(
    database_path: &Path,
    payload: &UndoPayload,
) -> Result<(), WorkspaceError> {
    validate_undo_paths(database_path, payload)?;
    for item in &payload.items {
        let target = Path::new(&item.target);
        let current_target_fingerprint = path_entry_exists(target)
            .then(|| operation_path_fingerprint(target))
            .transpose()?;
        if current_target_fingerprint != item.applied_target_fingerprint {
            return Err(WorkspaceError::StaleFileOperationPlan(item.target.clone()));
        }
        if item.kind == FileOperationKind::Move
            && Path::new(&item.source).symlink_metadata().is_ok()
        {
            return Err(WorkspaceError::StaleFileOperationPlan(item.source.clone()));
        }
        if let Some(backup) = &item.target_backup
            && Path::new(backup).symlink_metadata().is_err()
        {
            return Err(WorkspaceError::InvalidFileOperation(
                "撤销所需的目标备份不存在。".to_owned(),
            ));
        }
        if let Some(backup) = &item.source_backup
            && Path::new(backup).symlink_metadata().is_err()
        {
            return Err(WorkspaceError::InvalidFileOperation(
                "撤销所需的来源备份不存在。".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_undo_paths(database_path: &Path, payload: &UndoPayload) -> Result<(), WorkspaceError> {
    for item in &payload.items {
        let target = validated_recorded_path(
            item.target_root.as_deref(),
            item.target_relative_path.as_deref(),
            &item.target,
        )?;
        validate_target_ancestors(
            Path::new(item.target_root.as_deref().ok_or_else(|| {
                WorkspaceError::InvalidFileOperation("撤销记录缺少目标根目录。".to_owned())
            })?),
            &target,
        )?;
        if item.kind == FileOperationKind::Move {
            let source = validated_recorded_path(
                item.source_root.as_deref(),
                item.source_relative_path.as_deref(),
                &item.source,
            )?;
            validate_target_ancestors(
                Path::new(item.source_root.as_deref().ok_or_else(|| {
                    WorkspaceError::InvalidFileOperation("撤销记录缺少来源根目录。".to_owned())
                })?),
                &source,
            )?;
        }
        if let Some(backup) = &item.target_backup {
            validate_application_backup_path(database_path, Path::new(backup))?;
        }
        if let Some(backup) = &item.source_backup {
            validate_application_backup_path(database_path, Path::new(backup))?;
        }
    }
    Ok(())
}

fn validated_recorded_path(
    root: Option<&str>,
    relative_path: Option<&str>,
    recorded_path: &str,
) -> Result<PathBuf, WorkspaceError> {
    let root =
        Path::new(root.ok_or_else(|| {
            WorkspaceError::InvalidFileOperation("撤销记录缺少根目录。".to_owned())
        })?);
    let expected = root.join(safe_relative_path(relative_path.ok_or_else(|| {
        WorkspaceError::InvalidFileOperation("撤销记录缺少相对路径。".to_owned())
    })?)?);
    if expected != Path::new(recorded_path) {
        return Err(WorkspaceError::InvalidFileOperation(
            "撤销记录的路径与根目录不一致。".to_owned(),
        ));
    }
    Ok(expected)
}

fn rollback_undo_item(
    database_path: &Path,
    item: &UndoItem,
    identifier: i64,
) -> Result<(), WorkspaceError> {
    if let Some(backup) = &item.target_backup {
        validate_application_backup_path(database_path, Path::new(backup))?;
    }
    if let Some(backup) = &item.source_backup {
        validate_application_backup_path(database_path, Path::new(backup))?;
    }
    let target = PathBuf::from(&item.target);
    if item.target_was_new {
        remove_path_if_exists(&target)?;
    } else if let Some(backup) = &item.target_backup {
        prepare_target_parent(
            Path::new(item.target_root.as_deref().ok_or_else(|| {
                WorkspaceError::InvalidFileOperation("撤销记录缺少目标根目录。".to_owned())
            })?),
            &target,
        )?;
        let stage = sibling_work_path(&target, "file-operation-undo", identifier)?;
        remove_path_if_exists(&stage)?;
        copy_path_snapshot(Path::new(backup), &stage)?;
        atomic_replace_directory(&stage, &target, identifier)?;
    }
    if item.kind == FileOperationKind::Move
        && let Some(source_backup) = &item.source_backup
        && path_entry_exists(Path::new(source_backup))
    {
        let source = PathBuf::from(&item.source);
        remove_path_if_exists(&source)?;
        prepare_target_parent(
            Path::new(item.source_root.as_deref().ok_or_else(|| {
                WorkspaceError::InvalidFileOperation("撤销记录缺少来源根目录。".to_owned())
            })?),
            &source,
        )?;
        let stage = sibling_work_path(&source, "file-operation-source-undo", identifier)?;
        remove_path_if_exists(&stage)?;
        copy_path_snapshot(Path::new(source_backup), &stage)?;
        atomic_replace_directory(&stage, &source, identifier)?;
        remove_path_if_exists(Path::new(source_backup))?;
    }
    Ok(())
}

fn directory_impact(directory: &Path) -> Result<(usize, u64), WorkspaceError> {
    let mut count = 0;
    let mut size = 0;
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = path.symlink_metadata()?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            let child = directory_impact(&path)?;
            count += child.0;
            size += child.1;
        } else {
            count += 1;
            size += metadata.len();
        }
    }
    Ok((count, size))
}

fn operation_source_path(
    instance: &SkillInstance,
    kind: &FileOperationKind,
    root: &Path,
) -> Result<PathBuf, WorkspaceError> {
    let catalog_path = root.join(safe_relative_path(&instance.relative_path)?);
    if matches!(kind, FileOperationKind::Move | FileOperationKind::Trash) {
        validate_target_ancestors(root, &catalog_path).map_err(|_| {
            WorkspaceError::InvalidFileOperation(format!(
                "Skill“{}”位于符号链接目录下；为避免破坏共享真实目录，不能移动或删除。",
                instance.name
            ))
        })?;
    }
    let Some(link_path) = &instance.link_path else {
        return Ok(catalog_path);
    };
    let link_path = PathBuf::from(link_path);
    let metadata = link_path.symlink_metadata()?;
    if metadata.file_type().is_symlink() {
        // 叶节点本身是链接时，复制读取其内容；移动/删除只处理链接条目。
        return Ok(link_path);
    }
    if matches!(kind, FileOperationKind::Move | FileOperationKind::Trash) {
        return Err(WorkspaceError::InvalidFileOperation(format!(
            "Skill“{}”位于符号链接目录下；为避免破坏共享真实目录，不能移动或删除。",
            instance.name
        )));
    }
    Ok(PathBuf::from(&instance.real_path))
}

fn validate_target_ancestors(root: &Path, target: &Path) -> Result<(), WorkspaceError> {
    visit_target_ancestors(root, target, false)
}

fn prepare_target_parent(root: &Path, target: &Path) -> Result<(), WorkspaceError> {
    visit_target_ancestors(root, target, true)
}

fn visit_target_ancestors(
    root: &Path,
    target: &Path,
    create_missing: bool,
) -> Result<(), WorkspaceError> {
    let relative = target.strip_prefix(root).map_err(|_| {
        WorkspaceError::InvalidFileOperation("目标路径超出 Skill 根目录。".to_owned())
    })?;
    let root_metadata = root.symlink_metadata()?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(WorkspaceError::InvalidFileOperation(
            "目标 Skill 根目录不是安全的真实目录。".to_owned(),
        ));
    }
    let mut current = root.to_path_buf();
    for component in relative
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .components()
    {
        let Component::Normal(name) = component else {
            return Err(WorkspaceError::InvalidFileOperation(
                "目标路径包含不安全的目录片段。".to_owned(),
            ));
        };
        current.push(name);
        match current.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(WorkspaceError::InvalidFileOperation(format!(
                    "目标路径经过符号链接目录“{}”，已拒绝操作。",
                    current.to_string_lossy()
                )));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(WorkspaceError::InvalidFileOperation(format!(
                    "目标路径的父级不是目录：“{}”。",
                    current.to_string_lossy()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && create_missing => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.into()),
                }
                let metadata = current.symlink_metadata()?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(WorkspaceError::InvalidFileOperation(
                        "创建目标父目录时检测到路径被替换，已取消操作。".to_owned(),
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn copy_path_snapshot(source: &Path, destination: &Path) -> Result<(), WorkspaceError> {
    let metadata = source.symlink_metadata()?;
    if metadata.file_type().is_symlink() {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        create_symlink(&fs::read_link(source)?, destination)
    } else {
        copy_directory(source, destination)
    }
}

fn path_entry_exists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

fn operation_path_fingerprint(path: &Path) -> Result<u64, WorkspaceError> {
    let metadata = path.symlink_metadata()?;
    let mut hasher = DefaultHasher::new();
    if metadata.file_type().is_symlink() {
        "root-symbolic-link".hash(&mut hasher);
        fs::read_link(path)?.hash(&mut hasher);
        if path.exists() {
            directory_fingerprint(path)?.hash(&mut hasher);
        }
    } else {
        "root-directory".hash(&mut hasher);
        directory_fingerprint(path)?.hash(&mut hasher);
    }
    Ok(hasher.finish())
}

fn unique_staging_directory(database_path: &Path) -> Result<PathBuf, WorkspaceError> {
    let application_data_root = database_parent(database_path);
    let root = application_data_root.join("import-staging");
    prepare_target_parent(&application_data_root, &root.join("entry"))?;
    for suffix in 0..1_000 {
        let candidate = root.join(format!("zip-{}-{suffix}", unix_millis(SystemTime::now())));
        if !path_entry_exists(&candidate) {
            match fs::create_dir(&candidate) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
            let metadata = candidate.symlink_metadata()?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(WorkspaceError::InvalidFileOperation(
                    "创建 ZIP 暂存目录时检测到路径被替换。".to_owned(),
                ));
            }
            return Ok(candidate);
        }
    }
    Err(WorkspaceError::InvalidArchive(
        "无法创建 ZIP 临时解包目录。".to_owned(),
    ))
}

fn cleanup_staging_root(database_path: &Path, staging_root: &Path) -> Result<(), WorkspaceError> {
    let application_data_root = database_parent(database_path);
    let expected_parent = application_data_root.join("import-staging");
    if staging_root.parent() != Some(expected_parent.as_path()) {
        return Err(WorkspaceError::InvalidFileOperation(
            "ZIP 暂存目录不在应用管理范围内，已拒绝清理。".to_owned(),
        ));
    }
    validate_target_ancestors(&application_data_root, staging_root)?;
    remove_path_if_exists(staging_root)
}

fn application_backup_base(database_path: &Path) -> PathBuf {
    database_parent(database_path).join("file-operation-backups")
}

fn validate_application_backup_path(
    database_path: &Path,
    backup_path: &Path,
) -> Result<(), WorkspaceError> {
    let application_data_root = database_parent(database_path);
    let backup_base = application_backup_base(database_path);
    let relative = backup_path.strip_prefix(&backup_base).map_err(|_| {
        WorkspaceError::InvalidFileOperation("撤销备份不在应用管理目录内。".to_owned())
    })?;
    let components = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.len() != 3
        || !components[0].starts_with("batch-")
        || !components[1].starts_with("item-")
        || !matches!(components[2], "target" | "source")
    {
        return Err(WorkspaceError::InvalidFileOperation(
            "撤销备份路径格式无效。".to_owned(),
        ));
    }
    validate_target_ancestors(&application_data_root, backup_path)
}

fn cleanup_backup_root(database_path: &Path, backup_root: &Path) -> Result<(), WorkspaceError> {
    let application_data_root = database_parent(database_path);
    let backup_base = application_backup_base(database_path);
    if backup_root.parent() != Some(backup_base.as_path())
        || !backup_root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("batch-"))
    {
        return Err(WorkspaceError::InvalidFileOperation(
            "批次备份目录不在应用管理范围内。".to_owned(),
        ));
    }
    validate_target_ancestors(&application_data_root, backup_root)?;
    remove_path_if_exists(backup_root)
}

fn cleanup_backup_item(database_path: &Path, backup_item: &Path) -> Result<(), WorkspaceError> {
    let application_data_root = database_parent(database_path);
    let backup_base = application_backup_base(database_path);
    let relative = backup_item.strip_prefix(&backup_base).map_err(|_| {
        WorkspaceError::InvalidFileOperation("单项备份不在应用管理目录内。".to_owned())
    })?;
    let components = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();
    if components.len() != 2
        || !components[0].starts_with("batch-")
        || !components[1].starts_with("item-")
    {
        return Err(WorkspaceError::InvalidFileOperation(
            "单项备份路径格式无效。".to_owned(),
        ));
    }
    validate_target_ancestors(&application_data_root, backup_item)?;
    remove_path_if_exists(backup_item)
}

fn database_parent(database_path: &Path) -> PathBuf {
    database_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn extract_zip_safely(archive_path: &Path, destination: &Path) -> Result<PathBuf, WorkspaceError> {
    let file = fs::File::open(archive_path)
        .map_err(|error| WorkspaceError::InvalidArchive(format!("无法打开 ZIP：{error}")))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| WorkspaceError::InvalidArchive(format!("ZIP 格式无效：{error}")))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(WorkspaceError::InvalidArchive(
            "ZIP 文件条目过多。".to_owned(),
        ));
    }
    let mut entries = Vec::new();
    let mut total_size = 0u64;
    let mut seen = HashSet::new();
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| WorkspaceError::InvalidArchive(error.to_string()))?;
        let relative_path = safe_zip_entry_path(file.name())?;
        let key = relative_path.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            return Err(WorkspaceError::InvalidArchive(
                "ZIP 包含重复或大小写冲突路径。".to_owned(),
            ));
        }
        total_size = total_size.saturating_add(file.size());
        if total_size > MAX_ZIP_UNCOMPRESSED_BYTES {
            return Err(WorkspaceError::InvalidArchive(
                "ZIP 解包后超过 100 MB 限制。".to_owned(),
            ));
        }
        let mode = file.unix_mode().unwrap_or_default();
        entries.push((
            index,
            relative_path,
            file.is_dir(),
            mode & 0o170000 == 0o120000,
        ));
    }
    let symlink_paths = entries
        .iter()
        .filter(|entry| entry.3)
        .map(|entry| entry.1.clone())
        .collect::<Vec<_>>();
    if entries.iter().any(|entry| {
        symlink_paths
            .iter()
            .any(|symlink| entry.1 != *symlink && entry.1.starts_with(symlink))
    }) {
        return Err(WorkspaceError::InvalidArchive(
            "ZIP 文件路径经过符号链接。".to_owned(),
        ));
    }
    fs::create_dir_all(destination)?;
    for (index, relative_path, is_directory, is_symlink) in &entries {
        if *is_symlink {
            continue;
        }
        let target = destination.join(relative_path);
        if *is_directory {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut entry = archive
            .by_index(*index)
            .map_err(|error| WorkspaceError::InvalidArchive(error.to_string()))?;
        let mut output = fs::File::create(&target)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    for (index, relative_path, _, is_symlink) in &entries {
        if !*is_symlink {
            continue;
        }
        let mut entry = archive
            .by_index(*index)
            .map_err(|error| WorkspaceError::InvalidArchive(error.to_string()))?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        let link = String::from_utf8(bytes).map_err(|_| {
            WorkspaceError::InvalidArchive("ZIP 符号链接目标不是 UTF-8 文本。".to_owned())
        })?;
        let resolved = resolve_lexical_link(
            relative_path.parent().unwrap_or_else(|| Path::new("")),
            Path::new(&link),
        )?;
        if !entries
            .iter()
            .any(|candidate| candidate.1 == resolved || candidate.1.starts_with(&resolved))
        {
            return Err(WorkspaceError::InvalidArchive(
                "ZIP 包含越界符号链接。".to_owned(),
            ));
        }
        let target = destination.join(relative_path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        create_symlink(Path::new(&link), &target)?;
    }
    let mut skill_directories = Vec::new();
    find_skill_directories(destination, destination, &mut skill_directories)?;
    if skill_directories.len() != 1 {
        return Err(WorkspaceError::InvalidArchive(
            "ZIP 必须且只能包含一个带 SKILL.md 的本地 Skill。".to_owned(),
        ));
    }
    validate_selected_skill_links(&skill_directories[0])?;
    Ok(skill_directories.remove(0))
}

fn safe_zip_entry_path(name: &str) -> Result<PathBuf, WorkspaceError> {
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/')
        || normalized
            .split('/')
            .next()
            .is_some_and(|part| part.contains(':'))
    {
        return Err(WorkspaceError::InvalidArchive(
            "ZIP 包含绝对路径。".to_owned(),
        ));
    }
    let path = Path::new(&normalized);
    if path.as_os_str().is_empty()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(WorkspaceError::InvalidArchive(
            "ZIP 包含路径穿越。".to_owned(),
        ));
    }
    Ok(path
        .components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect())
}

fn resolve_lexical_link(parent: &Path, link: &Path) -> Result<PathBuf, WorkspaceError> {
    if link.is_absolute() {
        return Err(WorkspaceError::InvalidArchive(
            "ZIP 包含越界符号链接。".to_owned(),
        ));
    }
    let mut parts = parent
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for component in link.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => parts.push(value.to_owned()),
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(WorkspaceError::InvalidArchive(
                        "ZIP 包含越界符号链接。".to_owned(),
                    ));
                }
            }
            _ => {
                return Err(WorkspaceError::InvalidArchive(
                    "ZIP 包含越界符号链接。".to_owned(),
                ));
            }
        }
    }
    Ok(parts.into_iter().collect())
}

fn find_skill_directories(
    base: &Path,
    directory: &Path,
    found: &mut Vec<PathBuf>,
) -> Result<(), WorkspaceError> {
    if directory.join("SKILL.md").is_file() {
        found.push(directory.to_path_buf());
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = path.symlink_metadata()?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() && path.starts_with(base) {
            find_skill_directories(base, &path, found)?;
        }
    }
    Ok(())
}

fn validate_selected_skill_links(directory: &Path) -> Result<(), WorkspaceError> {
    fn visit(base: &Path, directory: &Path) -> Result<(), WorkspaceError> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let metadata = path.symlink_metadata()?;
            if metadata.file_type().is_symlink() {
                let relative = path.strip_prefix(base).unwrap_or(&path);
                let target = fs::read_link(&path)?;
                let resolved = resolve_lexical_link(
                    relative.parent().unwrap_or_else(|| Path::new("")),
                    &target,
                )?;
                if !base.join(resolved).starts_with(base) {
                    return Err(WorkspaceError::InvalidArchive(
                        "ZIP 包含越界符号链接。".to_owned(),
                    ));
                }
            } else if metadata.is_dir() {
                visit(base, &path)?;
            }
        }
        Ok(())
    }
    visit(directory, directory)
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> Result<(), WorkspaceError> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_symlink(_target: &Path, _link: &Path) -> Result<(), WorkspaceError> {
    Err(WorkspaceError::InvalidArchive(
        "当前系统不支持 ZIP 中的符号链接。".to_owned(),
    ))
}

fn operation_kind_database(kind: &FileOperationKind) -> &'static str {
    match kind {
        FileOperationKind::Import => "import",
        FileOperationKind::Copy => "copy",
        FileOperationKind::Move => "move",
        FileOperationKind::Trash => "trash",
    }
}

fn operation_kind_from_database(value: &str) -> FileOperationKind {
    match value {
        "import" => FileOperationKind::Import,
        "move" => FileOperationKind::Move,
        "trash" => FileOperationKind::Trash,
        _ => FileOperationKind::Copy,
    }
}
