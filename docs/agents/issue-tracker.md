# Issue tracker: GitHub

本仓库的 PRD 与任务使用 GitHub Issues 管理，目标仓库为 `hakityc/skill-management`。所有操作使用 `gh` CLI。

## Conventions

- 创建：`gh issue create --title "..." --body-file <file>`
- 读取：`gh issue view <number> --comments`
- 列表：`gh issue list --state open --json number,title,body,labels,comments`
- 评论：`gh issue comment <number> --body "..."`
- 添加或移除标签：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- 关闭：`gh issue close <number> --comment "..."`

在仓库目录内执行时，从 `origin` 自动确定目标仓库。

## Pull requests as a triage surface

**PRs as a request surface: no.**

外部 PR 不进入 `/triage` 的需求队列。PR 只表示代码协作，不自动等同于功能请求。

## Skill conventions

- “发布到任务系统”表示创建 GitHub Issue。
- “读取相关任务”表示执行 `gh issue view <number> --comments` 并读取标签。
- `/to-prd` 创建父 PRD Issue；`/to-issues` 创建可独立领取的子任务 Issues。
