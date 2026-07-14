use std::{
    fs::{self, FileTimes, OpenOptions},
    time::{Duration, Instant, UNIX_EPOCH},
};

use skill_workspace::{
    DuplicateCheckStatus, DuplicateCheckStatusUpdate, SkillClient, SkillFilters, SkillListDensity,
    SkillOrganizationSearchTermsUpdate, SkillQuery, SkillRepairFilter, SkillSort,
    SkillSortDirection, SkillSortField, SkillWorkspace, SkillWorkspaceViewPreferences,
};
use tempfile::tempdir;

#[test]
fn personal_user_searches_name_description_body_and_path_after_content_changes() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    let database_path = sandbox.path().join("index.sqlite3");
    write_skill(
        &root.join("api-review"),
        "api-review",
        "审查接口边界。",
        "重点检查 idempotency keys。",
    );
    write_skill(
        &root.join("release-notes"),
        "release-notes",
        "整理版本发布说明。",
        "汇总本周功能。",
    );
    write_skill(
        &root.join("teams/payments-guide"),
        "billing-guide",
        "维护计费文档。",
        "记录账单流程。",
    );
    let workspace = SkillWorkspace::open(&database_path).expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描根目录");
    let root_id = snapshot.roots[0].id;

    assert_names(&workspace, "api-review", &["api-review"]);
    assert_names(&workspace, "接口", &["api-review"]);
    assert_names(&workspace, "发布说明", &["release-notes"]);
    assert_names(&workspace, "idempotency", &["api-review"]);
    assert_names(&workspace, "payments", &["billing-guide"]);

    write_skill(
        &root.join("api-review"),
        "api-review",
        "审查接口边界。",
        "改为检查 replay protection。",
    );
    workspace.rescan_root(root_id).expect("内容变化后重建索引");

    assert_names(&workspace, "idempotency", &[]);
    assert_names(&workspace, "replay", &["api-review"]);
}

#[test]
fn personal_user_combines_client_root_repair_and_duplicate_check_filters() {
    let sandbox = tempdir().expect("创建临时工作区");
    let codex_root = sandbox.path().join(".codex/skills");
    let claude_root = sandbox.path().join(".claude/skills");
    write_skill(
        &codex_root.join("healthy-codex"),
        "healthy-codex",
        "可正常使用。",
        "Codex 内容。",
    );
    write_repairable_skill(&codex_root.join("broken-codex"));
    write_repairable_skill(&claude_root.join("broken-claude"));
    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    let codex_root_id = workspace
        .add_root(&codex_root)
        .expect("添加 Codex 根目录")
        .roots[0]
        .id;
    workspace
        .add_root(&claude_root)
        .expect("添加 Claude 根目录");

    let result = workspace
        .search_skills(&SkillQuery {
            filters: SkillFilters {
                clients: vec![SkillClient::Codex],
                root_ids: vec![codex_root_id],
                repair_status: SkillRepairFilter::NeedsRepair,
                duplicate_check_statuses: vec![DuplicateCheckStatus::None],
            },
            ..SkillQuery::default()
        })
        .expect("组合筛选本地 Skill");

    assert_eq!(result.total, 1);
    assert_eq!(result.instances[0].name, "broken-codex");

    let no_exact_duplicates = workspace
        .search_skills(&SkillQuery {
            filters: SkillFilters {
                duplicate_check_statuses: vec![DuplicateCheckStatus::Exact],
                ..SkillFilters::default()
            },
            ..SkillQuery::default()
        })
        .expect("筛选完全重复项");
    assert!(no_exact_duplicates.instances.is_empty());
}

#[test]
fn common_client_root_patterns_are_classified_for_filtering() {
    let sandbox = tempdir().expect("创建临时工作区");
    let opencode_root = sandbox.path().join(".config/opencode/skills");
    let hermes_root = sandbox.path().join(".hermes/skills");
    write_skill(
        &opencode_root.join("opencode-skill"),
        "opencode-skill",
        "OpenCode Skill。",
        "正文。",
    );
    write_skill(
        &hermes_root.join("hermes-skill"),
        "hermes-skill",
        "Hermes Skill。",
        "正文。",
    );
    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    workspace
        .add_root(&opencode_root)
        .expect("添加 OpenCode 根目录");
    workspace
        .add_root(&hermes_root)
        .expect("添加 Hermes 根目录");

    assert_client_names(&workspace, SkillClient::OpenCode, &["opencode-skill"]);
    assert_client_names(&workspace, SkillClient::Hermes, &["hermes-skill"]);
}

