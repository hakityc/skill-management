# 本地离线架构

Skill 管理器首版没有账号系统，也不连接 Skill Market、云同步或任何外部服务。运行时数据流只有：

1. React 界面通过 Tauri IPC 调用同进程 Rust 命令；
2. Rust 核心读取个人用户明确授权的 Skill 根目录；
3. 索引、视图偏好、Skill 组、Skill 标签、操作记录与撤销信息写入 macOS 应用数据目录；
4. 编辑和安全文件操作只写入已授权根目录，删除使用 macOS 系统废纸篓。

## 本地数据位置

- `skill-management.sqlite3`：位于系统为 `com.hakityc.skill-management` 分配的应用数据目录；
- `backups/`：编辑操作的本地撤销备份，与数据库同级；
- `file-operation-backups/`：复制、移动、覆盖和归并的本地撤销备份，与数据库同级；
- `file-operation-staging/`：ZIP 导入的短期本地暂存目录，与数据库同级；
- Skill 真实文件：只位于个人用户授权的根目录和 macOS 系统废纸篓。

应用不会上传这些内容。备份与操作记录是实现撤销、崩溃恢复和结果核对所必需的本地数据。

## 可重复检查

```bash
npm run check:local-only
```

该检查会阻止以下内容进入生产代码：浏览器网络 API、Rust 网络客户端、未批准的运行时依赖、远程 Tauri capability、开放式内容安全策略、生产 mock/原型开关，以及已退场的原型目录。

开发模式只允许 Vite 使用 `127.0.0.1:1420`；打包应用的内容安全策略只允许本地资源和 Tauri IPC。依赖安装与 CI 拉取工具属于构建活动，不属于应用运行时网络能力。
