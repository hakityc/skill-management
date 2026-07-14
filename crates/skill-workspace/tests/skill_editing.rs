use std::{fs, os::unix::fs::symlink};

use skill_workspace::{
    SkillChangeKind, SkillDraft, SkillDraftTarget, SkillFileDraftChange, SkillFileDraftOperation,
    SkillWorkspace,
};
use tempfile::tempdir;

#[test]
fn personal_user_validates_a_draft_and_previews_an_immutable_change_plan() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    let directory = root.join("api-review");
    fs::create_dir_all(&directory).expect("创建 Skill 目录");
    fs::write(
        directory.join("SKILL.md"),
        "---\nname: api-review\ndescription: 旧描述。\n---\n\n# API Review\n",
    )
    .expect("写入 SKILL.md");
    fs::write(directory.join("asset.bin"), [0, 1, 2]).expect("写入二进制附件");
    fs::write(directory.join("obsolete.txt"), "待删除\n").expect("写入待删除文件");
    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描根目录");
    let instance_id = snapshot.instances[0].id.clone();

    let invalid = SkillDraft {
        target: SkillDraftTarget::Existing {
            instance_id: instance_id.clone(),
        },
        name: "api-review".to_owned(),
        description: "".to_owned(),
        markdown_body: "# API Review\n".to_owned(),
        file_changes: vec![],
    };
    let validation = workspace.validate_skill_draft(&invalid);
    assert!(!validation.valid);
    assert!(
        validation
            .issues
            .iter()
            .any(|issue| issue.field == "description" && issue.message.contains("描述"))
    );

    let draft = SkillDraft {
        target: SkillDraftTarget::Existing { instance_id },
        name: "api-review".to_owned(),
        description: "审查 API 设计与接口边界。".to_owned(),
        markdown_body: "# API Review\n\n检查幂等性。\n".to_owned(),
        file_changes: vec![
            SkillFileDraftChange {
                relative_path: "references/checklist.md".to_owned(),
                operation: SkillFileDraftOperation::WriteText {
                    content: "- [ ] 鉴权\n".to_owned(),
                },
            },
            SkillFileDraftChange {
                relative_path: "asset.bin".to_owned(),
                operation: SkillFileDraftOperation::ReplaceBinary {
                    content: vec![3, 4, 5, 6],
                },
            },
            SkillFileDraftChange {
                relative_path: "obsolete.txt".to_owned(),
                operation: SkillFileDraftOperation::Delete,
            },
        ],
    };
    let plan = workspace
        .plan_skill_change(&draft)
        .expect("生成不可变变化计划");

    assert_eq!(
        plan.changes
            .iter()
            .map(|change| (change.relative_path.as_str(), &change.kind))
            .collect::<Vec<_>>(),
        vec![
            ("SKILL.md", &SkillChangeKind::Overwrite),
            ("asset.bin", &SkillChangeKind::Overwrite),
            ("obsolete.txt", &SkillChangeKind::Delete),
            ("references/checklist.md", &SkillChangeKind::Create),
        ]
    );
    assert!(plan.id > 0);
    assert_eq!(
        fs::read_to_string(directory.join("SKILL.md")).unwrap(),
        "---\nname: api-review\ndescription: 旧描述。\n---\n\n# API Review\n"
    );
    assert!(!directory.join("references/checklist.md").exists());
    assert!(directory.join("obsolete.txt").exists());
}

#[test]
fn personal_user_confirms_an_atomic_edit_and_undoes_it_with_the_index_restored() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    let directory = root.join("api-review");
    fs::create_dir_all(&directory).expect("创建 Skill 目录");
    let original_document = "---\nname: api-review\ndescription: 旧描述。\n---\n\n# API Review\n";
    fs::write(directory.join("SKILL.md"), original_document).expect("写入 SKILL.md");
    fs::write(directory.join("asset.bin"), [0, 1, 2]).expect("写入二进制附件");
    fs::write(directory.join("obsolete.txt"), "待删除\n").expect("写入待删除文件");
    let workspace = SkillWorkspace::open(sandbox.path().join("app/index.sqlite3"))
        .expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描根目录");
    let instance_id = snapshot.instances[0].id.clone();
    let plan = workspace
        .plan_skill_change(&SkillDraft {
            target: SkillDraftTarget::Existing { instance_id },
            name: "api-review".to_owned(),
            description: "审查安全回放边界。".to_owned(),
            markdown_body: "# API Review\n\n检查 replay protection。\n".to_owned(),
            file_changes: vec![
                SkillFileDraftChange {
                    relative_path: "references/checklist.md".to_owned(),
                    operation: SkillFileDraftOperation::WriteText {
                        content: "- [ ] 鉴权\n".to_owned(),
                    },
                },
                SkillFileDraftChange {
                    relative_path: "asset.bin".to_owned(),
                    operation: SkillFileDraftOperation::ReplaceBinary {
                        content: vec![3, 4, 5, 6],
                    },
                },
                SkillFileDraftChange {
                    relative_path: "obsolete.txt".to_owned(),
                    operation: SkillFileDraftOperation::Delete,
                },
            ],
        })
        .expect("生成变化计划");

    let outcome = workspace
        .execute_skill_change(plan.id)
        .expect("确认并原子执行变化计划");
    assert!(outcome.operation_id > 0);
    assert_eq!(fs::read(directory.join("asset.bin")).unwrap(), [3, 4, 5, 6]);
    assert_eq!(
        fs::read_to_string(directory.join("references/checklist.md")).unwrap(),
        "- [ ] 鉴权\n"
    );
    assert!(!directory.join("obsolete.txt").exists());
    assert_eq!(
        workspace
            .search_skills(&skill_workspace::SkillQuery {
                text: "replay protection".to_owned(),
                ..skill_workspace::SkillQuery::default()
            })
            .expect("检索更新后的索引")
            .total,
        1
    );

    workspace
        .undo_skill_change(outcome.operation_id)
        .expect("撤销最近编辑");
    assert_eq!(
        fs::read_to_string(directory.join("SKILL.md")).unwrap(),
        original_document
    );
    assert_eq!(fs::read(directory.join("asset.bin")).unwrap(), [0, 1, 2]);
    assert!(directory.join("obsolete.txt").exists());
    assert!(!directory.join("references/checklist.md").exists());
    assert_eq!(
        workspace
            .search_skills(&skill_workspace::SkillQuery {
                text: "replay protection".to_owned(),
                ..skill_workspace::SkillQuery::default()
            })
            .expect("检索撤销后的索引")
            .total,
        0
    );
}

