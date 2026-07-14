use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::Path,
    time::SystemTime,
};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::{
    DuplicateCheckStatus, DuplicateCheckStatusUpdate, SkillClient, SkillInstance, SkillStatus,
    SkillWorkspace, WorkspaceError, is_ignored_directory, unix_millis,
};

pub const DUPLICATE_SIMILARITY_THRESHOLD: f64 = 0.82;
const MAX_TEXT_DIFF_LINES: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReview {
    pub groups: Vec<DuplicateGroup>,
    pub suppressed_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub id: String,
    pub name: String,
    pub status: DuplicateCheckStatus,
    pub similarity: f64,
    pub hit_rules: Vec<DuplicateHitRule>,
    pub fingerprint_files: Vec<String>,
    pub instances: Vec<DuplicateReviewInstance>,
    pub comparisons: Vec<DuplicateComparison>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReviewInstance {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: String,
    pub client: SkillClient,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateComparison {
    pub left_instance_id: String,
    pub right_instance_id: String,
    pub status: DuplicateCheckStatus,
    pub similarity: f64,
    pub hit_rules: Vec<DuplicateHitRule>,
    pub files: Vec<DuplicateFileDifference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateHitRule {
    ExactContent,
    NormalizedName,
    ContentSimilarity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFileDifference {
    pub relative_path: String,
    pub status: DuplicateFileDifferenceStatus,
    pub kind: DuplicateFileKind,
    pub left_node_kind: Option<DuplicateFileNodeKind>,
    pub right_node_kind: Option<DuplicateFileNodeKind>,
    pub left_size: Option<u64>,
    pub right_size: Option<u64>,
    pub left_fingerprint: Option<String>,
    pub right_fingerprint: Option<String>,
    pub text_diff: Option<Vec<DuplicateTextDiffLine>>,
    pub text_diff_truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateFileDifferenceStatus {
    Identical,
    Modified,
    OnlyLeft,
    OnlyRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateFileKind {
    Text,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateFileNodeKind {
    File,
    SymbolicLink,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateTextDiffLine {
    pub kind: DuplicateTextDiffLineKind,
    pub left_line_number: Option<usize>,
    pub right_line_number: Option<usize>,
    pub left: Option<String>,
    pub right: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateTextDiffLineKind {
    Equal,
    Modified,
    OnlyLeft,
    OnlyRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateDecisionKind {
    NotDuplicate,
    Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateDecisionRecord {
    pub id: i64,
    pub instance_ids: Vec<String>,
    pub kind: DuplicateDecisionKind,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EffectiveFile {
    bytes: Vec<u8>,
    kind: DuplicateFileKind,
    node_kind: DuplicateFileNodeKind,
}

struct InstanceContent {
    instance: SkillInstance,
    files: BTreeMap<String, EffectiveFile>,
    similarity_bigrams: HashMap<(char, char), usize>,
    similarity_bigram_count: usize,
}

struct CandidatePair {
    left: usize,
    right: usize,
    comparison: DuplicateComparison,
}

struct PairClassification {
    status: DuplicateCheckStatus,
    similarity: f64,
    hit_rules: Vec<DuplicateHitRule>,
}

impl SkillWorkspace {
    pub fn review_duplicate_groups(&self) -> Result<DuplicateReview, WorkspaceError> {
        let snapshot = self.snapshot()?;
        let contents = snapshot
            .instances
            .iter()
            .filter(|instance| instance.status == SkillStatus::Ready)
            .map(read_instance_content)
            .collect::<Result<Vec<_>, _>>()?;
        let decisions = self.duplicate_decisions()?;
        let suppressed_pairs = suppressed_pairs(&decisions);
        let mut candidates = Vec::new();
        for left in 0..contents.len() {
            for right in (left + 1)..contents.len() {
                let classification = classify_pair(&contents[left], &contents[right]);
                if classification.status != DuplicateCheckStatus::None
                    && !suppressed_pairs.contains(&pair_key(
                        &contents[left].instance.id,
                        &contents[right].instance.id,
                    ))
                {
                    candidates.push(CandidatePair {
                        left,
                        right,
                        comparison: build_comparison(
                            &contents[left],
                            &contents[right],
                            classification,
                        ),
                    });
                }
            }
        }
        let groups = build_groups(&contents, candidates);
        self.persist_duplicate_statuses(&snapshot.instances, &groups)?;
        Ok(DuplicateReview {
            groups,
            suppressed_count: decisions.len(),
        })
    }

    pub fn compare_skill_instances(
        &self,
        left_instance_id: &str,
        right_instance_id: &str,
    ) -> Result<DuplicateComparison, WorkspaceError> {
        let snapshot = self.snapshot()?;
        let left = snapshot
            .instances
            .iter()
            .find(|instance| instance.id == left_instance_id)
            .ok_or_else(|| WorkspaceError::UnknownInstance(left_instance_id.to_owned()))?;
        let right = snapshot
            .instances
            .iter()
            .find(|instance| instance.id == right_instance_id)
            .ok_or_else(|| WorkspaceError::UnknownInstance(right_instance_id.to_owned()))?;
        Ok(compare_contents(
            &read_instance_content(left)?,
            &read_instance_content(right)?,
        ))
    }

    pub fn save_duplicate_decision(
        &self,
        instance_ids: &[String],
        kind: DuplicateDecisionKind,
    ) -> Result<(), WorkspaceError> {
        let mut instance_ids = instance_ids.to_vec();
        instance_ids.sort();
        instance_ids.dedup();
        if instance_ids.len() < 2 {
            return Err(WorkspaceError::InvalidDraft(
                "重复检查裁决至少需要两个 Skill 实例。".to_owned(),
            ));
        }
        let snapshot = self.snapshot()?;
        for instance_id in &instance_ids {
            if !snapshot
                .instances
                .iter()
                .any(|instance| instance.id == *instance_id)
            {
                return Err(WorkspaceError::UnknownInstance(instance_id.clone()));
            }
        }
        let serialized = serde_json::to_string(&instance_ids)?;
        let connection = Connection::open(&self.database_path)?;
        connection.execute(
            "
            INSERT INTO duplicate_decisions (group_key, instance_ids, kind, created_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(group_key) DO UPDATE SET
                instance_ids = excluded.instance_ids,
                kind = excluded.kind,
                created_at = excluded.created_at
            ",
            params![
                group_key(&instance_ids),
                serialized,
                decision_kind_database(&kind),
                unix_millis(SystemTime::now()),
            ],
        )?;
        self.review_duplicate_groups()?;
        Ok(())
    }

    pub fn duplicate_decisions(&self) -> Result<Vec<DuplicateDecisionRecord>, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let mut statement = connection.prepare(
            "SELECT id, instance_ids, kind, created_at FROM duplicate_decisions ORDER BY created_at DESC, id DESC",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .map(|row| {
                let (id, serialized, kind, created_at) = row?;
                Ok(DuplicateDecisionRecord {
                    id,
                    instance_ids: serde_json::from_str(&serialized)?,
                    kind: decision_kind_from_database(&kind),
                    created_at,
                })
            })
            .collect()
    }

    pub fn restore_duplicate_decision(&self, decision_id: i64) -> Result<(), WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        let changed = connection.execute(
            "DELETE FROM duplicate_decisions WHERE id = ?1",
            [decision_id],
        )?;
        if changed == 0 {
            return Err(WorkspaceError::InvalidDraft(
                "找不到要恢复的重复检查裁决。".to_owned(),
            ));
        }
        self.review_duplicate_groups()?;
        Ok(())
    }

    fn persist_duplicate_statuses(
        &self,
        instances: &[SkillInstance],
        groups: &[DuplicateGroup],
    ) -> Result<(), WorkspaceError> {
        let mut statuses = instances
            .iter()
            .map(|instance| (instance.id.clone(), DuplicateCheckStatus::None))
            .collect::<HashMap<_, _>>();
        for group in groups {
            for instance in &group.instances {
                let current = statuses
                    .entry(instance.id.clone())
                    .or_insert(DuplicateCheckStatus::None);
                if duplicate_status_priority(&group.status) > duplicate_status_priority(current) {
                    *current = group.status.clone();
                }
            }
        }
        let updates = statuses
            .into_iter()
            .map(|(instance_id, status)| DuplicateCheckStatusUpdate {
                instance_id,
                status,
            })
            .collect::<Vec<_>>();
        self.save_duplicate_check_statuses(&updates)
    }
}

fn read_instance_content(instance: &SkillInstance) -> Result<InstanceContent, WorkspaceError> {
    let base = Path::new(&instance.real_path);
    let mut files = BTreeMap::new();
    collect_files(base, base, &mut files, false)?;
    let similarity_bigrams = bigram_counts(&similarity_document(&files));
    let similarity_bigram_count = similarity_bigrams.values().sum();
    Ok(InstanceContent {
        instance: instance.clone(),
        files,
        similarity_bigrams,
        similarity_bigram_count,
    })
}

fn collect_files(
    base: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, EffectiveFile>,
    include_ignored: bool,
) -> Result<(), WorkspaceError> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let metadata = path.symlink_metadata()?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            if include_ignored || !is_ignored_directory(&name) {
                collect_files(base, &path, files, include_ignored)?;
            }
            continue;
        }
        if !include_ignored && is_ignored_file(&name) {
            continue;
        }
        let relative_path = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let is_symbolic_link = metadata.file_type().is_symlink();
        let bytes = if is_symbolic_link {
            fs::read_link(&path)?
                .to_string_lossy()
                .into_owned()
                .into_bytes()
        } else {
            fs::read(&path)?
        };
        let kind = if !bytes.contains(&0) && std::str::from_utf8(&bytes).is_ok() {
            DuplicateFileKind::Text
        } else {
            DuplicateFileKind::Binary
        };
        files.insert(
            relative_path,
            EffectiveFile {
                bytes,
                kind,
                node_kind: if is_symbolic_link {
                    DuplicateFileNodeKind::SymbolicLink
                } else {
                    DuplicateFileNodeKind::File
                },
            },
        );
    }
    Ok(())
}

pub(crate) fn compare_directory_trees_for_merge(
    left: &Path,
    right: &Path,
) -> Result<Vec<DuplicateFileDifference>, WorkspaceError> {
    let mut left_files = BTreeMap::new();
    let mut right_files = BTreeMap::new();
    collect_files(left, left, &mut left_files, true)?;
    collect_files(right, right, &mut right_files, true)?;
    Ok(compare_files(&left_files, &right_files))
}

fn is_ignored_file(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        matches!(
            name,
            ".DS_Store" | "Thumbs.db" | "desktop.ini" | ".localized"
        ) || name.starts_with("._")
    })
}

fn compare_contents(left: &InstanceContent, right: &InstanceContent) -> DuplicateComparison {
    let classification = classify_pair(left, right);
    build_comparison(left, right, classification)
}

fn classify_pair(left: &InstanceContent, right: &InstanceContent) -> PairClassification {
    let exact = left.files == right.files;
    let raw_similarity = if exact {
        1.0
    } else {
        dice_bigram_similarity(
            &left.similarity_bigrams,
            left.similarity_bigram_count,
            &right.similarity_bigrams,
            right.similarity_bigram_count,
        )
    };
    let same_name = normalized_name(&left.instance.name) == normalized_name(&right.instance.name);
    let status = if exact {
        DuplicateCheckStatus::Exact
    } else if same_name && raw_similarity < DUPLICATE_SIMILARITY_THRESHOLD {
        DuplicateCheckStatus::NameConflict
    } else if same_name || raw_similarity >= DUPLICATE_SIMILARITY_THRESHOLD {
        DuplicateCheckStatus::Suspected
    } else {
        DuplicateCheckStatus::None
    };
    let mut hit_rules = Vec::new();
    if exact {
        hit_rules.push(DuplicateHitRule::ExactContent);
    }
    if same_name {
        hit_rules.push(DuplicateHitRule::NormalizedName);
    }
    if raw_similarity >= DUPLICATE_SIMILARITY_THRESHOLD && !exact {
        hit_rules.push(DuplicateHitRule::ContentSimilarity);
    }
    PairClassification {
        status,
        similarity: raw_similarity,
        hit_rules,
    }
}

fn build_comparison(
    left: &InstanceContent,
    right: &InstanceContent,
    classification: PairClassification,
) -> DuplicateComparison {
    DuplicateComparison {
        left_instance_id: left.instance.id.clone(),
        right_instance_id: right.instance.id.clone(),
        status: classification.status,
        similarity: classification.similarity,
        hit_rules: classification.hit_rules,
        files: compare_files(&left.files, &right.files),
    }
}

fn similarity_document(files: &BTreeMap<String, EffectiveFile>) -> String {
    let mut document = String::new();
    for (path, file) in files {
        document.push_str(&path.to_lowercase());
        document.push('\n');
        match file.kind {
            DuplicateFileKind::Text => {
                let text = String::from_utf8_lossy(&file.bytes);
                for word in text.to_lowercase().split_whitespace() {
                    document.push_str(word);
                    document.push(' ');
                }
            }
            DuplicateFileKind::Binary => document.push_str(&stable_fingerprint(&file.bytes)),
        }
        document.push('\n');
    }
    document
}

fn dice_bigram_similarity(
    left: &HashMap<(char, char), usize>,
    left_count: usize,
    right: &HashMap<(char, char), usize>,
    right_count: usize,
) -> f64 {
    if left_count + right_count == 0 {
        return 0.0;
    }
    let intersection = left
        .iter()
        .map(|(bigram, count)| count.min(right.get(bigram).unwrap_or(&0)))
        .sum::<usize>();
    (2 * intersection) as f64 / (left_count + right_count) as f64
}

fn bigram_counts(value: &str) -> HashMap<(char, char), usize> {
    let characters = value.chars().collect::<Vec<_>>();
    let mut counts = HashMap::new();
    for window in characters.windows(2) {
        *counts.entry((window[0], window[1])).or_insert(0) += 1;
    }
    counts
}

fn compare_files(
    left: &BTreeMap<String, EffectiveFile>,
    right: &BTreeMap<String, EffectiveFile>,
) -> Vec<DuplicateFileDifference> {
    let paths = left
        .keys()
        .chain(right.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    paths
        .into_iter()
        .map(|relative_path| {
            let left_file = left.get(&relative_path);
            let right_file = right.get(&relative_path);
            let status = match (left_file, right_file) {
                (Some(left), Some(right)) if left == right => {
                    DuplicateFileDifferenceStatus::Identical
                }
                (Some(_), Some(_)) => DuplicateFileDifferenceStatus::Modified,
                (Some(_), None) => DuplicateFileDifferenceStatus::OnlyLeft,
                (None, Some(_)) => DuplicateFileDifferenceStatus::OnlyRight,
                (None, None) => unreachable!(),
            };
            let kind = if left_file
                .into_iter()
                .chain(right_file)
                .all(|file| file.kind == DuplicateFileKind::Text)
            {
                DuplicateFileKind::Text
            } else {
                DuplicateFileKind::Binary
            };
            let (text_diff, text_diff_truncated) = if kind == DuplicateFileKind::Text
                && status != DuplicateFileDifferenceStatus::Identical
            {
                let (diff, truncated) = readable_text_diff(
                    left_file.map(|file| String::from_utf8_lossy(&file.bytes)),
                    right_file.map(|file| String::from_utf8_lossy(&file.bytes)),
                );
                (Some(diff), truncated)
            } else {
                (None, false)
            };
            DuplicateFileDifference {
                relative_path,
                status,
                kind,
                left_node_kind: left_file.map(|file| file.node_kind.clone()),
                right_node_kind: right_file.map(|file| file.node_kind.clone()),
                left_size: left_file.map(|file| file.bytes.len() as u64),
                right_size: right_file.map(|file| file.bytes.len() as u64),
                left_fingerprint: left_file.map(|file| stable_fingerprint(&file.bytes)),
                right_fingerprint: right_file.map(|file| stable_fingerprint(&file.bytes)),
                text_diff,
                text_diff_truncated,
            }
        })
        .collect()
}

fn readable_text_diff(
    left: Option<std::borrow::Cow<'_, str>>,
    right: Option<std::borrow::Cow<'_, str>>,
) -> (Vec<DuplicateTextDiffLine>, bool) {
    let mut left = left
        .as_deref()
        .unwrap_or_default()
        .lines()
        .take(MAX_TEXT_DIFF_LINES + 1)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut right = right
        .as_deref()
        .unwrap_or_default()
        .lines()
        .take(MAX_TEXT_DIFF_LINES + 1)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let truncated = left.len() > MAX_TEXT_DIFF_LINES || right.len() > MAX_TEXT_DIFF_LINES;
    left.truncate(MAX_TEXT_DIFF_LINES);
    right.truncate(MAX_TEXT_DIFF_LINES);
    let mut lengths = vec![vec![0usize; right.len() + 1]; left.len() + 1];
    for left_index in (0..left.len()).rev() {
        for right_index in (0..right.len()).rev() {
            lengths[left_index][right_index] = if left[left_index] == right[right_index] {
                1 + lengths[left_index + 1][right_index + 1]
            } else {
                lengths[left_index + 1][right_index].max(lengths[left_index][right_index + 1])
            };
        }
    }
    let mut result = Vec::new();
    let (mut left_index, mut right_index) = (0, 0);
    while left_index < left.len() || right_index < right.len() {
        if left_index < left.len()
            && right_index < right.len()
            && left[left_index] == right[right_index]
        {
            result.push(diff_line(
                DuplicateTextDiffLineKind::Equal,
                Some(left_index),
                Some(right_index),
                Some(left[left_index].clone()),
                Some(right[right_index].clone()),
            ));
            left_index += 1;
            right_index += 1;
        } else if left_index < left.len()
            && right_index < right.len()
            && lengths[left_index + 1][right_index] == lengths[left_index][right_index + 1]
        {
            result.push(diff_line(
                DuplicateTextDiffLineKind::Modified,
                Some(left_index),
                Some(right_index),
                Some(left[left_index].clone()),
                Some(right[right_index].clone()),
            ));
            left_index += 1;
            right_index += 1;
        } else if left_index < left.len()
            && (right_index == right.len()
                || lengths[left_index + 1][right_index] >= lengths[left_index][right_index + 1])
        {
            result.push(diff_line(
                DuplicateTextDiffLineKind::OnlyLeft,
                Some(left_index),
                None,
                Some(left[left_index].clone()),
                None,
            ));
            left_index += 1;
        } else {
            result.push(diff_line(
                DuplicateTextDiffLineKind::OnlyRight,
                None,
                Some(right_index),
                None,
                Some(right[right_index].clone()),
            ));
            right_index += 1;
        }
    }
    (result, truncated)
}

fn diff_line(
    kind: DuplicateTextDiffLineKind,
    left_index: Option<usize>,
    right_index: Option<usize>,
    left: Option<String>,
    right: Option<String>,
) -> DuplicateTextDiffLine {
    DuplicateTextDiffLine {
        kind,
        left_line_number: left_index.map(|index| index + 1),
        right_line_number: right_index.map(|index| index + 1),
        left,
        right,
    }
}

fn build_groups(
    contents: &[InstanceContent],
    candidates: Vec<CandidatePair>,
) -> Vec<DuplicateGroup> {
    let mut parents = (0..contents.len()).collect::<Vec<_>>();
    for candidate in &candidates {
        union(&mut parents, candidate.left, candidate.right);
    }
    let mut members = BTreeMap::<usize, Vec<usize>>::new();
    for index in 0..contents.len() {
        let root = find(&mut parents, index);
        members.entry(root).or_default().push(index);
    }
    let mut groups = Vec::new();
    for indexes in members.into_values().filter(|indexes| indexes.len() > 1) {
        let index_set = indexes.iter().copied().collect::<HashSet<_>>();
        let comparisons = candidates
            .iter()
            .filter(|candidate| {
                index_set.contains(&candidate.left) && index_set.contains(&candidate.right)
            })
            .map(|candidate| candidate.comparison.clone())
            .collect::<Vec<_>>();
        let status = if comparisons
            .iter()
            .all(|comparison| comparison.status == DuplicateCheckStatus::Exact)
        {
            DuplicateCheckStatus::Exact
        } else if comparisons
            .iter()
            .any(|comparison| comparison.status == DuplicateCheckStatus::Suspected)
        {
            DuplicateCheckStatus::Suspected
        } else {
            DuplicateCheckStatus::NameConflict
        };
        let similarity = comparisons
            .iter()
            .map(|comparison| comparison.similarity)
            .sum::<f64>()
            / comparisons.len() as f64;
        let hit_rules = comparisons
            .iter()
            .flat_map(|comparison| comparison.hit_rules.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let fingerprint_files = indexes
            .iter()
            .flat_map(|index| contents[*index].files.keys().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut instances = indexes
            .iter()
            .map(|index| review_instance(&contents[*index].instance))
            .collect::<Vec<_>>();
        instances.sort_by(|left, right| left.id.cmp(&right.id));
        let instance_ids = instances
            .iter()
            .map(|instance| instance.id.clone())
            .collect::<Vec<_>>();
        let name = group_display_name(&instances);
        groups.push(DuplicateGroup {
            id: group_key(&instance_ids),
            name,
            status,
            similarity,
            hit_rules,
            fingerprint_files,
            instances,
            comparisons,
        });
    }
    groups.sort_by(|left, right| {
        duplicate_status_priority(&right.status)
            .cmp(&duplicate_status_priority(&left.status))
            .then_with(|| left.name.cmp(&right.name))
    });
    groups
}

fn review_instance(instance: &SkillInstance) -> DuplicateReviewInstance {
    DuplicateReviewInstance {
        id: instance.id.clone(),
        name: instance.name.clone(),
        description: instance.description.clone(),
        path: instance.real_path.clone(),
        client: instance.client.clone(),
    }
}

fn group_display_name(instances: &[DuplicateReviewInstance]) -> String {
    let first = &instances[0].name;
    if instances
        .iter()
        .all(|instance| normalized_name(&instance.name) == normalized_name(first))
    {
        first.clone()
    } else {
        instances
            .iter()
            .take(2)
            .map(|instance| instance.name.as_str())
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

fn normalized_name(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn stable_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn group_key(instance_ids: &[String]) -> String {
    stable_fingerprint(instance_ids.join("\0").as_bytes())
}

fn pair_key(left: &str, right: &str) -> String {
    if left <= right {
        format!("{left}\0{right}")
    } else {
        format!("{right}\0{left}")
    }
}

fn suppressed_pairs(decisions: &[DuplicateDecisionRecord]) -> HashSet<String> {
    let mut pairs = HashSet::new();
    for decision in decisions {
        for left in 0..decision.instance_ids.len() {
            for right in (left + 1)..decision.instance_ids.len() {
                pairs.insert(pair_key(
                    &decision.instance_ids[left],
                    &decision.instance_ids[right],
                ));
            }
        }
    }
    pairs
}

fn decision_kind_database(kind: &DuplicateDecisionKind) -> &'static str {
    match kind {
        DuplicateDecisionKind::NotDuplicate => "not_duplicate",
        DuplicateDecisionKind::Ignored => "ignored",
    }
}

fn decision_kind_from_database(value: &str) -> DuplicateDecisionKind {
    match value {
        "not_duplicate" => DuplicateDecisionKind::NotDuplicate,
        _ => DuplicateDecisionKind::Ignored,
    }
}

fn duplicate_status_priority(status: &DuplicateCheckStatus) -> u8 {
    match status {
        DuplicateCheckStatus::None => 0,
        DuplicateCheckStatus::Exact => 1,
        DuplicateCheckStatus::Suspected => 2,
        DuplicateCheckStatus::NameConflict => 3,
    }
}

fn find(parents: &mut [usize], index: usize) -> usize {
    if parents[index] != index {
        parents[index] = find(parents, parents[index]);
    }
    parents[index]
}

fn union(parents: &mut [usize], left: usize, right: usize) {
    let left = find(parents, left);
    let right = find(parents, right);
    if left != right {
        parents[right] = left;
    }
}
