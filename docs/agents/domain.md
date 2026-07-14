# Domain Docs

本仓库采用单一领域上下文。

## Before exploring

- 读取仓库根目录的 `CONTEXT.md`，使用其中定义的统一领域语言。
- 读取 `docs/adr/` 中与当前工作相关的架构决策。
- 文件不存在时静默继续；仅在真实术语或决策被解决时通过 `/domain-modeling` 延迟创建。

## Layout

```text
/
├── CONTEXT.md
├── docs/
│   └── adr/
└── src/
```

## Consumer rules

- Issue 标题、PRD、测试名称与实现说明必须采用 `CONTEXT.md` 中的标准术语。
- 不使用 glossary 明确列入 `_Avoid_` 的同义词。
- 如果需要的领域概念尚未定义，先确认它是真实缺口，再运行 `/domain-modeling`。
- 如果工作与既有 ADR 冲突，必须明确指出冲突，不得静默覆盖。
