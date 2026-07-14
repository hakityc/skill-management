use std::{
    env, fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use skill_workspace::{FileOperationKind, SkillStatus, SkillWorkspace};
use tauri::AppHandle;

const COMPLETION_GROUP: &str = "桌面验收完成";

pub fn database_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = PathBuf::from(env::var("SKILL_MANAGEMENT_SMOKE_DATABASE")?);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

pub fn script() -> String {
    let root = serde_json::to_string(
        &env::var("SKILL_MANAGEMENT_SMOKE_ROOT").expect("缺少桌面冒烟根目录"),
    )
    .expect("序列化桌面冒烟根目录");
    format!(
        ";\nwindow.__SKILL_MANAGEMENT_SMOKE_ROOT__ = {root};\n{}",
        include_str!("desktop_smoke.js")
    )
}

pub fn monitor(app: AppHandle, workspace: SkillWorkspace) {
    thread::spawn(move || {
        for _ in 0..300 {
            if let Ok(organization) = workspace.skill_organization() {
                if organization
                    .groups
                    .iter()
                    .any(|group| group.name == COMPLETION_GROUP)
                {
                    match verify_workspace(&workspace) {
                        Ok(()) => {
                            eprintln!(
                                "macOS 桌面冒烟通过：React、Tauri IPC、本地核心和真实文件恢复均已验证。"
                            );
                            finish(&app, Ok(()));
                        }
                        Err(error) => {
                            eprintln!("macOS 桌面冒烟失败：{error}");
                            finish(&app, Err(error));
                        }
                    }
                    return;
                }
                if organization
                    .groups
                    .iter()
                    .any(|group| group.name == "桌面验收失败")
                {
                    let stages = organization
                        .groups
                        .iter()
                        .map(|group| group.name.as_str())
                        .collect::<Vec<_>>()
                        .join("、");
                    let error = format!("WebView 驱动未完成界面流程；阶段标记：{stages}");
                    eprintln!("macOS 桌面冒烟失败：{error}");
                    finish(&app, Err(error));
                    return;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        let groups = workspace
            .skill_organization()
            .map(|organization| {
                organization
                    .groups
                    .into_iter()
                    .map(|group| group.name)
                    .collect::<Vec<_>>()
                    .join("、")
            })
            .unwrap_or_else(|_| "无法读取阶段标记".to_owned());
        let error = format!("等待 WebView 流程超时；阶段标记：{groups}");
        eprintln!("macOS 桌面冒烟失败：{error}");
        finish(&app, Err(error));
    });
}

fn finish(app: &AppHandle, result: Result<(), String>) {
    let (content, exit_code) = match result {
        Ok(()) => ("ok".to_owned(), 0),
        Err(error) => (error, 1),
    };
    if let Ok(path) = env::var("SKILL_MANAGEMENT_SMOKE_RESULT") {
        let _ = fs::write(path, content);
    }
    app.exit(exit_code);
}

fn verify_workspace(workspace: &SkillWorkspace) -> Result<(), String> {
    let root = PathBuf::from(
        env::var("SKILL_MANAGEMENT_SMOKE_ROOT").map_err(|_| "缺少桌面冒烟根目录。".to_owned())?,
    );
    let snapshot = workspace.snapshot().map_err(|error| error.user_message())?;
    if !snapshot
        .instances
        .iter()
        .any(|instance| instance.status == SkillStatus::NeedsRepair)
    {
        return Err("桌面冒烟未发现需要修复的 Skill 实例。".to_owned());
    }
    let organization = workspace
        .skill_organization()
        .map_err(|error| error.user_message())?;
    let acceptance_group = organization
        .groups
        .iter()
        .find(|group| group.name == "桌面验收")
        .ok_or_else(|| "桌面冒烟未创建 Skill 组。".to_owned())?;
    if acceptance_group.instance_ids.len() != 2 {
        return Err("桌面冒烟未把两个 Skill 实例加入 Skill 组。".to_owned());
    }
    let source = read(root.join("release-main/SKILL.md"))?;
    if !source.contains("desktop-smoke-edited") {
        return Err("桌面冒烟未通过编辑器保存主实例。".to_owned());
    }
    let target = read(root.join("release-copy/SKILL.md"))?;
    if !target.contains("desktop-smoke-target") || !root.join("release-copy/legacy.txt").is_file() {
        return Err("桌面冒烟撤销归并后没有恢复目标真实文件。".to_owned());
    }
    let merge = workspace
        .file_operation_history()
        .map_err(|error| error.user_message())?
        .into_iter()
        .find(|record| record.kind == FileOperationKind::Merge)
        .ok_or_else(|| "桌面冒烟没有生成归并记录。".to_owned())?;
    if !merge.undone {
        return Err("桌面冒烟没有撤销归并。".to_owned());
    }
    Ok(())
}

fn read(path: impl AsRef<Path>) -> Result<String, String> {
    fs::read_to_string(path).map_err(|_| "无法读取桌面冒烟真实文件。".to_owned())
}
