use std::fs;

use skill_workspace::{
    DuplicateCheckStatus, DuplicateDecisionKind, DuplicateFileDifferenceStatus, DuplicateFileKind,
    DuplicateHitRule, SkillWorkspace,
};
use tempfile::tempdir;

#[test]
fn personal_user_reviews_exact_suspected_and_name_conflict_groups_with_file_evidence() {
    let sandbox = tempdir().expect("创建临时工作区");
    let codex = sandbox.path().join(".codex/skills");
    let claude = sandbox.path().join(".claude/skills");
    let gemini = sandbox.path().join(".gemini/skills");

    write_skill(
        &codex,
        "api-review",
        "api-review",
        "审查 API 安全边界。",
        "# API Review\n\n检查鉴权、幂等性、重放保护与错误响应。\n",
    );
    write_skill(
        &claude,
        "api-review-copy",
        "api-review",
        "审查 API 安全边界。",
        "# API Review\n\n检查鉴权、幂等性、重放保护与错误响应。\n",
    );
    fs::write(codex.join("api-review/.DS_Store"), "codex 噪音").unwrap();
    fs::create_dir_all(claude.join("api-review-copy/.git")).unwrap();
    fs::write(claude.join("api-review-copy/.git/config"), "claude 噪音").unwrap();

    write_skill(
        &codex,
        "release-notes",
        "release-notes",
        "整理清晰的版本发布说明。",
        &format!(
            "# 发布说明\n\n{}",
            "收集功能、修复、兼容性和升级步骤，并输出清晰的版本摘要。\n".repeat(6)
        ),
    );
    write_skill(
        &gemini,
        "release-notes-v2",
        "Release Notes",
        "整理清晰且可核对的版本发布说明。",
        &format!(
            "# 发布说明\n\n{}",
            "收集功能、修复、兼容性和升级步骤，并输出清晰的版本摘要。\n".repeat(6)
        ),
    );
    fs::create_dir_all(codex.join("release-notes/references")).unwrap();
    fs::write(
        codex.join("release-notes/references/template.md"),
        "## 功能\n## 修复\n",
    )
    .unwrap();
    fs::write(
        codex.join("release-notes/references/legacy.md"),
        "旧版本兼容说明。\n",
    )
    .unwrap();
    fs::create_dir_all(gemini.join("release-notes-v2/references")).unwrap();
    fs::write(
        gemini.join("release-notes-v2/references/template.md"),
        "## 功能\n## 修复与升级\n",
    )
    .unwrap();

    write_skill(
        &claude,
        "auth-helper",
        "auth-helper",
        "生成 OAuth 鉴权流程。",
        "# OAuth\n\n生成登录、令牌刷新和退出流程。\n",
    );
    write_skill(
        &gemini,
        "auth-helper-other",
        "Auth Helper",
        "管理本地图片素材。",
        "# 图片整理\n\n压缩 PNG，重命名截图并生成缩略图。\n",
    );
    fs::write(claude.join("auth-helper/icon.bin"), [0, 1, 2, 3]).unwrap();
    fs::write(gemini.join("auth-helper-other/icon.bin"), [9, 8, 7, 6, 5]).unwrap();

    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    workspace.add_root(&codex).unwrap();
    workspace.add_root(&claude).unwrap();
    workspace.add_root(&gemini).unwrap();
    let review = workspace
        .review_duplicate_groups()
        .expect("生成重复检查结果");

    assert_eq!(review.groups.len(), 3);
    let exact = review
        .groups
        .iter()
        .find(|group| group.status == DuplicateCheckStatus::Exact)
        .expect("完全重复 Skill 组");
    assert_eq!(exact.instances.len(), 2);
    assert_eq!(exact.similarity, 1.0);
    assert!(exact.hit_rules.contains(&DuplicateHitRule::ExactContent));
    assert_eq!(exact.fingerprint_files, vec!["SKILL.md"]);
    assert!(
        exact.comparisons[0]
            .files
            .iter()
            .all(|file| file.status == DuplicateFileDifferenceStatus::Identical)
    );

    let suspected = review
        .groups
        .iter()
        .find(|group| group.status == DuplicateCheckStatus::Suspected)
        .expect("疑似重复 Skill 组");
    assert!(suspected.similarity >= 0.82);
    assert!(
        suspected
            .hit_rules
            .contains(&DuplicateHitRule::NormalizedName)
    );
    let template_diff = suspected.comparisons[0]
        .files
        .iter()
        .find(|file| file.relative_path == "references/template.md")
        .expect("模板文本差异");
    assert_eq!(
        template_diff.status,
        DuplicateFileDifferenceStatus::Modified
    );
    assert_eq!(template_diff.kind, DuplicateFileKind::Text);
    assert!(
        template_diff
            .text_diff
            .as_ref()
            .is_some_and(|lines| !lines.is_empty())
    );
    assert!(suspected.comparisons[0].files.iter().any(|file| {
        file.relative_path == "references/legacy.md"
            && matches!(
                file.status,
                DuplicateFileDifferenceStatus::OnlyLeft | DuplicateFileDifferenceStatus::OnlyRight
            )
    }));

    let conflict = review
        .groups
        .iter()
        .find(|group| group.status == DuplicateCheckStatus::NameConflict)
        .expect("同名冲突 Skill 组");
    assert!(conflict.similarity < 0.82);
    assert!(
        conflict
            .hit_rules
            .contains(&DuplicateHitRule::NormalizedName)
    );
    let binary_diff = conflict.comparisons[0]
        .files
        .iter()
        .find(|file| file.relative_path == "icon.bin")
        .expect("二进制差异");
    assert_eq!(binary_diff.kind, DuplicateFileKind::Binary);
    assert_eq!(binary_diff.status, DuplicateFileDifferenceStatus::Modified);
    assert!(binary_diff.text_diff.is_none());
    assert_ne!(binary_diff.left_fingerprint, binary_diff.right_fingerprint);

    let snapshot = workspace.snapshot().expect("读取更新后的实例状态");
    assert_eq!(
        snapshot
            .instances
            .iter()
            .filter(|instance| instance.duplicate_check_status != DuplicateCheckStatus::None)
            .count(),
        6
    );
}

