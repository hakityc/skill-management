use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use rusqlite::Connection;
use skill_workspace::{
    FileConflictPolicy, FileOperationKind, FileOperationRequest, FileOperationResultStatus,
    SkillWorkspace, ZipImportRequest,
};
use tempfile::tempdir;
use zip::{ZipWriter, write::SimpleFileOptions};

#[test]
fn copy_overwrite_is_planned_backed_up_and_undoable_without_changing_the_source() {
    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    write_skill(&source_root, "shared", "shared", "来源版本。", "source");
    write_skill(
        &target_root,
        "shared",
        "shared",
        "目标旧版本。",
        "target-old",
    );
    let source_before = read_skill(&source_root, "shared");
    let target_before = read_skill(&target_root, "shared");
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let snapshot = workspace.add_root(&source_root).unwrap();
    let source_id = snapshot.instances[0].id.clone();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;

    let skip_plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source_id.clone()],
            kind: FileOperationKind::Copy,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    assert!(skip_plan.items[0].conflict);
    assert!(!skip_plan.items[0].will_overwrite);
    let skipped = workspace.execute_file_operation_plan(skip_plan.id).unwrap();
    assert_eq!(
        skipped.results[0].status,
        FileOperationResultStatus::Skipped
    );
    assert_eq!(read_skill(&target_root, "shared"), target_before);

    let overwrite_plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source_id],
            kind: FileOperationKind::Copy,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Overwrite,
        })
        .unwrap();
    assert!(overwrite_plan.items[0].will_overwrite);
    assert!(!overwrite_plan.items[0].will_remove_source);
    let copied = workspace
        .execute_file_operation_plan(overwrite_plan.id)
        .unwrap();
    assert_eq!(copied.results[0].status, FileOperationResultStatus::Success);
    assert!(copied.results[0].backup_created);
    assert_eq!(read_skill(&source_root, "shared"), source_before);
    assert_eq!(read_skill(&target_root, "shared"), source_before);

    workspace
        .undo_file_operation_batch(copied.batch_id)
        .unwrap();
    assert_eq!(read_skill(&source_root, "shared"), source_before);
    assert_eq!(read_skill(&target_root, "shared"), target_before);
}

#[test]
fn move_batch_reports_each_result_and_rolls_back_only_successful_items() {
    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    write_skill(&source_root, "alpha", "alpha", "Alpha。", "alpha");
    write_skill(&source_root, "beta", "beta", "Beta。", "beta");
    fs::create_dir_all(&target_root).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let source_root_id = source.roots[0].id;
    let ids = source
        .instances
        .iter()
        .map(|instance| instance.id.clone())
        .collect();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: ids,
            kind: FileOperationKind::Move,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    fs::write(source_root.join("beta/external.txt"), "计划后变化").unwrap();

    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    assert_eq!(outcome.results.len(), 2);
    assert_eq!(
        outcome.results[0].status,
        FileOperationResultStatus::Success
    );
    assert_eq!(outcome.results[1].status, FileOperationResultStatus::Failed);
    assert!(outcome.results[1].message.contains("计划已过期"));
    assert!(!source_root.join("alpha").exists());
    assert!(target_root.join("alpha/SKILL.md").exists());
    assert!(source_root.join("beta/SKILL.md").exists());
    assert!(!target_root.join("beta").exists());

    let history = workspace.file_operation_history().unwrap();
    assert_eq!(history[0].results.len(), 2);
    assert!(history[0].undoable);
    assert_eq!(history[0].plan, plan);
    workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap();
    assert!(source_root.join("alpha/SKILL.md").exists());
    assert!(!target_root.join("alpha").exists());
    assert!(source_root.join("beta/external.txt").exists());
    assert_eq!(workspace.snapshot().unwrap().roots[0].id, source_root_id);
}

