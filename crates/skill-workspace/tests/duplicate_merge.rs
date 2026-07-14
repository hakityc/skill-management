use std::{fs, path::Path};

use skill_workspace::{
    DuplicateFileDifferenceStatus, DuplicateFileNodeKind, FileOperationKind,
    FileOperationResultStatus, SkillQuery, SkillWorkspace,
};
use tempfile::tempdir;

#[test]
fn personal_user_previews_mirrors_and_undoes_one_duplicate_target() {
    let sandbox = tempdir().unwrap();
    let master_root = sandbox.path().join("master/skills");
    let target_root = sandbox.path().join("target/skills");
    write_skill(&master_root, "release-notes", "主实例正文", "主模板");
    write_skill(&target_root, "release-notes", "目标旧正文", "旧模板");
    fs::write(target_root.join("release-notes/legacy.md"), "仅目标存在").unwrap();
    fs::write(
        target_root.join("release-notes/.DS_Store"),
        "目标噪音也必须明确预览",
    )
    .unwrap();
    fs::write(master_root.join("release-notes/icon.bin"), [1, 2, 3, 4]).unwrap();
    fs::write(target_root.join("release-notes/icon.bin"), [9, 8]).unwrap();
    let target_before = read_tree(&target_root.join("release-notes"));

    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let master_id = workspace.add_root(&master_root).unwrap().instances[0]
        .id
        .clone();
    let target_snapshot = workspace.add_root(&target_root).unwrap();
    let target_root_id = target_snapshot
        .roots
        .iter()
        .find(|root| Path::new(&root.path) == fs::canonicalize(&target_root).unwrap())
        .unwrap()
        .id;
    let target_id = target_snapshot
        .instances
        .iter()
        .find(|instance| instance.root_id == target_root_id)
        .unwrap()
        .id
        .clone();

    let plan = workspace
        .plan_duplicate_merge(&master_id, std::slice::from_ref(&target_id))
        .unwrap();
    assert_eq!(plan.kind, FileOperationKind::Merge);
    assert_eq!(plan.items.len(), 1);
    assert_eq!(
        plan.items[0].instance_id.as_deref(),
        Some(target_id.as_str())
    );
    assert!(plan.items[0].will_overwrite);
    assert!(plan.items[0].changes.iter().any(|change| {
        change.relative_path == "legacy.md"
            && change.status == DuplicateFileDifferenceStatus::OnlyRight
    }));
    assert!(plan.items[0].changes.iter().any(|change| {
        change.relative_path == ".DS_Store"
            && change.status == DuplicateFileDifferenceStatus::OnlyRight
    }));
    let skill_change = plan.items[0]
        .changes
        .iter()
        .find(|change| change.relative_path == "SKILL.md")
        .unwrap();
    assert_eq!(skill_change.status, DuplicateFileDifferenceStatus::Modified);
    assert!(
        skill_change
            .text_diff
            .as_ref()
            .is_some_and(|diff| !diff.is_empty())
    );
    let binary_change = plan.items[0]
        .changes
        .iter()
        .find(|change| change.relative_path == "icon.bin")
        .unwrap();
    assert_ne!(
        binary_change.left_fingerprint,
        binary_change.right_fingerprint
    );
    assert_ne!(binary_change.left_size, binary_change.right_size);
    assert!(target_root.join("release-notes/legacy.md").exists());

    let master_before = read_tree(&master_root.join("release-notes"));
    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    assert_eq!(
        outcome.results[0].status,
        FileOperationResultStatus::Success
    );
    assert!(outcome.results[0].backup_created);
    assert_eq!(read_tree(&master_root.join("release-notes")), master_before);
    assert_eq!(read_tree(&target_root.join("release-notes")), master_before);
    assert!(!target_root.join("release-notes/legacy.md").exists());
    assert!(!target_root.join("release-notes/.DS_Store").exists());
    assert_eq!(
        workspace
            .search_skills(&SkillQuery {
                text: "主实例正文".to_owned(),
                ..SkillQuery::default()
            })
            .unwrap()
            .total,
        2
    );

    fs::write(
        target_root.join("release-notes/local-after.md"),
        "归并后独立修改",
    )
    .unwrap();
    assert!(!master_root.join("release-notes/local-after.md").exists());
    fs::remove_file(target_root.join("release-notes/local-after.md")).unwrap();
    let record = workspace.file_operation_history().unwrap().remove(0);
    assert_eq!(record.kind, FileOperationKind::Merge);
    assert_eq!(record.plan.items[0].changes, plan.items[0].changes);

    workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap();
    assert_eq!(read_tree(&target_root.join("release-notes")), target_before);
    assert_eq!(read_tree(&master_root.join("release-notes")), master_before);
    let restored = workspace
        .search_skills(&SkillQuery {
            text: "目标旧正文".to_owned(),
            ..SkillQuery::default()
        })
        .unwrap();
    assert_eq!(restored.total, 1);
    assert_eq!(restored.instances[0].id, target_id);
}