#[test]
fn similarity_threshold_includes_exact_point_eight_two_and_excludes_the_lower_fixture() {
    let sandbox = tempdir().expect("创建临时工作区");
    let left_root = sandbox.path().join("at-threshold/left/skills");
    let right_root = sandbox.path().join("at-threshold/right/skills");
    write_skill(
        &left_root,
        "alpha",
        "boundary-review",
        "检查边界。",
        &format!("{}{}", "共".repeat(24), "甲".repeat(17)),
    );
    write_skill(
        &right_root,
        "beta",
        "boundary-review",
        "检查边界。",
        &format!("{}{}", "共".repeat(24), "乙".repeat(17)),
    );
    let workspace = SkillWorkspace::open(sandbox.path().join("at-threshold.sqlite3")).unwrap();
    let left = workspace.add_root(&left_root).unwrap().instances[0]
        .id
        .clone();
    let right = workspace.add_root(&right_root).unwrap().instances[1]
        .id
        .clone();
    let comparison = workspace
        .compare_skill_instances(&left, &right)
        .expect("比较阈值夹具");
    assert_eq!(comparison.similarity, 0.82);
    assert_eq!(comparison.status, DuplicateCheckStatus::Suspected);
    assert!(
        comparison
            .hit_rules
            .contains(&DuplicateHitRule::ContentSimilarity)
    );

    let below_left_root = sandbox.path().join("below-threshold/left/skills");
    let below_right_root = sandbox.path().join("below-threshold/right/skills");
    write_skill(
        &below_left_root,
        "alpha",
        "boundary-review",
        "检查边界。",
        &format!("{}{}", "共".repeat(24), "甲".repeat(18)),
    );
    write_skill(
        &below_right_root,
        "beta",
        "boundary-review",
        "检查边界。",
        &format!("{}{}", "共".repeat(24), "乙".repeat(18)),
    );
    let below_workspace =
        SkillWorkspace::open(sandbox.path().join("below-threshold.sqlite3")).unwrap();
    let below_left = below_workspace
        .add_root(&below_left_root)
        .unwrap()
        .instances[0]
        .id
        .clone();
    let below_right = below_workspace
        .add_root(&below_right_root)
        .unwrap()
        .instances[1]
        .id
        .clone();
    let below = below_workspace
        .compare_skill_instances(&below_left, &below_right)
        .expect("比较阈值下方夹具");
    assert!(below.similarity < 0.82);
    assert_eq!(below.status, DuplicateCheckStatus::NameConflict);
    assert!(
        !below
            .hit_rules
            .contains(&DuplicateHitRule::ContentSimilarity)
    );
}

