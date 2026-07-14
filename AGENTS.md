# Repository instructions

开发项目时优先实现项目 UI 语言为中文，除非用户明确要求 i18n 才考虑其他语言。

## Agent skills

### Issue tracker

PRD 与任务发布到 `hakityc/skill-management` 的 GitHub Issues；外部 PR 不作为需求入口。参见 `docs/agents/issue-tracker.md`。

### Triage labels

使用标准五类标签：`needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix`。参见 `docs/agents/triage-labels.md`。

### Domain docs

采用单一领域上下文：仓库根目录的 `CONTEXT.md` 与 `docs/adr/`。参见 `docs/agents/domain.md`。
