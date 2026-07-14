# Skill 管理器

一个中文优先、只管理本机文件的 macOS 桌面应用。首版覆盖 Skill 根目录管理、检索筛选排序、查看编辑、Skill 组与 Skill 标签、安全文件操作、重复检查、归并与撤销。

首版不包含 Skill Market、账号、云同步、团队协作或 AI 自动文本融合。

## 构建 macOS 应用

需要 macOS、Xcode Command Line Tools、Node.js 22 和 Rust stable。

```bash
npm ci
npm run build:app
```

可启动应用生成在：

```text
target/release/bundle/macos/Skill 管理器.app
```

当前首版没有 Apple 开发者签名或公证，仅用于本机构建与验收。

## 本地开发与检查

```bash
npm run tauri dev
npm run check
```

`npm run check` 会依次检查本地离线边界、TypeScript、前端测试、Rust 测试、Clippy 与生产前端构建。

macOS 桌面全链路冒烟使用隔离临时数据：

```bash
npm run smoke:macos
```

更多证据：

- [首版验收矩阵](docs/v1-acceptance.md)
- [千实例性能基线](docs/performance-baseline.md)
- [macOS 桌面冒烟](docs/macos-desktop-smoke.md)
- [本地离线架构](docs/local-only-architecture.md)
- [本地文件操作安全边界](docs/security-boundaries.md)