#[test]
fn oversized_text_diffs_are_bounded_and_report_truncation() {
    let sandbox = tempdir().expect("创建临时工作区");
    let left_root = sandbox.path().join("left/skills");
    let right_root = sandbox.path().join("right/skills");
    write_skill(
        &left_root,
        "long",
        "long-review",
        "长文本审阅。",
        "共同正文。\n",
    );
    write_skill(
        &right_root,
        "long",
        "Long Review",
        "长文本审阅。",
        "共同正文。\n",
    );
    fs::write(
        left_root.join("long/large.md"),
        (0..1_100)
            .map(|line| format!("左侧第 {line} 行\n"))
            .collect::<String>(),
    )
    .unwrap();
    fs::write(
        right_root.join("long/large.md"),
        (0..1_100)
            .map(|line| format!("右侧第 {line} 行\n"))
            .collect::<String>(),
    )
    .unwrap();

    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    let left = workspace.add_root(&left_root).unwrap().instances[0]
        .id
        .clone();
    let right = workspace.add_root(&right_root).unwrap().instances[1]
        .id
        .clone();
    let comparison = workspace
        .compare_skill_instances(&left, &right)
        .expect("比较超长文本");
    let large_diff = comparison
        .files
        .iter()
        .find(|file| file.relative_path == "large.md")
        .expect("超长文本差异");

    assert!(large_diff.text_diff_truncated);
    assert!(
        large_diff
            .text_diff
            .as_ref()
            .is_some_and(|lines| lines.len() <= 1_000)
    );
}

#[test]
fn personal_user_decisions_survive_rescan_and_can_be_restored_from_settings() {
    let sandbox = tempdir().expect("创建临时工作区");
    let left_root = sandbox.path().join(".codex/skills");
    let right_root = sandbox.path().join(".claude/skills");
    write_skill(
        &left_root,
        "deploy",
        "deploy-safe",
        "安全部署。",
        "# Deploy\n\n先检查再发布。\n",
    );
    write_skill(
        &right_root,
        "deploy-copy",
        "deploy-safe",
        "安全部署。",
        "# Deploy\n\n先检查再发布。\n",
    );
    write_skill(
        &left_root,
        "image",
        "image-helper",
        "图片处理。",
        "# Image\n\n压缩图片。\n",
    );
    write_skill(
        &right_root,
        "image-other",
        "Image Helper",
        "图片处理。",
        "# Image\n\n压缩图片并生成预览。\n",
    );
    let workspace = SkillWorkspace::open(sandbox.path().join("index.sqlite3")).unwrap();
    workspace.add_root(&left_root).unwrap();
    workspace.add_root(&right_root).unwrap();
    let review = workspace.review_duplicate_groups().unwrap();
    assert_eq!(review.groups.len(), 2);

    let exact_ids = review
        .groups
        .iter()
        .find(|group| group.status == DuplicateCheckStatus::Exact)
        .unwrap()
        .instances
        .iter()
        .map(|instance| instance.id.clone())
        .collect::<Vec<_>>();
    let suspected_ids = review
        .groups
        .iter()
        .find(|group| group.status == DuplicateCheckStatus::Suspected)
        .unwrap()
        .instances
        .iter()
        .map(|instance| instance.id.clone())
        .collect::<Vec<_>>();
    workspace
        .save_duplicate_decision(&exact_ids, DuplicateDecisionKind::NotDuplicate)
        .expect("标记不是重复");
    workspace
        .save_duplicate_decision(&suspected_ids, DuplicateDecisionKind::Ignored)
        .expect("暂时忽略");
    workspace.rescan_all_roots().expect("重新扫描");
    assert!(
        workspace
            .review_duplicate_groups()
            .unwrap()
            .groups
            .is_empty()
    );

    let decisions = workspace.duplicate_decisions().expect("查看设置中的忽略项");
    assert_eq!(decisions.len(), 2);
    assert!(
        decisions
            .iter()
            .any(|decision| decision.kind == DuplicateDecisionKind::NotDuplicate)
    );
    assert!(
        decisions
            .iter()
            .any(|decision| decision.kind == DuplicateDecisionKind::Ignored)
    );
    for decision in decisions {
        workspace
            .restore_duplicate_decision(decision.id)
            .expect("恢复重复检查结果");
    }
    assert_eq!(workspace.review_duplicate_groups().unwrap().groups.len(), 2);
}

fn write_skill(root: &std::path::Path, directory: &str, name: &str, description: &str, body: &str) {
    let path = root.join(directory);
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}"),
    )
    .unwrap();
}