#[test]
fn personal_user_creates_a_new_skill_and_can_undo_the_creation() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    fs::create_dir_all(&root).expect("创建 Skill 根目录");
    let workspace = SkillWorkspace::open(sandbox.path().join("app/index.sqlite3"))
        .expect("打开 SkillWorkspace");
    let root_id = workspace.add_root(&root).expect("添加根目录").roots[0].id;
    let plan = workspace
        .plan_skill_change(&SkillDraft {
            target: SkillDraftTarget::New {
                root_id,
                relative_path: "release-notes".to_owned(),
            },
            name: "release-notes".to_owned(),
            description: "整理版本发布说明。".to_owned(),
            markdown_body: "# Release Notes\n".to_owned(),
            file_changes: vec![SkillFileDraftChange {
                relative_path: "references/template.md".to_owned(),
                operation: SkillFileDraftOperation::WriteText {
                    content: "# 模板\n".to_owned(),
                },
            }],
        })
        .expect("预览新建计划");
    assert!(
        plan.changes
            .iter()
            .all(|change| change.kind == SkillChangeKind::Create)
    );

    let outcome = workspace.execute_skill_change(plan.id).expect("创建 Skill");
    assert_eq!(outcome.snapshot.instances.len(), 1);
    assert_eq!(outcome.snapshot.instances[0].name, "release-notes");
    assert!(root.join("release-notes/references/template.md").exists());

    let undone = workspace
        .undo_skill_change(outcome.operation_id)
        .expect("撤销新建 Skill");
    assert!(undone.snapshot.instances.is_empty());
    assert!(!root.join("release-notes").exists());
}

#[test]
fn failed_edit_through_a_symbolic_link_leaves_every_real_file_unchanged() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    let directory = root.join("api-review");
    let outside = sandbox.path().join("outside");
    fs::create_dir_all(&directory).expect("创建 Skill 目录");
    fs::create_dir_all(&outside).expect("创建外部目录");
    let original_document = "---\nname: api-review\ndescription: 原始描述。\n---\n\n# API Review\n";
    fs::write(directory.join("SKILL.md"), original_document).expect("写入 SKILL.md");
    symlink(&outside, directory.join("references")).expect("创建越界符号链接");
    let workspace = SkillWorkspace::open(sandbox.path().join("app/index.sqlite3"))
        .expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描根目录");
    let plan = workspace
        .plan_skill_change(&SkillDraft {
            target: SkillDraftTarget::Existing {
                instance_id: snapshot.instances[0].id.clone(),
            },
            name: "api-review".to_owned(),
            description: "不应落盘的新描述。".to_owned(),
            markdown_body: "# API Review\n\n不应落盘。\n".to_owned(),
            file_changes: vec![SkillFileDraftChange {
                relative_path: "references/new.md".to_owned(),
                operation: SkillFileDraftOperation::WriteText {
                    content: "不得写到外部。\n".to_owned(),
                },
            }],
        })
        .expect("预览包含越界符号链接的计划");

    let error = workspace
        .execute_skill_change(plan.id)
        .expect_err("整批编辑必须安全失败");
    assert!(error.to_string().contains("符号链接"));
    assert_eq!(
        fs::read_to_string(directory.join("SKILL.md")).unwrap(),
        original_document
    );
    assert!(!outside.join("new.md").exists());
}

#[test]
fn stale_change_plan_never_overwrites_an_external_file_change() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    let directory = root.join("api-review");
    fs::create_dir_all(&directory).expect("创建 Skill 目录");
    fs::write(
        directory.join("SKILL.md"),
        "---\nname: api-review\ndescription: 原始描述。\n---\n\n# 原始正文\n",
    )
    .expect("写入原始 SKILL.md");
    let workspace = SkillWorkspace::open(sandbox.path().join("app/index.sqlite3"))
        .expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描根目录");
    let plan = workspace
        .plan_skill_change(&SkillDraft {
            target: SkillDraftTarget::Existing {
                instance_id: snapshot.instances[0].id.clone(),
            },
            name: "api-review".to_owned(),
            description: "计划中的描述。".to_owned(),
            markdown_body: "# 计划中的正文\n".to_owned(),
            file_changes: vec![SkillFileDraftChange {
                relative_path: "new.md".to_owned(),
                operation: SkillFileDraftOperation::WriteText {
                    content: "不应写入\n".to_owned(),
                },
            }],
        })
        .expect("生成变化计划");

    let external_document =
        "---\nname: api-review\ndescription: 外部编辑。\n---\n\n# 外部最新正文\n";
    fs::write(directory.join("SKILL.md"), external_document).expect("模拟外部编辑");

    let error = workspace
        .execute_skill_change(plan.id)
        .expect_err("过期计划必须拒绝执行");
    assert!(error.to_string().contains("过期"));
    assert_eq!(
        fs::read_to_string(directory.join("SKILL.md")).unwrap(),
        external_document
    );
    assert!(!directory.join("new.md").exists());
}
