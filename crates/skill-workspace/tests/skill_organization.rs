use std::{collections::BTreeMap, fs, path::Path};

use skill_workspace::{SkillOrganizationChange, SkillQuery, SkillWorkspace};
use tempfile::tempdir;

#[test]
fn personal_user_organizes_instances_without_changing_real_skill_files() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join(".codex/skills");
    let database_path = sandbox.path().join("index.sqlite3");
    write_skill(&root, "api-review", "api-review", "审查 API。", "正文 A。");
    write_skill(&root, "release", "release", "整理发布说明。", "正文 B。");
    let files_before = file_contents(&root);

    let workspace = SkillWorkspace::open(&database_path).expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描 Skill 根目录");
    let api_id = instance_id(&snapshot, "api-review");
    let release_id = instance_id(&snapshot, "release");
    let organization = workspace
        .create_skill_group("发布工作")
        .expect("创建 Skill 组");
    let group_id = organization.groups[0].id;

    workspace
        .apply_skill_organization_change(&SkillOrganizationChange {
            instance_ids: vec![api_id.clone(), release_id.clone()],
            add_tags: vec!["常用".to_owned(), "发布".to_owned()],
            remove_tags: vec![],
            add_group_ids: vec![group_id],
            remove_group_ids: vec![],
        })
        .expect("批量加入 Skill 组并添加 Skill 标签");
    workspace
        .reorder_skill_group(group_id, &[release_id.clone(), api_id.clone()])
        .expect("自定义 Skill 组顺序");
    workspace
        .rename_skill_group(group_id, "发布流程")
        .expect("重命名 Skill 组");

    assert_search_names(&workspace, "常用", &["api-review", "release"]);
    assert_search_names(&workspace, "发布流程", &["api-review", "release"]);
    assert_eq!(file_contents(&root), files_before);
    drop(workspace);

    let reopened = SkillWorkspace::open(&database_path).expect("重新打开 SkillWorkspace");
    reopened.rescan_all_roots().expect("重新扫描 Skill 根目录");
    let organization = reopened
        .skill_organization()
        .expect("读取 Skill 组和 Skill 标签");
    assert_eq!(organization.groups[0].name, "发布流程");
    assert_eq!(
        organization.groups[0].instance_ids,
        vec![release_id.clone(), api_id.clone()]
    );
    assert_eq!(organization.instances.len(), 2);
    assert!(organization.instances.iter().all(|entry| {
        entry.tags == vec!["发布".to_owned(), "常用".to_owned()]
            && entry.group_ids == vec![group_id]
    }));

    reopened
        .apply_skill_organization_change(&SkillOrganizationChange {
            instance_ids: vec![api_id.clone()],
            add_tags: vec![],
            remove_tags: vec!["常用".to_owned()],
            add_group_ids: vec![],
            remove_group_ids: vec![group_id],
        })
        .expect("从单个实例移除 Skill 标签和 Skill 组");
    let organization = reopened
        .delete_skill_group(group_id)
        .expect("删除虚拟 Skill 组");
    assert!(organization.groups.is_empty());
    assert_search_names(&reopened, "发布流程", &[]);
    assert_search_names(&reopened, "常用", &["release"]);
    assert_eq!(file_contents(&root), files_before);
}

#[test]
fn removing_roots_and_missing_instances_prunes_only_stale_organization_references() {
    let sandbox = tempdir().expect("创建临时工作区");
    let first_root = sandbox.path().join(".codex/skills");
    let second_root = sandbox.path().join(".claude/skills");
    write_skill(&first_root, "alpha", "alpha", "Alpha。", "正文 A。");
    write_skill(&second_root, "beta", "beta", "Beta。", "正文 B。");
    write_skill(&second_root, "gamma", "gamma", "Gamma。", "正文 C。");
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let first_snapshot = workspace.add_root(&first_root).unwrap();
    let first_root_id = first_snapshot.roots[0].id;
    let alpha_id = instance_id(&first_snapshot, "alpha");
    let snapshot = workspace.add_root(&second_root).unwrap();
    let beta_id = instance_id(&snapshot, "beta");
    let gamma_id = instance_id(&snapshot, "gamma");
    let cross_group = workspace.create_skill_group("跨目录").unwrap().groups[0].id;
    let retained_group = workspace
        .create_skill_group("保留组")
        .unwrap()
        .groups
        .into_iter()
        .find(|group| group.name == "保留组")
        .unwrap()
        .id;
    workspace
        .apply_skill_organization_change(&SkillOrganizationChange {
            instance_ids: vec![alpha_id.clone(), beta_id.clone()],
            add_tags: vec!["共享".to_owned()],
            remove_tags: vec![],
            add_group_ids: vec![cross_group],
            remove_group_ids: vec![],
        })
        .unwrap();
    workspace
        .apply_skill_organization_change(&SkillOrganizationChange {
            instance_ids: vec![gamma_id.clone()],
            add_tags: vec!["保留".to_owned()],
            remove_tags: vec![],
            add_group_ids: vec![retained_group],
            remove_group_ids: vec![],
        })
        .unwrap();

    workspace
        .remove_root(first_root_id)
        .expect("移除第一个 Skill 根目录");
    let organization = workspace.skill_organization().unwrap();
    assert_eq!(
        group_members(&organization, cross_group),
        vec![beta_id.clone()]
    );
    assert_eq!(
        group_members(&organization, retained_group),
        vec![gamma_id.clone()]
    );
    assert!(
        organization
            .instances
            .iter()
            .all(|entry| entry.instance_id != alpha_id)
    );

    fs::remove_dir_all(second_root.join("beta")).unwrap();
    let second_root_id = workspace.snapshot().unwrap().roots[0].id;
    workspace
        .rescan_root(second_root_id)
        .expect("重扫移除失效 Skill 实例");
    let organization = workspace.skill_organization().unwrap();
    assert!(group_members(&organization, cross_group).is_empty());
    assert_eq!(
        group_members(&organization, retained_group),
        vec![gamma_id.clone()]
    );
    assert_eq!(organization.instances.len(), 1);
    assert_eq!(organization.instances[0].instance_id, gamma_id);
    assert_eq!(organization.instances[0].tags, vec!["保留"]);
}

fn group_members(
    organization: &skill_workspace::SkillOrganizationSnapshot,
    group_id: i64,
) -> Vec<String> {
    organization
        .groups
        .iter()
        .find(|group| group.id == group_id)
        .unwrap()
        .instance_ids
        .clone()
}

fn assert_search_names(workspace: &SkillWorkspace, text: &str, expected: &[&str]) {
    let result = workspace
        .search_skills(&SkillQuery {
            text: text.to_owned(),
            ..SkillQuery::default()
        })
        .expect("检索组织数据");
    let names = result
        .instances
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, expected, "检索词：{text}");
}

fn instance_id(snapshot: &skill_workspace::WorkspaceSnapshot, name: &str) -> String {
    snapshot
        .instances
        .iter()
        .find(|instance| instance.name == name)
        .unwrap()
        .id
        .clone()
}

fn write_skill(root: &Path, directory: &str, name: &str, description: &str, body: &str) {
    let directory = root.join(directory);
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n"),
    )
    .unwrap();
}

fn file_contents(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}
