use std::collections::{BTreeSet, HashSet};

use rusqlite::{Connection, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{SkillTagsAndGroupsUpdate, SkillWorkspace, WorkspaceError, rebuild_search_document};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOrganizationSnapshot {
    pub groups: Vec<OrganizationSkillGroup>,
    pub instances: Vec<SkillInstanceOrganization>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationSkillGroup {
    pub id: i64,
    pub name: String,
    pub instance_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInstanceOrganization {
    pub instance_id: String,
    pub tags: Vec<String>,
    pub group_ids: Vec<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillOrganizationChange {
    pub instance_ids: Vec<String>,
    pub add_tags: Vec<String>,
    pub remove_tags: Vec<String>,
    pub add_group_ids: Vec<i64>,
    pub remove_group_ids: Vec<i64>,
}

impl SkillWorkspace {
    pub fn skill_organization(&self) -> Result<SkillOrganizationSnapshot, WorkspaceError> {
        let connection = Connection::open(&self.database_path)?;
        organization_snapshot(&connection)
    }

    pub fn create_skill_group(
        &self,
        name: &str,
    ) -> Result<SkillOrganizationSnapshot, WorkspaceError> {
        let name = normalized_term(name, "Skill 组名称")?;
        let connection = Connection::open(&self.database_path)?;
        connection.execute(
            "INSERT INTO skill_organization_groups (name) VALUES (?1)",
            [&name],
        )?;
        organization_snapshot(&connection)
    }

    pub fn rename_skill_group(
        &self,
        group_id: i64,
        name: &str,
    ) -> Result<SkillOrganizationSnapshot, WorkspaceError> {
        let name = normalized_term(name, "Skill 组名称")?;
        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction()?;
        let instance_ids = group_instance_ids(&transaction, group_id)?;
        let changed = transaction.execute(
            "UPDATE skill_organization_groups SET name = ?1 WHERE id = ?2",
            params![name, group_id],
        )?;
        if changed == 0 {
            return Err(WorkspaceError::UnknownSkillGroup(group_id));
        }
        sync_organization_projection(&transaction, &instance_ids)?;
        transaction.commit()?;
        organization_snapshot(&connection)
    }

    pub fn delete_skill_group(
        &self,
        group_id: i64,
    ) -> Result<SkillOrganizationSnapshot, WorkspaceError> {
        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction()?;
        let instance_ids = group_instance_ids(&transaction, group_id)?;
        let changed = transaction.execute(
            "DELETE FROM skill_organization_groups WHERE id = ?1",
            [group_id],
        )?;
        if changed == 0 {
            return Err(WorkspaceError::UnknownSkillGroup(group_id));
        }
        transaction.execute(
            "DELETE FROM skill_group_memberships WHERE group_id = ?1",
            [group_id],
        )?;
        sync_organization_projection(&transaction, &instance_ids)?;
        transaction.commit()?;
        organization_snapshot(&connection)
    }

    pub fn apply_skill_organization_change(
        &self,
        change: &SkillOrganizationChange,
    ) -> Result<SkillOrganizationSnapshot, WorkspaceError> {
        let instance_ids = unique_instance_ids(&change.instance_ids)?;
        let add_tags = normalized_terms(&change.add_tags, "Skill 标签")?;
        let remove_tags = normalized_terms(&change.remove_tags, "Skill 标签")?;
        validate_disjoint(&add_tags, &remove_tags, "Skill 标签")?;
        let add_group_ids = unique_group_ids(&change.add_group_ids);
        let remove_group_ids = unique_group_ids(&change.remove_group_ids);
        validate_disjoint(&add_group_ids, &remove_group_ids, "Skill 组")?;

        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction()?;
        validate_instances(&transaction, &instance_ids)?;
        validate_groups(
            &transaction,
            &add_group_ids
                .iter()
                .chain(&remove_group_ids)
                .copied()
                .collect::<Vec<_>>(),
        )?;

        for instance_id in &instance_ids {
            for tag in &remove_tags {
                transaction.execute(
                    "DELETE FROM skill_instance_tags WHERE instance_id = ?1 AND tag = ?2",
                    params![instance_id, tag],
                )?;
            }
            for tag in &add_tags {
                transaction.execute(
                    "INSERT OR IGNORE INTO skill_instance_tags (instance_id, tag) VALUES (?1, ?2)",
                    params![instance_id, tag],
                )?;
            }
            for group_id in &remove_group_ids {
                transaction.execute(
                    "DELETE FROM skill_group_memberships WHERE group_id = ?1 AND instance_id = ?2",
                    params![group_id, instance_id],
                )?;
            }
        }
        for group_id in &add_group_ids {
            let mut position = next_group_position(&transaction, *group_id)?;
            for instance_id in &instance_ids {
                let changed = transaction.execute(
                    "INSERT OR IGNORE INTO skill_group_memberships (group_id, instance_id, position) VALUES (?1, ?2, ?3)",
                    params![group_id, instance_id, position],
                )?;
                if changed > 0 {
                    position += 1;
                }
            }
        }
        sync_organization_projection(&transaction, &instance_ids)?;
        transaction.commit()?;
        organization_snapshot(&connection)
    }

    pub fn reorder_skill_group(
        &self,
        group_id: i64,
        ordered_instance_ids: &[String],
    ) -> Result<SkillOrganizationSnapshot, WorkspaceError> {
        let ordered_instance_ids = unique_instance_ids(ordered_instance_ids)?;
        let mut connection = Connection::open(&self.database_path)?;
        let transaction = connection.transaction()?;
        validate_groups(&transaction, &[group_id])?;
        let current = group_instance_ids(&transaction, group_id)?;
        let current_set = current.iter().collect::<HashSet<_>>();
        let ordered_set = ordered_instance_ids.iter().collect::<HashSet<_>>();
        if current_set != ordered_set {
            return Err(WorkspaceError::InvalidOrganization(
                "自定义顺序必须包含 Skill 组中的全部实例，且不能包含其他实例。".to_owned(),
            ));
        }
        for (position, instance_id) in ordered_instance_ids.iter().enumerate() {
            transaction.execute(
                "UPDATE skill_group_memberships SET position = ?1 WHERE group_id = ?2 AND instance_id = ?3",
                params![position as i64, group_id, instance_id],
            )?;
        }
        transaction.commit()?;
        organization_snapshot(&connection)
    }
}

pub(crate) fn initialize_organization(connection: &Connection) -> Result<(), WorkspaceError> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS skill_organization_groups (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE IF NOT EXISTS skill_instance_tags (
            instance_id TEXT NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (instance_id, tag)
        );
        CREATE TABLE IF NOT EXISTS skill_group_memberships (
            group_id INTEGER NOT NULL,
            instance_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            PRIMARY KEY (group_id, instance_id)
        );
        CREATE INDEX IF NOT EXISTS skill_group_membership_order
        ON skill_group_memberships (group_id, position, instance_id);
        ",
    )?;
    migrate_legacy_projection(connection)
}

pub(crate) fn replace_legacy_organization(
    workspace: &SkillWorkspace,
    updates: &[SkillTagsAndGroupsUpdate],
) -> Result<(), WorkspaceError> {
    let mut connection = Connection::open(&workspace.database_path)?;
    let transaction = connection.transaction()?;
    let instance_ids = updates
        .iter()
        .map(|update| update.instance_id.clone())
        .collect::<Vec<_>>();
    validate_instances(&transaction, &instance_ids)?;
    for update in updates {
        transaction.execute(
            "DELETE FROM skill_instance_tags WHERE instance_id = ?1",
            [&update.instance_id],
        )?;
        transaction.execute(
            "DELETE FROM skill_group_memberships WHERE instance_id = ?1",
            [&update.instance_id],
        )?;
        for tag in normalized_terms(&update.tags, "Skill 标签")? {
            transaction.execute(
                "INSERT INTO skill_instance_tags (instance_id, tag) VALUES (?1, ?2)",
                params![update.instance_id, tag],
            )?;
        }
        for group_name in normalized_terms(&update.skill_groups, "Skill 组名称")? {
            let group_id = ensure_group(&transaction, &group_name)?;
            let position = next_group_position(&transaction, group_id)?;
            transaction.execute(
                "INSERT INTO skill_group_memberships (group_id, instance_id, position) VALUES (?1, ?2, ?3)",
                params![group_id, update.instance_id, position],
            )?;
        }
    }
    sync_organization_projection(&transaction, &instance_ids)?;
    transaction.commit()?;
    Ok(())
}

pub(crate) fn prune_orphaned_organization_records(
    transaction: &Transaction<'_>,
) -> Result<(), WorkspaceError> {
    transaction.execute(
        "DELETE FROM skill_instance_tags WHERE instance_id NOT IN (SELECT id FROM skill_instances)",
        [],
    )?;
    transaction.execute(
        "DELETE FROM skill_group_memberships WHERE instance_id NOT IN (SELECT id FROM skill_instances)",
        [],
    )?;
    transaction.execute(
        "DELETE FROM skill_tags_and_groups WHERE instance_id NOT IN (SELECT id FROM skill_instances)",
        [],
    )?;
    Ok(())
}

fn organization_snapshot(
    connection: &Connection,
) -> Result<SkillOrganizationSnapshot, WorkspaceError> {
    let mut group_statement = connection.prepare(
        "SELECT id, name FROM skill_organization_groups ORDER BY name COLLATE NOCASE, id",
    )?;
    let mut groups = group_statement
        .query_map([], |row| {
            Ok(OrganizationSkillGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                instance_ids: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for group in &mut groups {
        group.instance_ids = group_instance_ids(connection, group.id)?;
    }

    let mut instance_statement = connection.prepare(
        "
        SELECT DISTINCT instance_id FROM (
            SELECT instance_id FROM skill_instance_tags
            UNION
            SELECT instance_id FROM skill_group_memberships
        )
        WHERE instance_id IN (SELECT id FROM skill_instances)
        ORDER BY instance_id
        ",
    )?;
    let instance_ids = instance_statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let instances = instance_ids
        .into_iter()
        .map(|instance_id| {
            Ok(SkillInstanceOrganization {
                tags: instance_tags(connection, &instance_id)?,
                group_ids: instance_group_ids(connection, &instance_id)?,
                instance_id,
            })
        })
        .collect::<Result<Vec<_>, WorkspaceError>>()?;
    Ok(SkillOrganizationSnapshot { groups, instances })
}

fn migrate_legacy_projection(connection: &Connection) -> Result<(), WorkspaceError> {
    let mut statement = connection.prepare(
        "SELECT instance_id, tags, skill_groups FROM skill_tags_and_groups ORDER BY instance_id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (instance_id, tags, groups) in rows {
        for tag in split_terms(&tags) {
            connection.execute(
                "INSERT OR IGNORE INTO skill_instance_tags (instance_id, tag) VALUES (?1, ?2)",
                params![instance_id, tag],
            )?;
        }
        for group_name in split_terms(&groups) {
            connection.execute(
                "INSERT OR IGNORE INTO skill_organization_groups (name) VALUES (?1)",
                [&group_name],
            )?;
            let group_id = connection.query_row(
                "SELECT id FROM skill_organization_groups WHERE name = ?1",
                [&group_name],
                |row| row.get::<_, i64>(0),
            )?;
            let position = connection.query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM skill_group_memberships WHERE group_id = ?1",
                [group_id],
                |row| row.get::<_, i64>(0),
            )?;
            connection.execute(
                "INSERT OR IGNORE INTO skill_group_memberships (group_id, instance_id, position) VALUES (?1, ?2, ?3)",
                params![group_id, instance_id, position],
            )?;
        }
    }
    Ok(())
}

fn sync_organization_projection(
    transaction: &Transaction<'_>,
    instance_ids: &[String],
) -> Result<(), WorkspaceError> {
    for instance_id in instance_ids.iter().collect::<BTreeSet<_>>() {
        let exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM skill_instances WHERE id = ?1)",
            [instance_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            continue;
        }
        let tags = instance_tags(transaction, instance_id)?.join("\n");
        let groups = instance_group_names(transaction, instance_id)?.join("\n");
        transaction.execute(
            "
            INSERT INTO skill_tags_and_groups (instance_id, tags, skill_groups)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(instance_id) DO UPDATE SET
                tags = excluded.tags,
                skill_groups = excluded.skill_groups
            ",
            params![instance_id, tags, groups],
        )?;
        rebuild_search_document(transaction, instance_id)?;
    }
    Ok(())
}

fn validate_instances(
    connection: &Connection,
    instance_ids: &[String],
) -> Result<(), WorkspaceError> {
    for instance_id in instance_ids {
        let exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM skill_instances WHERE id = ?1)",
            [instance_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Err(WorkspaceError::UnknownInstance(instance_id.clone()));
        }
    }
    Ok(())
}

fn validate_groups(connection: &Connection, group_ids: &[i64]) -> Result<(), WorkspaceError> {
    for group_id in group_ids.iter().collect::<BTreeSet<_>>() {
        let exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM skill_organization_groups WHERE id = ?1)",
            [group_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Err(WorkspaceError::UnknownSkillGroup(*group_id));
        }
    }
    Ok(())
}

fn unique_instance_ids(instance_ids: &[String]) -> Result<Vec<String>, WorkspaceError> {
    if instance_ids.is_empty() {
        return Err(WorkspaceError::InvalidOrganization(
            "至少选择一个 Skill 实例。".to_owned(),
        ));
    }
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for instance_id in instance_ids {
        if seen.insert(instance_id.clone()) {
            unique.push(instance_id.clone());
        }
    }
    Ok(unique)
}

fn unique_group_ids(group_ids: &[i64]) -> Vec<i64> {
    group_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalized_terms(values: &[String], label: &str) -> Result<Vec<String>, WorkspaceError> {
    values
        .iter()
        .map(|value| normalized_term(value, label))
        .collect::<Result<BTreeSet<_>, _>>()
        .map(|values| values.into_iter().collect())
}

fn normalized_term(value: &str, label: &str) -> Result<String, WorkspaceError> {
    let value = value.trim();
    if value.is_empty() || value.contains(['\n', '\r']) || value.chars().count() > 80 {
        return Err(WorkspaceError::InvalidOrganization(format!(
            "{label}不能为空、不能换行，且最多 80 个字符。"
        )));
    }
    Ok(value.to_owned())
}

fn validate_disjoint<T: Eq + std::hash::Hash>(
    add: &[T],
    remove: &[T],
    label: &str,
) -> Result<(), WorkspaceError> {
    let remove = remove.iter().collect::<HashSet<_>>();
    if add.iter().any(|value| remove.contains(value)) {
        return Err(WorkspaceError::InvalidOrganization(format!(
            "同一个{label}不能同时添加和移除。"
        )));
    }
    Ok(())
}

fn ensure_group(connection: &Connection, name: &str) -> Result<i64, WorkspaceError> {
    connection.execute(
        "INSERT OR IGNORE INTO skill_organization_groups (name) VALUES (?1)",
        [name],
    )?;
    Ok(connection.query_row(
        "SELECT id FROM skill_organization_groups WHERE name = ?1",
        [name],
        |row| row.get(0),
    )?)
}

fn next_group_position(connection: &Connection, group_id: i64) -> Result<i64, WorkspaceError> {
    Ok(connection.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM skill_group_memberships WHERE group_id = ?1",
        [group_id],
        |row| row.get(0),
    )?)
}

fn group_instance_ids(
    connection: &Connection,
    group_id: i64,
) -> Result<Vec<String>, WorkspaceError> {
    let mut statement = connection.prepare(
        "SELECT instance_id FROM skill_group_memberships WHERE group_id = ?1 ORDER BY position, instance_id",
    )?;
    Ok(statement
        .query_map([group_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn instance_tags(
    connection: &Connection,
    instance_id: &str,
) -> Result<Vec<String>, WorkspaceError> {
    let mut statement = connection.prepare(
        "SELECT tag FROM skill_instance_tags WHERE instance_id = ?1 ORDER BY tag COLLATE NOCASE",
    )?;
    Ok(statement
        .query_map([instance_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn instance_group_ids(
    connection: &Connection,
    instance_id: &str,
) -> Result<Vec<i64>, WorkspaceError> {
    let mut statement = connection.prepare(
        "SELECT group_id FROM skill_group_memberships WHERE instance_id = ?1 ORDER BY group_id",
    )?;
    Ok(statement
        .query_map([instance_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn instance_group_names(
    connection: &Connection,
    instance_id: &str,
) -> Result<Vec<String>, WorkspaceError> {
    let mut statement = connection.prepare(
        "
        SELECT skill_organization_groups.name
        FROM skill_group_memberships
        JOIN skill_organization_groups ON skill_organization_groups.id = skill_group_memberships.group_id
        WHERE skill_group_memberships.instance_id = ?1
        ORDER BY skill_organization_groups.name COLLATE NOCASE
        ",
    )?;
    Ok(statement
        .query_map([instance_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn split_terms(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}