#[test]
fn trash_batch_uses_the_configured_trash_boundary_and_explains_finder_recovery() {
    let sandbox = tempdir().unwrap();
    let root = sandbox.path().join("skills");
    let trash = sandbox.path().join("fake-system-trash");
    write_skill(&root, "alpha", "alpha", "Alpha。", "alpha");
    write_skill(&root, "beta", "beta", "Beta。", "beta");
    let workspace =
        SkillWorkspace::open_with_trash_directory(sandbox.path().join("index.sqlite3"), &trash)
            .unwrap();
    let snapshot = workspace.add_root(&root).unwrap();
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: snapshot
                .instances
                .iter()
                .map(|instance| instance.id.clone())
                .collect(),
            kind: FileOperationKind::Trash,
            target_root_id: None,
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    assert!(plan.items.iter().all(|item| item.will_remove_source));
    assert!(!plan.undoable);

    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    assert!(outcome.results.iter().all(|result| {
        result.status == FileOperationResultStatus::Success
            && result.message.contains("访达的废纸篓")
    }));
    assert!(!root.join("alpha").exists());
    assert!(!root.join("beta").exists());
    assert_eq!(fs::read_dir(&trash).unwrap().count(), 2);
    assert!(
        workspace
            .latest_undoable_file_operation()
            .unwrap()
            .is_none()
    );
}

#[test]
fn zip_import_rejects_attack_paths_and_confirms_valid_archives_before_writing() {
    let sandbox = tempdir().unwrap();
    let root = sandbox.path().join("skills");
    fs::create_dir_all(&root).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let root_id = workspace.add_root(&root).unwrap().roots[0].id;

    for (name, entry, content, mode) in [
        ("parent.zip", "../escaped.txt", "escape", None),
        ("absolute.zip", "/absolute.txt", "escape", None),
        (
            "symlink.zip",
            "skill/outside",
            "../../outside",
            Some(0o120777),
        ),
    ] {
        let archive = sandbox.path().join(name);
        write_zip(&archive, &[(entry, content, mode)]);
        let error = workspace
            .preview_zip_import(&ZipImportRequest {
                zip_path: archive.to_string_lossy().into_owned(),
                target_root_id: root_id,
                relative_path: "imported".to_owned(),
                conflict_policy: FileConflictPolicy::Skip,
            })
            .unwrap_err();
        assert!(error.to_string().contains("ZIP"));
    }
    assert!(!sandbox.path().join("escaped.txt").exists());

    let archive = sandbox.path().join("valid.zip");
    write_zip(
        &archive,
        &[
            (
                "skill/SKILL.md",
                "---\nname: imported\ndescription: ZIP 导入。\n---\n",
                None,
            ),
            ("skill/references/guide.md", "guide", None),
        ],
    );
    let plan = workspace
        .preview_zip_import(&ZipImportRequest {
            zip_path: archive.to_string_lossy().into_owned(),
            target_root_id: root_id,
            relative_path: "imported".to_owned(),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    assert!(!root.join("imported").exists());
    assert_eq!(plan.kind, FileOperationKind::Import);
    assert_eq!(plan.items[0].file_count, 2);
    let imported = workspace.execute_file_operation_plan(plan.id).unwrap();
    assert!(root.join("imported/SKILL.md").exists());
    assert_eq!(workspace.snapshot().unwrap().instances[0].name, "imported");
    workspace
        .undo_file_operation_batch(imported.batch_id)
        .unwrap();
    assert!(!root.join("imported").exists());
}

#[cfg(unix)]
#[test]
fn moving_linked_instances_never_removes_shared_targets_and_undo_restores_links() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    let shared_source = sandbox.path().join("shared/source-skill");
    let shared_target = sandbox.path().join("shared/target-skill");
    write_skill(
        shared_source.parent().unwrap(),
        "source-skill",
        "linked",
        "共享来源。",
        "source",
    );
    write_skill(
        shared_target.parent().unwrap(),
        "target-skill",
        "linked",
        "共享目标。",
        "target",
    );
    fs::create_dir_all(&source_root).unwrap();
    fs::create_dir_all(&target_root).unwrap();
    symlink(&shared_source, source_root.join("linked")).unwrap();
    symlink(&shared_target, target_root.join("linked")).unwrap();

    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let source_snapshot = workspace.add_root(&source_root).unwrap();
    let source_id = source_snapshot.instances[0].id.clone();
    assert!(source_snapshot.instances[0].link_path.is_some());
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source_id],
            kind: FileOperationKind::Move,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Overwrite,
        })
        .unwrap();

    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    assert!(!source_root.join("linked").exists());
    assert!(shared_source.join("SKILL.md").exists());
    assert!(
        !target_root
            .join("linked")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        read_skill(&target_root, "linked"),
        read_skill(shared_source.parent().unwrap(), "source-skill")
    );
    assert!(shared_target.join("SKILL.md").exists());

    workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap();
    assert!(
        source_root
            .join("linked")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        target_root
            .join("linked")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_link(source_root.join("linked")).unwrap(),
        shared_source
    );
    assert_eq!(
        fs::read_link(target_root.join("linked")).unwrap(),
        shared_target
    );
}

