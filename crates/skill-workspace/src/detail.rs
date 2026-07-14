use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{SkillInstance, SkillRoot, SkillWorkspace, WorkspaceError, unix_millis};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    pub instance: SkillInstance,
    pub root: SkillRoot,
    pub tags: Vec<String>,
    pub skill_groups: Vec<String>,
    pub files: Vec<SkillFileEntry>,
    pub file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFileEntry {
    pub relative_path: String,
    pub kind: SkillFileKind,
    pub size: u64,
    pub modified_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillFileKind {
    Directory,
    Text,
    Binary,
    SymbolicLink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SkillFilePreview {
    Text {
        content: String,
    },
    Binary {
        size: u64,
        media_type: Option<String>,
        preview_content: Option<Vec<u8>>,
    },
}

impl SkillWorkspace {
    pub fn skill_detail(&self, instance_id: &str) -> Result<SkillDetail, WorkspaceError> {
        let snapshot = self.snapshot()?;
        let instance = snapshot
            .instances
            .into_iter()
            .find(|instance| instance.id == instance_id)
            .ok_or_else(|| WorkspaceError::UnknownInstance(instance_id.to_owned()))?;
        let root = snapshot
            .roots
            .into_iter()
            .find(|root| root.id == instance.root_id)
            .ok_or_else(|| WorkspaceError::InvalidRoot("Skill 实例缺少根目录".to_owned()))?;
        let base = PathBuf::from(&instance.real_path);
        let mut files = Vec::new();
        collect_file_entries(&base, &base, &mut files)?;
        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        let file_count = files
            .iter()
            .filter(|entry| !matches!(entry.kind, SkillFileKind::Directory))
            .count();
        let (tags, skill_groups) = self.skill_tags_and_groups(instance_id)?;

        Ok(SkillDetail {
            instance,
            root,
            tags,
            skill_groups,
            files,
            file_count,
        })
    }

    pub fn read_skill_file(
        &self,
        instance_id: &str,
        relative_path: &str,
    ) -> Result<SkillFilePreview, WorkspaceError> {
        let detail = self.skill_detail(instance_id)?;
        let base = PathBuf::from(&detail.instance.real_path).canonicalize()?;
        let requested = safe_relative_path(relative_path)?;
        let target = base
            .join(requested)
            .canonicalize()
            .map_err(|error| WorkspaceError::InvalidSkillPath(format!("文件不可访问：{error}")))?;
        if !target.starts_with(&base) {
            return Err(WorkspaceError::InvalidSkillPath(
                "路径指向 Skill 目录之外".to_owned(),
            ));
        }
        if !target.is_file() {
            return Err(WorkspaceError::InvalidSkillPath(
                "所选路径不是文件".to_owned(),
            ));
        }
        let bytes = fs::read(&target)?;
        match String::from_utf8(bytes) {
            Ok(content) => Ok(SkillFilePreview::Text { content }),
            Err(error) => {
                let bytes = error.into_bytes();
                let media_type = previewable_media_type(&bytes).map(str::to_owned);
                let preview_content = (media_type.is_some() && bytes.len() <= 5 * 1024 * 1024)
                    .then_some(bytes.clone());
                Ok(SkillFilePreview::Binary {
                    size: bytes.len() as u64,
                    media_type,
                    preview_content,
                })
            }
        }
    }

    fn skill_tags_and_groups(
        &self,
        instance_id: &str,
    ) -> Result<(Vec<String>, Vec<String>), WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let stored = connection
            .query_row(
                "SELECT tags, skill_groups FROM skill_tags_and_groups WHERE instance_id = ?1",
                [instance_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let (tags, groups) = stored.unwrap_or_default();
        Ok((split_terms(&tags), split_terms(&groups)))
    }
}

fn collect_file_entries(
    base: &Path,
    directory: &Path,
    entries: &mut Vec<SkillFileEntry>,
) -> Result<(), WorkspaceError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = path.symlink_metadata()?;
        let relative_path = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let kind = if metadata.file_type().is_symlink() {
            SkillFileKind::SymbolicLink
        } else if metadata.is_dir() {
            SkillFileKind::Directory
        } else if is_probably_text(&path)? {
            SkillFileKind::Text
        } else {
            SkillFileKind::Binary
        };
        entries.push(SkillFileEntry {
            relative_path,
            kind: kind.clone(),
            size: metadata.len(),
            modified_at: metadata.modified().map(unix_millis).unwrap_or_default(),
        });
        if matches!(kind, SkillFileKind::Directory) {
            collect_file_entries(base, &path, entries)?;
        }
    }
    Ok(())
}

fn is_probably_text(path: &Path) -> io::Result<bool> {
    let mut sample = Vec::new();
    fs::File::open(path)?
        .take(8 * 1024)
        .read_to_end(&mut sample)?;
    Ok(!sample.contains(&0) && std::str::from_utf8(&sample).is_ok())
}

pub(crate) fn safe_relative_path(path: &str) -> Result<PathBuf, WorkspaceError> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err(WorkspaceError::InvalidSkillPath(
            "路径指向 Skill 目录之外".to_owned(),
        ));
    }
    Ok(path.to_path_buf())
}

fn previewable_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn split_terms(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}
