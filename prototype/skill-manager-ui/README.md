# Skill Management UI Prototype

> THROWAWAY PROTOTYPE — 用来回答信息架构与交互问题，不是生产代码。

三套 Skill 管理界面方案位于同一路由，通过 `?variant=A|B|C` 切换：

- A：日常管理工作台
- B：重复治理中心
- C：分组收藏工作区

从仓库根目录运行：

```sh
npm --prefix prototype/skill-manager-ui run prototype
```

然后打开 `http://localhost:4173/?variant=A`。

所有数据与修改都仅存在于浏览器内存中。