#[test]
fn undo_refuses_to_overwrite_external_changes_made_after_a_copy() {
    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    write_skill(&source_root, "alpha", "alpha", "Alpha。", "source");
    fs::create_dir_all(&target_root).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source.instances[0].id.clone()],
            kind: FileOperationKind::Copy,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    fs::write(target_root.join("alpha/external.txt"), "用户稍后写入").unwrap();

    let error = workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap_err();
    assert!(error.to_string().contains("计划已过期"));
    assert!(target_root.join("alpha/external.txt").exists());
    assert!(source_root.join("alpha/SKILL.md").exists());
}

#[test]
fn reopening_workspace_finishes_an_interrupted_file_operation_undo() {
    let sandbox = tempdir().unwrap();
    let database = sandbox.path().join("index.sqlite3");
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    write_skill(&source_root, "alpha", "alpha", "Alpha。", "source");
    fs::create_dir_all(&target_root).unwrap();
    let workspace = SkillWorkspace::open(&database).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source.instances[0].id.clone()],
            kind: FileOperationKind::Move,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    Connection::open(&database)
        .unwrap()
        .execute(
            "UPDATE file_operation_batches SET undoing = 1 WHERE id = ?1",
            [outcome.batch_id],
        )
        .unwrap();
    drop(workspace);

    let reopened = SkillWorkspace::open(&database).unwrap();
    assert!(source_root.join("alpha/SKILL.md").exists());
    assert!(!target_root.join("alpha").exists());
    assert!(reopened.file_operation_history().unwrap()[0].undone);
    assert_eq!(reopened.snapshot().unwrap().instances.len(), 1);
}

#[cfg(unix)]
#[test]
fn overwrite_and_undo_preserve_a_broken_target_symbolic_link() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    let missing_target = sandbox.path().join("missing/shared-skill");
    write_skill(&source_root, "alpha", "alpha", "Alpha。", "source");
    fs::create_dir_all(&target_root).unwrap();
    symlink(&missing_target, target_root.join("alpha")).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source.instances[0].id.clone()],
            kind: FileOperationKind::Copy,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Overwrite,
        })
        .unwrap();
    assert!(plan.items[0].conflict);
    assert!(plan.items[0].will_overwrite);

    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    assert!(target_root.join("alpha/SKILL.md").exists());
    workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap();
    assert!(
        target_root
            .join("alpha")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::read_link(target_root.join("alpha")).unwrap(),
        missing_target
    );
}

#[test]
fn batch_plan_rejects_two_sources_that_would_write_the_same_target() {
    let sandbox = tempdir().unwrap();
    let first_root = sandbox.path().join("first/skills");
    let second_root = sandbox.path().join("second/skills");
    let target_root = sandbox.path().join("target/skills");
    write_skill(&first_root, "shared", "first", "第一个来源。", "first");
    write_skill(&second_root, "shared", "second", "第二个来源。", "second");
    fs::create_dir_all(&target_root).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let first = workspace.add_root(&first_root).unwrap();
    let first_id = first.instances[0].id.clone();
    let second = workspace.add_root(&second_root).unwrap();
    let second_id = second
        .instances
        .iter()
        .find(|instance| instance.root_id == second.roots[1].id)
        .unwrap()
        .id
        .clone();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[2].id;

    let error = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![first_id, second_id],
            kind: FileOperationKind::Copy,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Overwrite,
        })
        .unwrap_err();
    assert!(error.to_string().contains("同一目标"));
    assert!(!target_root.join("shared").exists());
}

