use std::io;

use skill_workspace::WorkspaceError;

#[test]
fn user_facing_errors_never_expose_platform_english_diagnostics() {
    let error = WorkspaceError::from(io::Error::new(
        io::ErrorKind::NotFound,
        "No such file or directory (os error 2)",
    ));

    assert_eq!(
        error.to_string(),
        "读取本地文件失败，请检查路径、权限或磁盘状态。"
    );
    assert_eq!(error.user_message(), error.to_string());
    assert!(!error.to_string().contains("No such file"));
}
