use std::{collections::BTreeMap, fs, path::Path};

use skill_workspace::{
    DuplicateCheckStatus, FileOperationResultStatus, SkillDraft, SkillDraftTarget,
    SkillOrganizationChange, SkillQuery, SkillStatus, SkillWorkspace,
};
use tempfile::tempdir;

#[test]
fn macos_v1_smoke_keeps_the_full_local_workflow_recoverable() {
    let sandbox = tempdir().expect("创建临时验收目录");
    let codex_root = sandbox.path().join(".codex/skills");
    let claude_root = sandbox.path().join(".claude/skills");
    let source_directory = codex_root.join("release-notes");
    let target_directory = claude_root.join("release-notes-copy");

    write_skill(
        &source_directory,
        "release-notes",
        "整理版本发布说明。",
        "# 发布说明\n\n原始主实例。\n",
    );
    write_skill(
        &target_directory,
        "release-notes",
        "旧版发布流程。",
        "# 发布说明\n\n目标实例旧内容。\n",
    );
    fs::write(target_directory.join("legacy.txt"), "归并前的目标附件\n").expect("写入目标附件");
    let repair_directory = codex_root.join("needs-repair");
    fs::create_dir_all(&repair_directory).expect("创建需要修复的 Skill");
    fs::write(
        repair_directory.join("SKILL.md"),
        "---\nname: needs-repair\n---\n\n# 缺少描述\n",
    )
    .expect("写入需要修复的 Skill");

    let workspace = SkillWorkspace::open(sandbox.path().join("app-data/index.sqlite3"))
        .expect("打开本地工作区");
    workspace.add_root(&codex_root).expect("授权 Codex 根目录");
    let snapshot = workspace
        .add_root(&claude_root)
        .expect("授权 Claude 根目录");
    assert_eq!(snapshot.roots.len(), 2);
    assert!(snapshot.instances.iter().any(|instance| {
        instance.name == "needs-repair" && instance.status == SkillStatus::NeedsRepair
    }));

    let source_id = instance_id_at(&snapshot, &source_directory);
    let target_id = instance_id_at(&snapshot, &target_directory);
    let searched = workspace
        .search_skills(&SkillQuery {
            text: "版本发布".to_owned(),
            ..SkillQuery::default()
        })
        .expect("检索 Skill");
    assert_eq!(searched.total, 1);
    assert_eq!(searched.instances[0].id, source_id);

    let draft = SkillDraft {
        target: SkillDraftTarget::Existing {
            instance_id: source_id.clone(),
        },
        name: "release-notes".to_owned(),
        description: "整理已审核的发布说明。".to_owned(),
        markdown_body: "# 发布说明\n\n检索标记 v1-smoke-edited。\n".to_owned(),
        file_changes: vec![],
    };
    let edit_plan = workspace.plan_skill_change(&draft).expect("预览编辑");
    workspace
        .execute_skill_change(edit_plan.id)
        .expect("保存编辑");
    assert_eq!(
        workspace
            .search_skills(&SkillQuery {
                text: "v1-smoke-edited".to_owned(),
                ..SkillQuery::default()
            })
            .expect("检索编辑后的正文")
            .total,
        1
    );

    let organization = workspace
        .create_skill_group("首版验收")
        .expect("创建 Skill 组");
    let group_id = organization.groups[0].id;
    let organization = workspace
        .apply_skill_organization_change(&SkillOrganizationChange {
            instance_ids: vec![source_id.clone(), target_id.clone()],
            add_tags: vec!["本地".to_owned(), "已验收".to_owned()],
            add_group_ids: vec![group_id],
            ..SkillOrganizationChange::default()
        })
        .expect("批量加入 Skill 组并添加 Skill 标签");
    assert_eq!(organization.groups[0].instance_ids.len(), 2);

    let review = workspace.review_duplicate_groups().expect("执行重复检查");
    let related_group = review
        .groups
        .iter()
        .find(|group| {
            let ids = group
                .instances
                .iter()
                .map(|instance| instance.id.as_str())
                .collect::<Vec<_>>();
            ids.contains(&source_id.as_str()) && ids.contains(&target_id.as_str())
        })
        .expect("找到疑似重复或同名冲突结果");
    assert!(matches!(
        related_group.status,
        DuplicateCheckStatus::Suspected | DuplicateCheckStatus::NameConflict
    ));

    let target_before_merge = read_tree(&target_directory);
    let source_before_merge = read_tree(&source_directory);
    let merge_plan = workspace
        .plan_duplicate_merge(&source_id, std::slice::from_ref(&target_id))
        .expect("预览归并");
    let merge_outcome = workspace
        .execute_file_operation_plan(merge_plan.id)
        .expect("执行归并");
    assert_eq!(merge_outcome.results.len(), 1);
    assert_eq!(
        merge_outcome.results[0].status,
        FileOperationResultStatus::Success
    );
    assert_eq!(read_tree(&target_directory), source_before_merge);

    workspace
        .undo_file_operation_batch(merge_outcome.batch_id)
        .expect("撤销归并");
    assert_eq!(read_tree(&target_directory), target_before_merge);
    assert_eq!(read_tree(&source_directory), source_before_merge);
    assert_eq!(
        workspace
            .search_skills(&SkillQuery {
                text: "目标实例旧内容".to_owned(),
                ..SkillQuery::default()
            })
            .expect("确认撤销后索引恢复")
            .total,
        1
    );
}

fn write_skill(directory: &Path, name: &str, description: &str, markdown_body: &str) {
    fs::create_dir_all(directory).expect("创建 Skill 目录");
    fs::write(
        directory.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n{markdown_body}"),
    )
    .expect("写入 SKILL.md");
}

fn instance_id_at(snapshot: &skill_workspace::WorkspaceSnapshot, directory: &Path) -> String {
    let real_path = fs::canonicalize(directory).expect("规范化 Skill 路径");
    snapshot
        .instances
        .iter()
        .find(|instance| Path::new(&instance.real_path) == real_path)
        .expect("找到 Skill 实例")
        .id
        .clone()
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        let mut entries = fs::read_dir(directory)
            .expect("读取 Skill 目录")
            .collect::<Result<Vec<_>, _>>()
            .expect("读取目录项");
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root)
                        .expect("生成相对路径")
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(path).expect("读取文件"),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}