#[cfg(unix)]
#[test]
fn destructive_operations_reject_skills_reached_through_a_parent_symbolic_link() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    let shared_group = sandbox.path().join("shared/group");
    let linked_target = sandbox.path().join("shared/linked-target");
    write_skill(&shared_group, "alpha", "alpha", "共享 Skill。", "shared");
    write_skill(
        linked_target.parent().unwrap(),
        "linked-target",
        "linked-alpha",
        "共享链接 Skill。",
        "linked",
    );
    symlink(&linked_target, shared_group.join("linked-alpha")).unwrap();
    fs::create_dir_all(&source_root).unwrap();
    fs::create_dir_all(&target_root).unwrap();
    symlink(&shared_group, source_root.join("linked-group")).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;

    for instance in &source.instances {
        assert!(instance.link_path.is_some());
        for (kind, target_root_id) in [
            (FileOperationKind::Move, Some(target_root_id)),
            (FileOperationKind::Trash, None),
        ] {
            let error = workspace
                .plan_file_operations(&FileOperationRequest {
                    instance_ids: vec![instance.id.clone()],
                    kind,
                    target_root_id,
                    conflict_policy: FileConflictPolicy::Skip,
                })
                .unwrap_err();
            assert!(error.to_string().contains("符号链接目录下"));
        }
    }
    assert!(shared_group.join("alpha/SKILL.md").exists());
    assert!(shared_group.join("linked-alpha").symlink_metadata().is_ok());
}

#[cfg(unix)]
#[test]
fn target_paths_through_symbolic_link_ancestors_are_rejected_before_writing() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    let outside = sandbox.path().join("outside");
    write_skill(
        &source_root.join("namespace"),
        "alpha",
        "alpha",
        "Alpha。",
        "source",
    );
    fs::create_dir_all(&target_root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, target_root.join("namespace")).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;

    let error = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source.instances[0].id.clone()],
            kind: FileOperationKind::Copy,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Overwrite,
        })
        .unwrap_err();
    assert!(error.to_string().contains("经过符号链接目录"));
    assert!(!outside.join("alpha").exists());
}

#[test]
fn cancelling_or_reopening_an_abandoned_zip_plan_removes_its_staging_files() {
    let sandbox = tempdir().unwrap();
    let database = sandbox.path().join("index.sqlite3");
    let root = sandbox.path().join("skills");
    fs::create_dir_all(&root).unwrap();
    let archive = sandbox.path().join("valid.zip");
    write_zip(
        &archive,
        &[(
            "skill/SKILL.md",
            "---\nname: imported\ndescription: ZIP 导入。\n---\n",
            None,
        )],
    );
    let workspace = SkillWorkspace::open(&database).unwrap();
    let root_id = workspace.add_root(&root).unwrap().roots[0].id;
    let request = ZipImportRequest {
        zip_path: archive.to_string_lossy().into_owned(),
        target_root_id: root_id,
        relative_path: "imported".to_owned(),
        conflict_policy: FileConflictPolicy::Skip,
    };
    let cancelled = workspace.preview_zip_import(&request).unwrap();
    let cancelled_source = PathBuf::from(&cancelled.items[0].source);
    assert!(cancelled_source.exists());
    workspace.cancel_file_operation_plan(cancelled.id).unwrap();
    assert!(!cancelled_source.exists());
    assert!(workspace.execute_file_operation_plan(cancelled.id).is_err());

    let abandoned = workspace.preview_zip_import(&request).unwrap();
    let abandoned_source = PathBuf::from(&abandoned.items[0].source);
    drop(workspace);
    let _reopened = SkillWorkspace::open(&database).unwrap();
    assert!(!abandoned_source.exists());
}

#[test]
fn interrupted_trash_keeps_an_uncertain_audit_record_instead_of_claiming_undo() {
    let sandbox = tempdir().unwrap();
    let database = sandbox.path().join("index.sqlite3");
    let root = sandbox.path().join("skills");
    let trash = sandbox.path().join("fake-system-trash");
    write_skill(&root, "alpha", "alpha", "Alpha。", "alpha");
    let workspace = SkillWorkspace::open_with_trash_directory(&database, &trash).unwrap();
    let snapshot = workspace.add_root(&root).unwrap();
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![snapshot.instances[0].id.clone()],
            kind: FileOperationKind::Trash,
            target_root_id: None,
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    let connection = Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "
            CREATE TRIGGER interrupt_trash_result
            BEFORE UPDATE OF results_payload ON file_operation_batches
            WHEN NEW.results_payload LIKE '%\"status\":\"success\"%'
            BEGIN
                SELECT RAISE(ABORT, 'simulated crash after trash');
            END;
            ",
        )
        .unwrap();
    assert!(workspace.execute_file_operation_plan(plan.id).is_err());
    connection
        .execute_batch("DROP TRIGGER interrupt_trash_result;")
        .unwrap();
    drop(connection);
    drop(workspace);

    let reopened = SkillWorkspace::open_with_trash_directory(&database, &trash).unwrap();
    let record = &reopened.file_operation_history().unwrap()[0];
    assert!(!record.undone);
    assert_eq!(record.results[0].status, FileOperationResultStatus::Failed);
    assert!(record.results[0].message.contains("访达的废纸篓中核对"));
    assert!(!root.join("alpha").exists());
    assert_eq!(fs::read_dir(&trash).unwrap().count(), 1);
}

