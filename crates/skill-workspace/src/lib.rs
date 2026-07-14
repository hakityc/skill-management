//! 本地 Skill 管理的最高层应用接缝。

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillStatus {
    Ready,
    NeedsRepair,
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
                error TEXT
            );
            ",
        )?;
        migrate_workspace_index(&connection)?;
        Ok(Self { database_path })
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
        transaction.execute("DELETE FROM skill_instances WHERE root_id = ?1", [root_id])?;
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
            transaction.execute("DELETE FROM skill_instances WHERE root_id = ?1", [root_id])?;
            persist_instances(&transaction, &outcome.instances)?;
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
        let mut statement = connection.prepare(
            "
            SELECT id, root_id, name, description, relative_path, skill_file_path,
                   link_path, real_path, status, error
            FROM skill_instances
            ORDER BY root_id, relative_path
            ",
        )?;
        let instances = statement
            .query_map([], |row| {
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
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(WorkspaceSnapshot {
            authorized_root,
            roots,
            instances,
        })
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
                link_path, real_path, status, error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
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
            ],
        )?;
    }
    Ok(())
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
    ) || name.ends_with("_cache")
}

fn read_skill_instance(
    root_id: i64,
    root: &Path,
    directory: &Path,
    skill_file: &Path,
) -> SkillInstance {
    let parsed = match fs::read(skill_file) {
        Ok(content) => match String::from_utf8(content) {
            Ok(content) => parse_skill_document(&content),
            Err(_) => ParsedSkillDocument {
                error: Some("SKILL.md 不是有效的 UTF-8 文本".to_owned()),
                ..ParsedSkillDocument::default()
            },
        },
        Err(error) => ParsedSkillDocument {
            error: Some(format!("无法读取 SKILL.md：{error}")),
            ..ParsedSkillDocument::default()
        },
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
