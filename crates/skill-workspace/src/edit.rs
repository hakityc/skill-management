use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::SystemTime,
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{
    SkillWorkspace, WorkspaceError, WorkspaceSnapshot, detail::safe_relative_path, unix_millis,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDraft {
    pub target: SkillDraftTarget,
    pub name: String,
    pub description: String,
    pub markdown_body: String,
    pub file_changes: Vec<SkillFileDraftChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SkillDraftTarget {
    Existing { instance_id: String },
    New { root_id: i64, relative_path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFileDraftChange {
    pub relative_path: String,
    pub operation: SkillFileDraftOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SkillFileDraftOperation {
    WriteText { content: String },
    ReplaceBinary { content: Vec<u8> },
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDraftValidation {
    pub valid: bool,
    pub issues: Vec<SkillValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillValidationIssue {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillChangePlan {
    pub id: i64,
    pub changes: Vec<SkillPlannedChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillChangeOutcome {
    pub operation_id: i64,
    pub snapshot: WorkspaceSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillChangeRecord {
    pub operation_id: i64,
    pub target_directory: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillPlannedChange {
    pub relative_path: String,
    pub kind: SkillChangeKind,
    pub binary: bool,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillChangeKind {
    Create,
    Overwrite,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingSkillChange {
    pub root_id: i64,
    pub instance_id: Option<String>,
    pub target_directory: String,
    pub baseline_fingerprint: Option<u64>,
    pub writes: Vec<PendingWrite>,
    pub deletes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingWrite {
    pub relative_path: String,
    pub content: Vec<u8>,
}

impl SkillWorkspace {
    pub fn validate_skill_draft(&self, draft: &SkillDraft) -> SkillDraftValidation {
        let mut issues = Vec::new();
        if draft.name.trim().is_empty() {
            issues.push(validation_issue("name", "Skill 名称不能为空。"));
        }
        if draft.description.trim().is_empty() {
            issues.push(validation_issue("description", "Skill 描述不能为空。"));
        }
        if let SkillDraftTarget::New { relative_path, .. } = &draft.target
            && safe_relative_path(relative_path).is_err()
        {
            issues.push(validation_issue(
                "relativePath",
                "新 Skill 路径必须位于所选 Skill 根目录内。",
            ));
        }

        let mut paths = HashSet::new();
        for change in &draft.file_changes {
            match safe_relative_path(&change.relative_path) {
                Err(_) => issues.push(validation_issue(
                    "fileChanges",
                    format!("文件路径不安全：{}。", change.relative_path),
                )),
                Ok(path) => {
                    let normalized = normalized_path(&path);
                    if normalized.eq_ignore_ascii_case("SKILL.md") {
                        issues.push(validation_issue(
                            "fileChanges",
                            "请通过元数据表单和 Markdown 编辑器修改 SKILL.md。",
                        ));
                    } else if !paths.insert(normalized) {
                        issues.push(validation_issue(
                            "fileChanges",
                            format!("同一文件不能重复修改：{}。", change.relative_path),
                        ));
                    }
                }
            }
        }

        SkillDraftValidation {
            valid: issues.is_empty(),
            issues,
        }
    }

    pub fn plan_skill_change(&self, draft: &SkillDraft) -> Result<SkillChangePlan, WorkspaceError> {
        let validation = self.validate_skill_draft(draft);
        if !validation.valid {
            return Err(WorkspaceError::InvalidDraft(
                validation
                    .issues
                    .iter()
                    .map(|issue| issue.message.as_str())
                    .collect::<Vec<_>>()
                    .join("；"),
            ));
        }
        let (root_id, instance_id, target_directory) = self.resolve_draft_target(&draft.target)?;
        let mut writes = Vec::new();
        let mut deletes = Vec::new();
        let mut changes = Vec::new();
        let skill_document = compose_skill_document(draft)?;
        plan_write(
            &target_directory,
            "SKILL.md",
            skill_document.into_bytes(),
            false,
            &mut writes,
            &mut changes,
        )?;

        for change in &draft.file_changes {
            let relative_path = safe_relative_path(&change.relative_path)?;
            let normalized = normalized_path(&relative_path);
            match &change.operation {
                SkillFileDraftOperation::WriteText { content } => plan_write(
                    &target_directory,
                    &normalized,
                    content.as_bytes().to_vec(),
                    false,
                    &mut writes,
                    &mut changes,
                )?,
                SkillFileDraftOperation::ReplaceBinary { content } => plan_write(
                    &target_directory,
                    &normalized,
                    content.clone(),
                    true,
                    &mut writes,
                    &mut changes,
                )?,
                SkillFileDraftOperation::Delete => {
                    let target = target_directory.join(&relative_path);
                    if target.symlink_metadata().is_ok() {
                        deletes.push(normalized.clone());
                        changes.push(SkillPlannedChange {
                            relative_path: normalized,
                            kind: SkillChangeKind::Delete,
                            binary: target.is_file() && !is_text_file(&target),
                            size: target
                                .metadata()
                                .map(|metadata| metadata.len())
                                .unwrap_or(0),
                        });
                    }
                }
            }
        }
        changes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let baseline_fingerprint = target_directory
            .exists()
            .then(|| directory_fingerprint(&target_directory))
            .transpose()?;
        let pending = PendingSkillChange {
            root_id,
            instance_id,
            target_directory: target_directory.to_string_lossy().into_owned(),
            baseline_fingerprint,
            writes,
            deletes,
        };
        let serialized = serde_json::to_string(&pending)?;
        let connection = Connection::open(&self.database_path)?;
        connection.execute(
            "INSERT INTO skill_change_plans (payload, created_at) VALUES (?1, ?2)",
            params![serialized, unix_millis(SystemTime::now())],
        )?;

        Ok(SkillChangePlan {
            id: connection.last_insert_rowid(),
            changes,
        })
    }

    pub fn execute_skill_change(&self, plan_id: i64) -> Result<SkillChangeOutcome, WorkspaceError> {
        let mut connection = Connection::open(&self.database_path)?;
        let serialized = connection
            .query_row(
                "SELECT payload FROM skill_change_plans WHERE id = ?1",
                [plan_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(WorkspaceError::UnknownChangePlan(plan_id))?;
        let pending: PendingSkillChange = serde_json::from_str(&serialized)?;
        let target = PathBuf::from(&pending.target_directory);
        let current_fingerprint = target
            .exists()
            .then(|| directory_fingerprint(&target))
            .transpose()?;
        if current_fingerprint != pending.baseline_fingerprint {
            return Err(WorkspaceError::StaleChangePlan);
        }

        let backup_root = self
            .database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("backups");
        fs::create_dir_all(&backup_root)?;
        let backup = pending.baseline_fingerprint.is_some().then(|| {
            backup_root.join(format!("plan-{plan_id}-{}", unix_millis(SystemTime::now())))
        });
        if let Some(backup) = &backup {
            copy_directory(&target, backup)?;
        }

        let stage = sibling_work_path(&target, "stage", plan_id)?;
        remove_path_if_exists(&stage)?;
        if target.exists() {
            copy_directory(&target, &stage)?;
        } else {
            fs::create_dir_all(&stage)?;
        }
        if let Err(error) = apply_pending_change(&stage, &pending) {
            let _ = remove_path_if_exists(&stage);
            return Err(error);
        }
        if let Err(error) = connection.execute(
            "
            INSERT INTO skill_change_operations (
                plan_id, root_id, target_directory, backup_directory,
                was_new, created_at, undone, completed
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0)
            ",
            params![
                plan_id,
                pending.root_id,
                pending.target_directory,
                backup
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                pending.baseline_fingerprint.is_none(),
                unix_millis(SystemTime::now()),
            ],
        ) {
            let _ = remove_path_if_exists(&stage);
            return Err(error.into());
        }
        let operation_id = connection.last_insert_rowid();
        if let Err(error) = atomic_replace_directory(&stage, &target, plan_id) {
            let _ = connection.execute(
                "DELETE FROM skill_change_operations WHERE id = ?1 AND completed = 0",
                [operation_id],
            );
            return Err(error);
        }

        let finalization = (|| -> Result<(), WorkspaceError> {
            let transaction = connection.transaction()?;
            transaction.execute(
                "UPDATE skill_change_operations SET completed = 1 WHERE id = ?1",
                [operation_id],
            )?;
            transaction.execute("DELETE FROM skill_change_plans WHERE id = ?1", [plan_id])?;
            transaction.commit()?;
            Ok(())
        })();
        if let Err(error) = finalization {
            let rollback = rollback_applied_change(
                &target,
                backup.as_deref(),
                pending.baseline_fingerprint.is_none(),
                operation_id,
            );
            let _ = connection.execute(
                "DELETE FROM skill_change_operations WHERE id = ?1 AND completed = 0",
                [operation_id],
            );
            if let Err(rollback_error) = rollback {
                return Err(WorkspaceError::InvalidDraft(format!(
                    "保存记录失败，且自动恢复失败：{error}；{rollback_error}"
                )));
            }
            return Err(error);
        }
        let snapshot = self.rescan_root(pending.root_id)?;
        Ok(SkillChangeOutcome {
            operation_id,
            snapshot,
        })
    }

    pub fn latest_undoable_skill_change(
        &self,
    ) -> Result<Option<SkillChangeRecord>, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        connection
            .query_row(
                "
                SELECT id, target_directory, created_at
                FROM skill_change_operations
                WHERE completed = 1 AND undone = 0
                ORDER BY created_at DESC, id DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok(SkillChangeRecord {
                        operation_id: row.get(0)?,
                        target_directory: row.get(1)?,
                        created_at: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(WorkspaceError::from)
    }

    pub fn undo_skill_change(
        &self,
        operation_id: i64,
    ) -> Result<SkillChangeOutcome, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let operation = connection
            .query_row(
                "
                SELECT root_id, target_directory, backup_directory, was_new, undone, undoing
                FROM skill_change_operations
                WHERE id = ?1 AND completed = 1
                ",
                [operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, bool>(3)?,
                        row.get::<_, bool>(4)?,
                        row.get::<_, bool>(5)?,
                    ))
                },
            )
            .optional()?
            .ok_or(WorkspaceError::UnknownChangeOperation(operation_id))?;
        let (root_id, target_directory, backup_directory, was_new, undone, _undoing) = operation;
        if undone {
            return Err(WorkspaceError::ChangeAlreadyUndone);
        }
        connection.execute(
            "UPDATE skill_change_operations SET undoing = 1 WHERE id = ?1",
            [operation_id],
        )?;
        drop(connection);
        apply_undo(
            &PathBuf::from(target_directory),
            backup_directory.as_deref().map(Path::new),
            was_new,
            operation_id,
        )?;
        let snapshot = self.rescan_root(root_id)?;
        let connection = Connection::open(&self.database_path)?;
        connection.execute(
            "UPDATE skill_change_operations SET undone = 1, undoing = 0 WHERE id = ?1",
            [operation_id],
        )?;
        Ok(SkillChangeOutcome {
            operation_id,
            snapshot,
        })
    }

    pub(crate) fn recover_interrupted_changes(&self) -> Result<(), WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let mut statement = connection.prepare(
            "
            SELECT id, root_id, target_directory, backup_directory,
                   was_new, completed, undone, undoing
            FROM skill_change_operations
            WHERE completed = 0 OR (completed = 1 AND undone = 0 AND undoing = 1)
            ORDER BY id
            ",
        )?;
        let operations = statement
            .query_map([], |row| {
                Ok(RecoverableOperation {
                    id: row.get(0)?,
                    root_id: row.get(1)?,
                    target_directory: row.get(2)?,
                    backup_directory: row.get(3)?,
                    was_new: row.get(4)?,
                    completed: row.get(5)?,
                    undone: row.get(6)?,
                    undoing: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);

        for operation in operations {
            let target = PathBuf::from(&operation.target_directory);
            let backup = operation.backup_directory.as_deref().map(Path::new);
            if !operation.completed {
                rollback_applied_change(&target, backup, operation.was_new, operation.id)?;
                self.rescan_root(operation.root_id)?;
                let connection = Connection::open(&self.database_path)?;
                connection.execute(
                    "DELETE FROM skill_change_operations WHERE id = ?1 AND completed = 0",
                    [operation.id],
                )?;
            } else if !operation.undone && operation.undoing {
                apply_undo(&target, backup, operation.was_new, operation.id)?;
                self.rescan_root(operation.root_id)?;
                let connection = Connection::open(&self.database_path)?;
                connection.execute(
                    "UPDATE skill_change_operations SET undone = 1, undoing = 0 WHERE id = ?1",
                    [operation.id],
                )?;
            }
        }
        Ok(())
    }

    fn resolve_draft_target(
        &self,
        target: &SkillDraftTarget,
    ) -> Result<(i64, Option<String>, PathBuf), WorkspaceError> {
        let snapshot = self.snapshot()?;
        match target {
            SkillDraftTarget::Existing { instance_id } => {
                let instance = snapshot
                    .instances
                    .into_iter()
                    .find(|instance| instance.id == *instance_id)
                    .ok_or_else(|| WorkspaceError::UnknownInstance(instance_id.clone()))?;
                Ok((
                    instance.root_id,
                    Some(instance.id),
                    PathBuf::from(instance.real_path),
                ))
            }
            SkillDraftTarget::New {
                root_id,
                relative_path,
            } => {
                let root = snapshot
                    .roots
                    .into_iter()
                    .find(|root| root.id == *root_id)
                    .ok_or_else(|| {
                        WorkspaceError::InvalidRoot("找不到所选 Skill 根目录".to_owned())
                    })?;
                let directory = PathBuf::from(root.path).join(safe_relative_path(relative_path)?);
                if directory.exists() {
                    return Err(WorkspaceError::InvalidDraft(
                        "新 Skill 的目标目录已经存在。".to_owned(),
                    ));
                }
                Ok((*root_id, None, directory))
            }
        }
    }
}

struct RecoverableOperation {
    id: i64,
    root_id: i64,
    target_directory: String,
    backup_directory: Option<String>,
    was_new: bool,
    completed: bool,
    undone: bool,
    undoing: bool,
}

fn validation_issue(field: &str, message: impl Into<String>) -> SkillValidationIssue {
    SkillValidationIssue {
        field: field.to_owned(),
        message: message.into(),
    }
}

#[derive(Serialize)]
struct DraftSkillMetadata<'a> {
    name: &'a str,
    description: &'a str,
}

fn compose_skill_document(draft: &SkillDraft) -> Result<String, WorkspaceError> {
    let metadata = serde_yaml::to_string(&DraftSkillMetadata {
        name: draft.name.trim(),
        description: draft.description.trim(),
    })?;
    Ok(format!(
        "---\n{metadata}---\n\n{}",
        draft.markdown_body.trim_start()
    ))
}

fn plan_write(
    directory: &Path,
    relative_path: &str,
    content: Vec<u8>,
    binary: bool,
    writes: &mut Vec<PendingWrite>,
    changes: &mut Vec<SkillPlannedChange>,
) -> Result<(), WorkspaceError> {
    let target = directory.join(safe_relative_path(relative_path)?);
    let current = fs::read(&target).ok();
    if current.as_deref() == Some(content.as_slice()) {
        return Ok(());
    }
    let kind = if current.is_some() {
        SkillChangeKind::Overwrite
    } else {
        SkillChangeKind::Create
    };
    changes.push(SkillPlannedChange {
        relative_path: relative_path.to_owned(),
        kind,
        binary,
        size: content.len() as u64,
    });
    writes.push(PendingWrite {
        relative_path: relative_path.to_owned(),
        content,
    });
    Ok(())
}

fn normalized_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_text_file(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .is_some_and(|content| std::str::from_utf8(&content).is_ok())
}

fn directory_fingerprint(directory: &Path) -> Result<u64, WorkspaceError> {
    let mut hasher = DefaultHasher::new();
    fingerprint_directory(directory, directory, &mut hasher)?;
    Ok(hasher.finish())
}

fn fingerprint_directory(
    base: &Path,
    directory: &Path,
    hasher: &mut DefaultHasher,
) -> Result<(), WorkspaceError> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = path.symlink_metadata()?;
        normalized_path(path.strip_prefix(base).unwrap_or(&path)).hash(hasher);
        if metadata.file_type().is_symlink() {
            "symlink".hash(hasher);
            fs::read_link(&path)?.hash(hasher);
        } else if metadata.is_dir() {
            "directory".hash(hasher);
            fingerprint_directory(base, &path, hasher)?;
        } else {
            "file".hash(hasher);
            fs::read(path)?.hash(hasher);
        }
    }
    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), WorkspaceError> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = source_path.symlink_metadata()?;
        if metadata.file_type().is_symlink() {
            copy_symbolic_link(&source_path, &destination_path)?;
        } else if metadata.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
            fs::set_permissions(&destination_path, metadata.permissions())?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symbolic_link(source: &Path, destination: &Path) -> Result<(), WorkspaceError> {
    std::os::unix::fs::symlink(fs::read_link(source)?, destination)?;
    Ok(())
}

#[cfg(not(unix))]
fn copy_symbolic_link(_source: &Path, _destination: &Path) -> Result<(), WorkspaceError> {
    Err(WorkspaceError::InvalidDraft(
        "当前系统暂不支持复制 Skill 中的符号链接。".to_owned(),
    ))
}

fn apply_pending_change(stage: &Path, pending: &PendingSkillChange) -> Result<(), WorkspaceError> {
    for relative_path in &pending.deletes {
        let target = stage.join(safe_relative_path(relative_path)?);
        remove_path_if_exists(&target)?;
    }
    for write in &pending.writes {
        let relative_path = safe_relative_path(&write.relative_path)?;
        ensure_safe_parent(stage, &relative_path)?;
        let target = stage.join(relative_path);
        remove_path_if_exists(&target)?;
        fs::write(target, &write.content)?;
    }
    Ok(())
}

fn ensure_safe_parent(stage: &Path, relative_path: &Path) -> Result<(), WorkspaceError> {
    let mut current = stage.to_path_buf();
    if let Some(parent) = relative_path.parent() {
        for component in parent.components() {
            current.push(component.as_os_str());
            match current.symlink_metadata() {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(WorkspaceError::InvalidSkillPath(
                        "写入路径经过符号链接，已取消操作".to_owned(),
                    ));
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(WorkspaceError::InvalidSkillPath(
                        "写入路径的父级不是目录".to_owned(),
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    fs::create_dir(&current)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

fn sibling_work_path(
    target: &Path,
    purpose: &str,
    identifier: i64,
) -> Result<PathBuf, WorkspaceError> {
    let parent = target
        .parent()
        .ok_or_else(|| WorkspaceError::InvalidSkillPath("Skill 目标目录缺少父目录".to_owned()))?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("skill");
    Ok(parent.join(format!(".skill-management-{purpose}-{identifier}-{name}")))
}

fn atomic_replace_directory(
    stage: &Path,
    target: &Path,
    identifier: i64,
) -> Result<(), WorkspaceError> {
    if target.symlink_metadata().is_err() {
        fs::rename(stage, target)?;
        return Ok(());
    }
    let old = sibling_work_path(target, "old", identifier)?;
    remove_path_if_exists(&old)?;
    fs::rename(target, &old)?;
    if let Err(error) = fs::rename(stage, target) {
        let _ = fs::rename(&old, target);
        return Err(error.into());
    }
    let _ = remove_path_if_exists(&old);
    Ok(())
}

fn rollback_applied_change(
    target: &Path,
    backup: Option<&Path>,
    was_new: bool,
    operation_id: i64,
) -> Result<(), WorkspaceError> {
    if was_new {
        return remove_path_if_exists(target);
    }
    let backup = backup.ok_or_else(|| {
        WorkspaceError::InvalidDraft("保存失败后找不到可恢复的本地备份。".to_owned())
    })?;
    let stage = sibling_work_path(target, "rollback", operation_id)?;
    remove_path_if_exists(&stage)?;
    copy_directory(backup, &stage)?;
    atomic_replace_directory(&stage, target, operation_id)
}

fn apply_undo(
    target: &Path,
    backup: Option<&Path>,
    was_new: bool,
    operation_id: i64,
) -> Result<(), WorkspaceError> {
    if was_new {
        let tombstone = sibling_work_path(target, "undo", operation_id)?;
        if target.symlink_metadata().is_ok() {
            remove_path_if_exists(&tombstone)?;
            fs::rename(target, &tombstone)?;
        }
        return remove_path_if_exists(&tombstone);
    }
    let backup = backup
        .ok_or_else(|| WorkspaceError::InvalidDraft("编辑操作的本地备份不存在。".to_owned()))?;
    let stage = sibling_work_path(target, "undo", operation_id)?;
    remove_path_if_exists(&stage)?;
    copy_directory(backup, &stage)?;
    atomic_replace_directory(&stage, target, operation_id)
}

fn remove_path_if_exists(path: &Path) -> Result<(), WorkspaceError> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}
