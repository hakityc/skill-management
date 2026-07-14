//! 本地 Skill 管理的最高层应用接缝。

use std::{
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
pub struct SkillInstance {
    pub id: String,
    pub name: String,
    pub description: String,
    pub relative_path: String,
    pub skill_file_path: String,
    pub status: SkillStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub authorized_root: Option<String>,
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
            CREATE TABLE IF NOT EXISTS skill_instances (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                skill_file_path TEXT NOT NULL,
                status TEXT NOT NULL,
                error TEXT
            );
            ",
        )?;
        Ok(Self { database_path })
    }

    pub fn authorize_root(
        &self,
        root: impl AsRef<Path>,
    ) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|error| WorkspaceError::InvalidRoot(error.to_string()))?;
        if !root.is_dir() {
            return Err(WorkspaceError::InvalidRoot("选择的路径不是目录".to_owned()));
        }

        let mut instances = Vec::new();
        discover_skill_instances(&root, &root, &mut instances)?;
        instances.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        let snapshot = WorkspaceSnapshot {
            authorized_root: Some(root.to_string_lossy().into_owned()),
            instances,
        };
        self.persist(&snapshot)?;
        Ok(snapshot)
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
        let mut statement = connection.prepare(
            "
            SELECT id, name, description, relative_path, skill_file_path, status, error
            FROM skill_instances
            ORDER BY relative_path
            ",
        )?;
        let instances = statement
            .query_map([], |row| {
                let status: String = row.get(5)?;
                Ok(SkillInstance {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    relative_path: row.get(3)?,
                    skill_file_path: row.get(4)?,
                    status: SkillStatus::from_database(&status),
                    error: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(WorkspaceSnapshot {
            authorized_root,
            instances,
        })
    }

    fn persist(&self, snapshot: &WorkspaceSnapshot) -> Result<(), WorkspaceError> {
        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM skill_instances", [])?;
        if let Some(root) = &snapshot.authorized_root {
            transaction.execute(
                "
                INSERT INTO workspace_settings (key, value)
                VALUES ('authorized_root', ?1)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value
                ",
                [root],
            )?;
        }
        for skill in &snapshot.instances {
            transaction.execute(
                "
                INSERT INTO skill_instances (
                    id, name, description, relative_path, skill_file_path, status, error
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
                params![
                    skill.id,
                    skill.name,
                    skill.description,
                    skill.relative_path,
                    skill.skill_file_path,
                    skill.status.as_database(),
                    skill.error,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
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

fn discover_skill_instances(
    root: &Path,
    directory: &Path,
    instances: &mut Vec<SkillInstance>,
) -> Result<(), WorkspaceError> {
    let skill_file = directory.join("SKILL.md");
    if skill_file
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_file())
    {
        instances.push(read_skill_instance(root, directory, &skill_file));
        return Ok(());
    }

    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            discover_skill_instances(root, &entry.path(), instances)?;
        }
    }

    Ok(())
}

fn read_skill_instance(root: &Path, directory: &Path, skill_file: &Path) -> SkillInstance {
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
    let status = if parsed.error.is_some() {
        SkillStatus::NeedsRepair
    } else {
        SkillStatus::Ready
    };

    SkillInstance {
        id: relative_path.clone(),
        name: parsed.name.unwrap_or(fallback_name),
        description: parsed.description.unwrap_or_default(),
        relative_path,
        skill_file_path: skill_file.to_string_lossy().into_owned(),
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
