# macOS 首版验收矩阵

| 验收线 | 证据 | 结果 |
| --- | --- | --- |
| 中文默认界面、错误与反馈；键盘焦点可见；控件名称可理解 | `src/*.test.tsx`；全局 `:focus-visible`；`html lang="zh-CN"` | 通过 |
| 1000 个 Skill 的检索、筛选、列表选择与重复检查结果 | `search_catalog`、`duplicate_review`、`App.test.tsx` 千实例基线 | 通过 |
| 授权根目录、正常与需要修复的 Skill 实例、检索、编辑、Skill 组、重复检查、归并、撤销 | `npm run smoke:macos` 经过真实 WebView、Tauri IPC 与真实临时文件；核心回归见 `v1_acceptance.rs` | 通过 |
| 无账号与外部服务；索引、备份和记录只在本地 | `npm run check:local-only`；[本地离线架构](local-only-architecture.md) | 通过 |
| 一条命令生成可启动 `.app`，CI 覆盖生产构建和静态检查 | `npm run build:app`；`.github/workflows/ci.yml` | 通过 |
| 原型结论进入生产应用，原型目录、mock 数据和切换入口退场 | `prototype/skill-manager-ui` 已删除；本地离线门禁持续检查 | 通过 |

## 桌面发布检查

```bash
npm ci
npm run check
npm run smoke:macos
npm run build:app
open "target/release/bundle/macos/Skill 管理器.app"
```

首版验收不包含 Apple 签名、公证、Skill Market、云同步、团队协作或其他平台安装包。
