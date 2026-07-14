use std::fs;

use skill_workspace::{SkillStatus, SkillWorkspace};
use tempfile::tempdir;

#[test]
fn personal_user_can_authorize_a_root_and_see_ready_and_repairable_skill_instances() {
    let sandbox = tempdir().expect("创建临时工作区");
    let authorized_root = sandbox.path().join("authorized");
    let outside_root = sandbox.path().join("outside");

    write_skill(
        &authorized_root.join("nested/api-review"),
        "---\nname: api-review\ndescription: 审查 API 设计与接口边界。\n---\n\n# API Review\n",
    );
    write_skill(
        &authorized_root.join("broken-skill"),
        "---\nname: broken-skill\n---\n\n# Broken\n",
    );
    write_skill(
        &outside_root.join("private-skill"),
        "---\nname: private-skill\ndescription: 不应被读取。\n---\n",
    );

    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    let snapshot = workspace
        .authorize_root(&authorized_root)
        .expect("授权并扫描 Skill 根目录");

    assert_eq!(snapshot.instances.len(), 2);

    let ready = snapshot
        .instances
        .iter()
        .find(|skill| skill.name == "api-review")
        .expect("合法 Skill 应进入列表");
    assert_eq!(ready.status, SkillStatus::Ready);
    assert_eq!(ready.description, "审查 API 设计与接口边界。");
    assert_eq!(ready.relative_path, "nested/api-review");

    let repairable = snapshot
        .instances
        .iter()
        .find(|skill| skill.name == "broken-skill")
        .expect("损坏 Skill 仍应进入列表");
    assert_eq!(repairable.status, SkillStatus::NeedsRepair);
    assert!(
        repairable
            .error
            .as_deref()
            .is_some_and(|error| error.contains("description")),
        "错误摘要应指出缺少 description"
    );

    assert!(
        snapshot
            .instances
            .iter()
            .all(|skill| skill.name != "private-skill"),
        "授权目录外的 Skill 不得进入结果"
    );
}

#[test]
fn personal_user_recovers_the_authorized_root_and_index_after_reopening() {
    let sandbox = tempdir().expect("创建临时工作区");
    let authorized_root = sandbox.path().join("authorized");
    let database_path = sandbox.path().join("index.sqlite3");
    write_skill(
        &authorized_root.join("release-notes"),
        "---\nname: release-notes\ndescription: 整理版本发布说明。\n---\n",
    );

    {
        let workspace = SkillWorkspace::open(&database_path).expect("打开 SkillWorkspace");
        workspace
            .authorize_root(&authorized_root)
            .expect("授权并扫描 Skill 根目录");
    }

    let reopened = SkillWorkspace::open(&database_path).expect("重新打开 SkillWorkspace");
    let snapshot = reopened.snapshot().expect("恢复本地索引");

    assert_eq!(
        snapshot
            .authorized_root
            .as_deref()
            .map(std::path::Path::new),
        Some(
            authorized_root
                .canonicalize()
                .expect("规范化授权目录")
                .as_path()
        )
    );
    assert_eq!(snapshot.instances.len(), 1);
    assert_eq!(snapshot.instances[0].name, "release-notes");
    assert_eq!(snapshot.instances[0].status, SkillStatus::Ready);
}

#[test]
fn unreadable_skill_document_is_repairable_without_hiding_healthy_siblings() {
    let sandbox = tempdir().expect("创建临时工作区");
    let authorized_root = sandbox.path().join("authorized");
    write_skill(
        &authorized_root.join("healthy-skill"),
        "---\nname: healthy-skill\ndescription: 可以正常读取。\n---\n",
    );
    let unreadable_directory = authorized_root.join("binary-skill");
    fs::create_dir_all(&unreadable_directory).expect("创建损坏 Skill 目录");
    fs::write(unreadable_directory.join("SKILL.md"), [0xff, 0xfe, 0xfd])
        .expect("写入非 UTF-8 SKILL.md");

    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    let snapshot = workspace
        .authorize_root(&authorized_root)
        .expect("单个损坏文档不应中止扫描");

    assert_eq!(snapshot.instances.len(), 2);
    let unreadable = snapshot
        .instances
        .iter()
        .find(|skill| skill.name == "binary-skill")
        .expect("无法读取的 Skill 仍应进入列表");
    assert_eq!(unreadable.status, SkillStatus::NeedsRepair);
    assert_eq!(
        unreadable.error.as_deref(),
        Some("SKILL.md 不是有效的 UTF-8 文本")
    );
    assert!(
        snapshot
            .instances
            .iter()
            .any(|skill| skill.name == "healthy-skill"),
        "同目录的合法 Skill 不应被隐藏"
    );
}

fn write_skill(directory: &std::path::Path, content: &str) {
    fs::create_dir_all(directory).expect("创建 Skill 目录");
    fs::write(directory.join("SKILL.md"), content).expect("写入 SKILL.md");
}