#[test]
fn personal_user_sorts_by_name_modified_time_and_root_path() {
    let sandbox = tempdir().expect("创建临时工作区");
    let later_root = sandbox.path().join("z-root");
    let earlier_root = sandbox.path().join("a-root");
    write_skill(
        &later_root.join("alpha"),
        "alpha",
        "Alpha Skill。",
        "内容。",
    );
    write_skill(&earlier_root.join("zeta"), "zeta", "Zeta Skill。", "内容。");
    set_modified_time(
        &later_root.join("alpha/SKILL.md"),
        UNIX_EPOCH + Duration::from_secs(1_700_000_000),
    );
    set_modified_time(
        &earlier_root.join("zeta/SKILL.md"),
        UNIX_EPOCH + Duration::from_secs(1_800_000_000),
    );
    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    workspace.add_root(&later_root).expect("添加较后根目录");
    workspace.add_root(&earlier_root).expect("添加较前根目录");

    assert_sorted_names(
        &workspace,
        SkillSortField::Name,
        SkillSortDirection::Asc,
        &["alpha", "zeta"],
    );
    assert_sorted_names(
        &workspace,
        SkillSortField::ModifiedAt,
        SkillSortDirection::Desc,
        &["zeta", "alpha"],
    );
    assert_sorted_names(
        &workspace,
        SkillSortField::Root,
        SkillSortDirection::Asc,
        &["zeta", "alpha"],
    );
}

#[test]
fn personal_user_sorts_by_created_time_and_filters_duplicate_check_results() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    for (index, name) in ["zeta", "alpha", "beta", "omega"].into_iter().enumerate() {
        write_skill(&root.join(name), name, "排序夹具。", "正文。");
        if index < 3 {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描排序夹具");
    let id_for = |name: &str| {
        snapshot
            .instances
            .iter()
            .find(|skill| skill.name == name)
            .expect("找到 Skill 实例")
            .id
            .clone()
    };
    workspace
        .save_duplicate_check_statuses(&[
            DuplicateCheckStatusUpdate {
                instance_id: id_for("alpha"),
                status: DuplicateCheckStatus::Exact,
            },
            DuplicateCheckStatusUpdate {
                instance_id: id_for("zeta"),
                status: DuplicateCheckStatus::Suspected,
            },
            DuplicateCheckStatusUpdate {
                instance_id: id_for("beta"),
                status: DuplicateCheckStatus::NameConflict,
            },
        ])
        .expect("保存重复检查状态");

    assert_sorted_names(
        &workspace,
        SkillSortField::CreatedAt,
        SkillSortDirection::Asc,
        &["zeta", "alpha", "beta", "omega"],
    );
    assert_sorted_names(
        &workspace,
        SkillSortField::DuplicateCheckStatus,
        SkillSortDirection::Asc,
        &["alpha", "zeta", "beta", "omega"],
    );
    let suspected = workspace
        .search_skills(&SkillQuery {
            filters: SkillFilters {
                duplicate_check_statuses: vec![DuplicateCheckStatus::Suspected],
                ..SkillFilters::default()
            },
            ..SkillQuery::default()
        })
        .expect("筛选疑似重复实例");
    assert_eq!(suspected.instances.len(), 1);
    assert_eq!(suspected.instances[0].name, "zeta");
}

#[test]
fn personal_user_keeps_view_preferences_after_restart_and_index_rebuild() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join(".codex/skills");
    let database_path = sandbox.path().join("index.sqlite3");
    write_skill(&root.join("api"), "api", "接口设计。", "正文。");
    let workspace = SkillWorkspace::open(&database_path).expect("打开 SkillWorkspace");
    let root_id = workspace.add_root(&root).expect("添加根目录").roots[0].id;
    let preferences = SkillWorkspaceViewPreferences {
        filters: SkillFilters {
            clients: vec![SkillClient::Codex],
            root_ids: vec![root_id],
            repair_status: SkillRepairFilter::Ready,
            duplicate_check_statuses: vec![DuplicateCheckStatus::None],
        },
        sort: SkillSort {
            field: SkillSortField::ModifiedAt,
            direction: SkillSortDirection::Desc,
        },
        density: SkillListDensity::Comfortable,
    };
    workspace
        .save_view_preferences(&preferences)
        .expect("保存视图偏好");
    drop(workspace);

    let reopened = SkillWorkspace::open(&database_path).expect("重启后打开 SkillWorkspace");
    assert_eq!(
        reopened.load_view_preferences().expect("读取偏好"),
        preferences
    );
    reopened.rescan_all_roots().expect("重建全文索引");
    assert_eq!(
        reopened.load_view_preferences().expect("再次读取偏好"),
        preferences
    );
}

#[test]
fn thousand_instance_catalog_keeps_common_search_and_filter_within_baseline() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join(".codex/skills");
    for index in 0..1_000 {
        let marker = if index % 10 == 0 {
            "needle"
        } else {
            "ordinary"
        };
        write_skill(
            &root.join(format!("skill-{index:04}")),
            &format!("skill-{index:04}"),
            &format!("{marker} 本地 Skill 描述。"),
            "用于性能基线的稳定正文。",
        );
    }
    let workspace =
        SkillWorkspace::open(sandbox.path().join("index.sqlite3")).expect("打开 SkillWorkspace");
    workspace.add_root(&root).expect("扫描一千个 Skill");
    let query = SkillQuery {
        text: "needle".to_owned(),
        filters: SkillFilters {
            clients: vec![SkillClient::Codex],
            repair_status: SkillRepairFilter::Ready,
            ..SkillFilters::default()
        },
        sort: SkillSort {
            field: SkillSortField::ModifiedAt,
            direction: SkillSortDirection::Desc,
        },
    };
    assert_eq!(
        workspace.search_skills(&query).expect("预热查询").total,
        100
    );

    let started = Instant::now();
    for _ in 0..30 {
        assert_eq!(
            workspace.search_skills(&query).expect("重复基线查询").total,
            100
        );
    }
    let elapsed = started.elapsed();
    eprintln!(
        "1000 个实例，30 次检索/筛选总耗时 {:?}，平均 {:?}",
        elapsed,
        elapsed / 30
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "30 次常用检索应在 3 秒内完成，实际为 {elapsed:?}"
    );
}