#[test]
fn multi_target_merge_reports_a_stale_target_without_harming_other_instances() {
    let sandbox = tempdir().unwrap();
    let master_root = sandbox.path().join("master/skills");
    let first_root = sandbox.path().join("first/skills");
    let second_root = sandbox.path().join("second/skills");
    write_skill(&master_root, "shared", "主版本", "主参考");
    write_skill(&first_root, "shared", "目标一旧版本", "一号参考");
    write_skill(&second_root, "shared", "目标二旧版本", "二号参考");
    let first_before = read_tree(&first_root.join("shared"));
    let second_before = read_tree(&second_root.join("shared"));

    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let master_id = workspace.add_root(&master_root).unwrap().instances[0]
        .id
        .clone();
    workspace.add_root(&first_root).unwrap();
    let snapshot = workspace.add_root(&second_root).unwrap();
    let first_root_id = snapshot
        .roots
        .iter()
        .find(|root| Path::new(&root.path) == fs::canonicalize(&first_root).unwrap())
        .unwrap()
        .id;
    let second_root_id = snapshot
        .roots
        .iter()
        .find(|root| Path::new(&root.path) == fs::canonicalize(&second_root).unwrap())
        .unwrap()
        .id;
    let first_id = snapshot
        .instances
        .iter()
        .find(|instance| instance.root_id == first_root_id)
        .unwrap()
        .id
        .clone();
    let second_id = snapshot
        .instances
        .iter()
        .find(|instance| instance.root_id == second_root_id)
        .unwrap()
        .id
        .clone();
    let plan = workspace
        .plan_duplicate_merge(&master_id, &[first_id, second_id])
        .unwrap();
    fs::write(second_root.join("shared/external.txt"), "预览后外部变化").unwrap();

    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    assert_eq!(outcome.results.len(), 2);
    assert_eq!(
        outcome.results[0].status,
        FileOperationResultStatus::Success
    );
    assert_eq!(outcome.results[1].status, FileOperationResultStatus::Failed);
    assert!(outcome.results[1].message.contains("计划已过期"));
    let master_after = read_tree(&master_root.join("shared"));
    assert_eq!(read_tree(&first_root.join("shared")), master_after);
    let second_after = read_tree(&second_root.join("shared"));
    assert!(
        second_before
            .iter()
            .all(|expected| second_after.contains(expected))
    );
    assert!(second_root.join("shared/external.txt").exists());

    workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap();
    assert_eq!(read_tree(&first_root.join("shared")), first_before);
    assert!(second_root.join("shared/external.txt").exists());
    assert_eq!(read_tree(&master_root.join("shared")), master_after);
}

#[cfg(unix)]
#[test]
fn merge_preview_reports_a_symbolic_link_replacing_an_equal_text_file() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let master_root = sandbox.path().join("master/skills");
    let target_root = sandbox.path().join("target/skills");
    write_skill(&master_root, "shared", "主版本", "主参考");
    write_skill(&target_root, "shared", "目标版本", "目标参考");
    symlink("references/guide.md", master_root.join("shared/guide-link")).unwrap();
    fs::write(target_root.join("shared/guide-link"), "references/guide.md").unwrap();

    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let master = workspace.add_root(&master_root).unwrap();
    let master_id = master.instances[0].id.clone();
    let snapshot = workspace.add_root(&target_root).unwrap();
    let target_root_id = snapshot
        .roots
        .iter()
        .find(|root| Path::new(&root.path) == fs::canonicalize(&target_root).unwrap())
        .unwrap()
        .id;
    let target_id = snapshot
        .instances
        .iter()
        .find(|instance| instance.root_id == target_root_id)
        .unwrap()
        .id
        .clone();

    let plan = workspace
        .plan_duplicate_merge(&master_id, &[target_id])
        .unwrap();
    let type_change = plan.items[0]
        .changes
        .iter()
        .find(|change| change.relative_path == "guide-link")
        .expect("符号链接替换普通文件必须进入覆盖预览");
    assert_eq!(type_change.status, DuplicateFileDifferenceStatus::Modified);
    assert_eq!(
        type_change.left_node_kind,
        Some(DuplicateFileNodeKind::SymbolicLink)
    );
    assert_eq!(
        type_change.right_node_kind,
        Some(DuplicateFileNodeKind::File)
    );

    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    assert!(
        target_root
            .join("shared/guide-link")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap();
    assert!(target_root.join("shared/guide-link").is_file());
    assert_eq!(
        fs::read_to_string(target_root.join("shared/guide-link")).unwrap(),
        "references/guide.md"
    );
}

fn write_skill(root: &Path, directory: &str, body: &str, reference: &str) {
    let skill = root.join(directory);
    fs::create_dir_all(skill.join("references")).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        format!("---\nname: shared\ndescription: 用于安全归并测试。\n---\n\n{body}\n"),
    )
    .unwrap();
    fs::write(skill.join("references/guide.md"), reference).unwrap();
}

fn read_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn visit(root: &Path, current: &Path, files: &mut Vec<(String, Vec<u8>)>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.push((
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut files = Vec::new();
    visit(root, root, &mut files);
    files
}
