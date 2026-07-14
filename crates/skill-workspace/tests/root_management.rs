use std::fs;

use skill_workspace::{SkillRootStatus, SkillWorkspace};
use tempfile::tempdir;

#[test]
fn personal_user_manages_multiple_roots_without_deleting_skill_files() {
    let sandbox = tempdir().expect("创建临时工作区");
    let first_root = sandbox.path().join("codex-skills");
    let second_root = sandbox.path().join("claude-skills");
    let database_path = sandbox.path().join("index.sqlite3");
    write_skill(&first_root.join("shared"), "来自 Codex 根目录");
    write_skill(&second_root.join("shared"), "来自 Claude 根目录");

    let workspace = SkillWorkspace::open(&database_path).expect("打开 SkillWorkspace");
    workspace.add_root(&first_root).expect("添加第一个根目录");
    let snapshot = workspace.add_root(&second_root).expect("添加第二个根目录");

    assert_eq!(snapshot.roots.len(), 2);
    assert_eq!(snapshot.instances.len(), 2);
    assert_ne!(snapshot.instances[0].id, snapshot.instances[1].id);
    let first_root_id = snapshot
        .roots
        .iter()
        .find(|root| root.path.ends_with("codex-skills"))
        .expect("找到第一个根目录")
        .id;

    let after_removal = workspace
        .remove_root(first_root_id)
        .expect("移除第一个根目录的纳管记录");

    assert_eq!(after_removal.roots.len(), 1);
    assert_eq!(after_removal.instances.len(), 1);
    assert!(first_root.join("shared/SKILL.md").exists());
    assert_eq!(
        fs::read_to_string(first_root.join("shared/SKILL.md")).expect("原文件仍可读取"),
        "---\nname: shared\ndescription: 来自 Codex 根目录\n---\n"
    );

    drop(workspace);
    let reopened = SkillWorkspace::open(&database_path).expect("重新打开 SkillWorkspace");
    let restored = reopened.snapshot().expect("恢复多根目录索引");
    assert_eq!(restored.roots.len(), 1);
    assert!(restored.roots[0].path.ends_with("claude-skills"));
    assert_eq!(restored.instances.len(), 1);
}

#[test]
fn scanner_respects_skill_boundaries_and_ignored_directories() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    write_skill(&root.join("visible-skill"), "应被发现");
    write_skill(
        &root.join("visible-skill/nested-should-stop"),
        "Skill 内部不应继续扫描",
    );
    for ignored in [
        ".git",
        "node_modules",
        ".cache",
        "__pycache__",
        ".pytest_cache",
        ".mypy_cache",
        ".npm",
        ".Spotlight-V100",
        ".Trash",
    ] {
        write_skill(&root.join(ignored).join("hidden-skill"), "应被忽略");
    }

    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描 Skill 根目录");

    assert_eq!(snapshot.instances.len(), 1);
    assert_eq!(snapshot.instances[0].relative_path, "visible-skill");
}

#[cfg(unix)]
#[test]
fn scanner_reports_symbolic_link_locations_and_targets_without_following_loops() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    let target = root.join("source/linked-skill");
    let alias = root.join("aliases/linked-skill");
    write_skill(&target, "符号链接目标");
    fs::create_dir_all(alias.parent().expect("别名父目录")).expect("创建别名目录");
    symlink(&target, &alias).expect("创建 Skill 符号链接");
    let loop_directory = root.join("loop");
    fs::create_dir_all(&loop_directory).expect("创建循环目录");
    symlink(&loop_directory, loop_directory.join("back")).expect("创建循环符号链接");

    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描含符号链接的根目录");

    assert_eq!(snapshot.instances.len(), 2);
    let linked = snapshot
        .instances
        .iter()
        .find(|skill| skill.relative_path == "aliases/linked-skill")
        .expect("符号链接位置应作为独立实例进入列表");
    let canonical_alias = root
        .canonicalize()
        .expect("规范化根目录")
        .join("aliases/linked-skill");
    assert_eq!(
        linked.link_path.as_deref(),
        Some(canonical_alias.to_string_lossy().as_ref())
    );
    assert_eq!(
        std::path::Path::new(&linked.real_path),
        target.canonicalize().expect("规范化真实目标")
    );
}