#[test]
fn completed_batches_with_leftover_staging_are_cleaned_without_being_undone() {
    let sandbox = tempdir().unwrap();
    let database = sandbox.path().join("index.sqlite3");
    let workspace = SkillWorkspace::open(&database).unwrap();
    let leftover = database
        .parent()
        .unwrap()
        .join("import-staging/zip-leftover");
    fs::create_dir_all(&leftover).unwrap();
    fs::write(leftover.join("orphan.txt"), "orphan").unwrap();
    Connection::open(&database)
        .unwrap()
        .execute(
            "
            INSERT INTO file_operation_batches (
                plan_id, kind, created_at, undoable, undone, completed, undoing,
                results_payload, undo_payload, plan_payload, staging_root
            ) VALUES (999, 'import', 1, 1, 0, 1, 0, '[]', '{\"items\":[]}', NULL, ?1)
            ",
            [leftover.to_string_lossy().into_owned()],
        )
        .unwrap();
    drop(workspace);

    let reopened = SkillWorkspace::open(&database).unwrap();
    assert!(!leftover.exists());
    let connection = Connection::open(&database).unwrap();
    let (undone, staging_root): (bool, Option<String>) = connection
        .query_row(
            "SELECT undone, staging_root FROM file_operation_batches WHERE plan_id = 999",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(!undone);
    assert!(staging_root.is_none());
    drop(reopened);
}

#[cfg(unix)]
#[test]
fn undo_refuses_paths_whose_ancestors_were_replaced_by_symbolic_links() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    let outside = sandbox.path().join("outside");
    write_skill(
        &source_root.join("namespace"),
        "alpha",
        "alpha",
        "Alpha。",
        "source",
    );
    fs::create_dir_all(&target_root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source.instances[0].id.clone()],
            kind: FileOperationKind::Copy,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    fs::rename(
        target_root.join("namespace"),
        target_root.join("namespace-before-link"),
    )
    .unwrap();
    symlink(&outside, target_root.join("namespace")).unwrap();
    fs::create_dir_all(outside.join("alpha")).unwrap();
    fs::write(outside.join("alpha/sentinel.txt"), "outside").unwrap();

    let error = workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap_err();
    assert!(error.to_string().contains("符号链接目录"));
    assert_eq!(
        fs::read_to_string(outside.join("alpha/sentinel.txt")).unwrap(),
        "outside"
    );
}

#[cfg(unix)]
#[test]
fn execution_rechecks_source_ancestors_changed_after_preview() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    let moved_outside = sandbox.path().join("outside/namespace");
    write_skill(
        &source_root.join("namespace"),
        "alpha",
        "alpha",
        "Alpha。",
        "source",
    );
    fs::create_dir_all(&target_root).unwrap();
    fs::create_dir_all(moved_outside.parent().unwrap()).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source.instances[0].id.clone()],
            kind: FileOperationKind::Move,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    fs::rename(source_root.join("namespace"), &moved_outside).unwrap();
    symlink(&moved_outside, source_root.join("namespace")).unwrap();

    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    assert_eq!(outcome.results[0].status, FileOperationResultStatus::Failed);
    assert!(outcome.results[0].message.contains("符号链接目录"));
    assert!(moved_outside.join("alpha/SKILL.md").exists());
    assert!(!target_root.join("namespace/alpha").exists());
}

