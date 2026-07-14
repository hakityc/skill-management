use std::{fs, os::unix::fs::symlink};

use skill_workspace::{SkillClient, SkillFileKind, SkillFilePreview, SkillWorkspace};
use tempfile::tempdir;

#[test]
fn personal_user_views_metadata_file_tree_and_safe_file_previews() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join(".codex/skills");
    let skill = root.join("api-review");
    fs::create_dir_all(skill.join("references")).expect("创建 Skill 目录");
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: api-review\ndescription: 审查接口边界。\n---\n\n# API Review\n",
    )
    .expect("写入 SKILL.md");
    fs::write(skill.join("references/guide.md"), "检查幂等性。\n").expect("写入文本文件");
    fs::write(skill.join("preview.png"), [0x89, b'P', b'N', b'G', 0, 0xff])
        .expect("写入二进制文件");
    symlink("missing.txt", skill.join("broken-link")).expect("创建符号链接");

    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描根目录");
    let instance_id = &snapshot.instances[0].id;
    let detail = workspace
        .skill_detail(instance_id)
        .expect("读取 Skill 详情");

    assert_eq!(detail.instance.name, "api-review");
    assert_eq!(detail.instance.client, SkillClient::Codex);
    assert_eq!(
        detail.root.path,
        root.canonicalize().unwrap().to_string_lossy()
    );
    assert_eq!(detail.file_count, 4);
    assert!(detail.instance.modified_at > 0);
    assert_eq!(
        detail
            .files
            .iter()
            .map(|file| (file.relative_path.as_str(), &file.kind))
            .collect::<Vec<_>>(),
        vec![
            ("SKILL.md", &SkillFileKind::Text),
            ("broken-link", &SkillFileKind::SymbolicLink),
            ("preview.png", &SkillFileKind::Binary),
            ("references", &SkillFileKind::Directory),
            ("references/guide.md", &SkillFileKind::Text),
        ]
    );

    assert_eq!(
        workspace
            .read_skill_file(instance_id, "references/guide.md")
            .expect("读取文本预览"),
        SkillFilePreview::Text {
            content: "检查幂等性。\n".to_owned(),
        }
    );
    assert_eq!(
        workspace
            .read_skill_file(instance_id, "preview.png")
            .expect("读取二进制信息"),
        SkillFilePreview::Binary { size: 6 }
    );
    let traversal = workspace
        .read_skill_file(instance_id, "../outside.txt")
        .expect_err("不得读取 Skill 目录之外的文件");
    assert!(traversal.to_string().contains("Skill 目录之外"));
}