#[cfg(unix)]
#[test]
fn rescanning_all_roots_keeps_healthy_results_when_other_paths_fail() {
    use std::os::unix::fs::symlink;

    let sandbox = tempdir().expect("创建临时工作区");
    let healthy_root = sandbox.path().join("healthy-skills");
    let disappearing_root = sandbox.path().join("disappearing-skills");
    let database_path = sandbox.path().join("index.sqlite3");
    write_skill(&healthy_root.join("healthy"), "扫描前");
    write_skill(&disappearing_root.join("stale"), "路径随后消失");
    let workspace = SkillWorkspace::open(&database_path).expect("打开 SkillWorkspace");
    workspace.add_root(&healthy_root).expect("添加健康根目录");
    workspace
        .add_root(&disappearing_root)
        .expect("添加稍后消失的根目录");

    write_skill(&healthy_root.join("healthy"), "重新扫描后");
    let broken_link = healthy_root.join("broken-directory-link");
    symlink(healthy_root.join("missing-target"), &broken_link).expect("创建损坏目录链接");
    fs::remove_dir_all(&disappearing_root).expect("模拟根目录消失");

    let snapshot = workspace.rescan_all_roots().expect("其余根目录应继续扫描");

    let partial = root_ending_with(&snapshot.roots, "healthy-skills");
    assert_eq!(partial.status, SkillRootStatus::PartialFailure);
    assert!(
        partial
            .error
            .as_deref()
            .is_some_and(|error| error.contains("broken-directory-link"))
    );
    assert!(partial.recovery_hint.is_some());
    let healthy = snapshot
        .instances
        .iter()
        .find(|skill| skill.root_id == partial.id && skill.name == "shared")
        .expect("部分失败根目录中的健康 Skill 仍应更新");
    assert_eq!(healthy.description, "重新扫描后");

    let missing = root_ending_with(&snapshot.roots, "disappearing-skills");
    assert_eq!(missing.status, SkillRootStatus::Missing);
    assert!(
        missing
            .error
            .as_deref()
            .is_some_and(|error| error.contains("不存在"))
    );
    assert!(missing.recovery_hint.is_some());
}

#[cfg(unix)]
#[test]
fn inaccessible_root_reports_permission_status_and_recovery_hint() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("protected-skills");
    write_skill(&root.join("protected"), "稍后失去权限");
    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    let initial = workspace.add_root(&root).expect("添加根目录");
    let root_id = initial.roots[0].id;

    fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).expect("撤销目录权限");
    let snapshot = workspace
        .rescan_root(root_id)
        .expect("权限错误应成为根目录状态");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).expect("恢复目录权限");

    assert_eq!(snapshot.roots[0].status, SkillRootStatus::PermissionDenied);
    assert!(
        snapshot.roots[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("权限"))
    );
    assert!(snapshot.roots[0].recovery_hint.is_some());
}

#[test]
fn missing_root_stays_managed_and_recovers_after_the_path_returns() {
    let sandbox = tempdir().expect("创建临时工作区");
    let missing_root = sandbox.path().join("temporarily-missing");
    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");

    let missing = workspace
        .add_root(&missing_root)
        .expect("消失路径仍应保留为可恢复根目录");

    assert_eq!(missing.roots.len(), 1);
    assert_eq!(missing.roots[0].status, SkillRootStatus::Missing);
    assert!(missing.roots[0].recovery_hint.is_some());

    write_skill(&missing_root.join("returned"), "路径已经恢复");
    let recovered = workspace
        .rescan_root(missing.roots[0].id)
        .expect("路径恢复后重新扫描");
    assert_eq!(recovered.roots[0].status, SkillRootStatus::Ready);
    assert_eq!(recovered.instances.len(), 1);
}

fn root_ending_with<'a>(
    roots: &'a [skill_workspace::SkillRoot],
    suffix: &str,
) -> &'a skill_workspace::SkillRoot {
    roots
        .iter()
        .find(|root| root.path.ends_with(suffix))
        .expect("找到预期根目录")
}

fn write_skill(directory: &std::path::Path, description: &str) {
    fs::create_dir_all(directory).expect("创建 Skill 目录");
    fs::write(
        directory.join("SKILL.md"),
        format!("---\nname: shared\ndescription: {description}\n---\n"),
    )
    .expect("写入 SKILL.md");
}