#[cfg(unix)]
#[test]
fn move_uses_only_the_application_backup_root_and_rejects_a_linked_app_backup_root() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let database = sandbox.path().join("app-data/index.sqlite3");
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    let outside = sandbox.path().join("outside");
    write_skill(&source_root, "alpha", "alpha", "Alpha。", "source");
    fs::create_dir_all(&target_root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, source_root.join(".skill-management-backups")).unwrap();
    let workspace = SkillWorkspace::open(&database).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let first = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source.instances[0].id.clone()],
            kind: FileOperationKind::Move,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    let moved = workspace.execute_file_operation_plan(first.id).unwrap();
    assert_eq!(moved.results[0].status, FileOperationResultStatus::Success);
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
    workspace.undo_file_operation_batch(moved.batch_id).unwrap();

    let app_backup = database.parent().unwrap().join("file-operation-backups");
    fs::remove_dir_all(&app_backup).unwrap();
    symlink(&outside, &app_backup).unwrap();
    let refreshed = workspace.rescan_all_roots().unwrap();
    let second = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![refreshed.instances[0].id.clone()],
            kind: FileOperationKind::Move,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    let error = workspace
        .execute_file_operation_plan(second.id)
        .unwrap_err();
    assert!(error.to_string().contains("符号链接目录"));
    assert!(source_root.join("alpha/SKILL.md").exists());
    assert!(!target_root.join("alpha").exists());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn undo_restores_a_source_link_even_when_its_target_became_missing() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    let shared_target = sandbox.path().join("shared/alpha");
    write_skill(
        shared_target.parent().unwrap(),
        "alpha",
        "alpha",
        "共享 Alpha。",
        "shared",
    );
    fs::create_dir_all(&source_root).unwrap();
    fs::create_dir_all(&target_root).unwrap();
    symlink(&shared_target, source_root.join("alpha")).unwrap();
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source.instances[0].id.clone()],
            kind: FileOperationKind::Move,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    fs::remove_dir_all(&shared_target).unwrap();

    workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap();
    let restored = source_root.join("alpha");
    assert!(
        restored
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_link(restored).unwrap(), shared_target);
}

#[cfg(unix)]
#[test]
fn undo_rejects_an_application_backup_root_replaced_after_execution() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().unwrap();
    let database = sandbox.path().join("app-data/index.sqlite3");
    let source_root = sandbox.path().join("source/skills");
    let target_root = sandbox.path().join("target/skills");
    let outside = sandbox.path().join("outside");
    write_skill(&source_root, "alpha", "alpha", "Alpha。", "source");
    fs::create_dir_all(&target_root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let workspace = SkillWorkspace::open(&database).unwrap();
    let source = workspace.add_root(&source_root).unwrap();
    let target_root_id = workspace.add_root(&target_root).unwrap().roots[1].id;
    let plan = workspace
        .plan_file_operations(&FileOperationRequest {
            instance_ids: vec![source.instances[0].id.clone()],
            kind: FileOperationKind::Move,
            target_root_id: Some(target_root_id),
            conflict_policy: FileConflictPolicy::Skip,
        })
        .unwrap();
    let outcome = workspace.execute_file_operation_plan(plan.id).unwrap();
    let backup_root = database.parent().unwrap().join("file-operation-backups");
    fs::rename(
        &backup_root,
        database
            .parent()
            .unwrap()
            .join("file-operation-backups-before-link"),
    )
    .unwrap();
    symlink(&outside, &backup_root).unwrap();

    let error = workspace
        .undo_file_operation_batch(outcome.batch_id)
        .unwrap_err();
    assert!(error.to_string().contains("符号链接目录"));
    assert!(!source_root.join("alpha").exists());
    assert!(target_root.join("alpha/SKILL.md").exists());
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
}

#[test]
fn reopening_removes_unreferenced_application_backup_batches() {
    let sandbox = tempdir().unwrap();
    let database = sandbox.path().join("app-data/index.sqlite3");
    let workspace = SkillWorkspace::open(&database).unwrap();
    let orphan = database
        .parent()
        .unwrap()
        .join("file-operation-backups/batch-999/item-0/source");
    fs::create_dir_all(&orphan).unwrap();
    fs::write(orphan.join("partial.txt"), "partial snapshot").unwrap();
    drop(workspace);

    let _reopened = SkillWorkspace::open(&database).unwrap();
    assert!(
        !database
            .parent()
            .unwrap()
            .join("file-operation-backups/batch-999")
            .exists()
    );
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

fn read_skill(root: &Path, directory: &str) -> Vec<u8> {
    fs::read(root.join(directory).join("SKILL.md")).unwrap()
}

fn write_zip(path: &Path, entries: &[(&str, &str, Option<u32>)]) {
    let file = fs::File::create(path).unwrap();
    let mut writer = ZipWriter::new(file);
    for (name, content, mode) in entries {
        let mut options = SimpleFileOptions::default();
        if let Some(mode) = mode {
            options = options.unix_permissions(*mode);
        }
        writer.start_file(*name, options).unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
}