#[test]
fn upgraded_legacy_catalog_rebuilds_search_after_instance_ids_are_migrated() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("legacy-skills");
    let database_path = sandbox.path().join("legacy.sqlite3");
    fs::create_dir_all(&root).expect("创建旧根目录");
    let connection = rusqlite::Connection::open(&database_path).expect("创建旧版索引");
    connection
        .execute_batch(
            "
            CREATE TABLE workspace_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE skill_instances (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                skill_file_path TEXT NOT NULL,
                status TEXT NOT NULL,
                error TEXT
            );
            INSERT INTO skill_instances (
                id, name, description, relative_path, skill_file_path, status, error
            ) VALUES (
                'legacy-skill', 'legacy-skill', '旧版索引中的 Skill',
                'legacy-skill', '/tmp/legacy-skill/SKILL.md', 'ready', NULL
            );
            ",
        )
        .expect("写入旧版索引");
    connection
        .execute(
            "INSERT INTO workspace_settings (key, value) VALUES ('authorized_root', ?1)",
            [root.to_string_lossy().as_ref()],
        )
        .expect("写入旧版根目录设置");
    drop(connection);

    let upgraded = SkillWorkspace::open(&database_path).expect("升级旧版 SkillWorkspace");
    assert_names(&upgraded, "legacy-skill", &["legacy-skill"]);
}

#[test]
fn organization_terms_participate_in_search_after_restart_and_rescan() {
    let sandbox = tempdir().expect("创建临时工作区");
    let root = sandbox.path().join("skills");
    let database_path = sandbox.path().join("index.sqlite3");
    write_skill(
        &root.join("api-review"),
        "api-review",
        "接口审查。",
        "正文。",
    );
    let workspace = SkillWorkspace::open(&database_path).expect("打开 SkillWorkspace");
    let snapshot = workspace.add_root(&root).expect("扫描根目录");
    workspace
        .save_organization_search_terms(&[SkillOrganizationSearchTermsUpdate {
            instance_id: snapshot.instances[0].id.clone(),
            tags: vec!["安全审计".to_owned(), "API".to_owned()],
            skill_groups: vec!["支付项目".to_owned()],
        }])
        .expect("保存组织检索词");

    assert_names(&workspace, "安全审计", &["api-review"]);
    assert_names(&workspace, "支付项目", &["api-review"]);
    drop(workspace);

    let reopened = SkillWorkspace::open(&database_path).expect("重新打开 SkillWorkspace");
    assert_names(&reopened, "安全审计", &["api-review"]);
    reopened.rescan_all_roots().expect("重新扫描并重建索引");
    assert_names(&reopened, "支付项目", &["api-review"]);
}

fn assert_names(workspace: &SkillWorkspace, text: &str, expected: &[&str]) {
    let result = workspace
        .search_skills(&SkillQuery {
            text: text.to_owned(),
            ..SkillQuery::default()
        })
        .expect("全文检索本地 Skill");
    let names = result
        .instances
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, expected);
}

fn assert_sorted_names(
    workspace: &SkillWorkspace,
    field: SkillSortField,
    direction: SkillSortDirection,
    expected: &[&str],
) {
    let result = workspace
        .search_skills(&SkillQuery {
            sort: SkillSort { field, direction },
            ..SkillQuery::default()
        })
        .expect("排序本地 Skill");
    let names = result
        .instances
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, expected);
}

fn assert_client_names(workspace: &SkillWorkspace, client: SkillClient, expected: &[&str]) {
    let result = workspace
        .search_skills(&SkillQuery {
            filters: SkillFilters {
                clients: vec![client],
                ..SkillFilters::default()
            },
            ..SkillQuery::default()
        })
        .expect("按客户端筛选 Skill");
    let names = result
        .instances
        .iter()
        .map(|skill| skill.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, expected);
}

fn write_skill(directory: &std::path::Path, name: &str, description: &str, body: &str) {
    fs::create_dir_all(directory).expect("创建 Skill 目录");
    fs::write(
        directory.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n\n{body}\n"),
    )
    .expect("写入 SKILL.md");
}

fn write_repairable_skill(directory: &std::path::Path) {
    fs::create_dir_all(directory).expect("创建需要修复的 Skill 目录");
    fs::write(directory.join("SKILL.md"), "# 缺少 frontmatter\n").expect("写入需要修复的 SKILL.md");
}

fn set_modified_time(path: &std::path::Path, modified: std::time::SystemTime) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .expect("打开 SKILL.md")
        .set_times(FileTimes::new().set_modified(modified))
        .expect("设置修改时间");
}
