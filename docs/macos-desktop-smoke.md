# macOS 桌面冒烟

运行以下一条命令：

```bash
npm run smoke:macos
```

该命令会创建隔离的临时 HOME、应用数据库和 Skill 根目录，启动真实 macOS WebView，并通过生产 React 界面、gateway、Tauri IPC 和 Rust 本地核心完成：

1. 授权临时 Skill 根目录；
2. 确认正常与需要修复的 Skill 实例都可见；
3. 检索并通过编辑器保存真实 `SKILL.md`；
4. 创建 Skill 组，并把两个 Skill 实例批量加入该组；
5. 打开重复检查，选择主实例和归并目标；
6. 预览、执行并撤销归并；
7. 退出 WebView 后重新打开本地索引，核对归并记录已经撤销，主实例保留编辑，目标目录和额外文件逐字节恢复。

桌面驱动仅在 Rust `desktop-smoke` 测试 feature 中编译。默认 `npm run build:app` 不包含驱动脚本、测试入口或临时数据。测试结束后会删除全部隔离数据；不读取或修改个人用户现有的应用数据和 Skill 根目录。
