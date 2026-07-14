/*
 * THROWAWAY UI PROTOTYPE
 * Three variants of the local Skill management surface, switchable via ?variant=.
 * All state is in memory and every mutation is simulated.
 */

const CLIENT_META = {
  Claude: { short: "Cl", label: "Claude", tone: "coral" },
  Codex: { short: "Cx", label: "Codex", tone: "mint" },
  Gemini: { short: "Ge", label: "Gemini", tone: "blue" },
  OpenCode: { short: "Op", label: "OpenCode", tone: "violet" },
  Hermes: { short: "He", label: "Hermes", tone: "sand" },
};

const CATALOG = [
  ["access-control-rbac", "构建基于角色的访问控制、权限矩阵与授权策略。", "安全与权限", ["安全", "架构"]],
  ["agents-sdk", "构建有状态 AI Agent、实时工作流与 Durable Object 服务。", "Agent 工程", ["Agent", "SDK"]],
  ["aihot", "查询中文 AI 资讯、日报、热点与模型动态。", "内容与研究", ["资讯", "中文"]],
  ["api-and-interface-design", "设计稳定 API、模块边界和公共接口契约。", "工程设计", ["API", "架构"]],
  ["apikey-image-gen", "通过已配置的图像模型生成或编辑图片。", "视觉创作", ["图像", "生成"]],
  ["arkcli-api-explorer", "探索 Ark CLI 原始 API 与底层 Action 契约。", "命令行工具", ["CLI", "API"]],
  ["arkcli-auth", "处理交互式登录、SSO、凭据状态与退出。", "安全与权限", ["认证", "CLI"]],
  ["arkcli-billing", "查看用量账单、配额与消费趋势。", "命令行工具", ["账单", "CLI"]],
  ["browser-testing", "使用浏览器开发者工具验证页面行为与视觉状态。", "质量保障", ["浏览器", "测试"]],
  ["ci-cd-automation", "设计持续集成、发布流水线与自动化检查。", "发布与运维", ["CI", "自动化"]],
  ["cloudflare", "构建和部署 Cloudflare Workers 与边缘应用。", "发布与运维", ["边缘", "部署"]],
  ["code-review", "从规范和需求两个维度审查代码差异。", "质量保障", ["审查", "质量"]],
  ["code-simplification", "降低代码复杂度并改善模块局部性。", "工程设计", ["重构", "质量"]],
  ["copywriting", "为产品页面、消息和发布材料撰写清晰文案。", "内容与研究", ["文案", "产品"]],
  ["debugging", "建立紧密反馈回路，定位并修复复杂缺陷。", "质量保障", ["调试", "测试"]],
  ["domain-modeling", "建立领域语言、澄清模糊术语并记录关键决策。", "工程设计", ["DDD", "架构"]],
  ["frontend-design", "构建有辨识度、信息层级清晰的前端界面。", "视觉创作", ["前端", "设计"]],
  ["git-workflow", "维护分支、提交、版本与协作工作流。", "发布与运维", ["Git", "协作"]],
  ["handoff", "把当前上下文压缩成可由新任务继续的交接文档。", "Agent 工程", ["上下文", "协作"]],
  ["imagegen", "根据描述生成图片，或对现有图片进行定向编辑。", "视觉创作", ["图像", "创作"]],
  ["implement", "依据需求按测试驱动切片完成实现。", "Agent 工程", ["实现", "TDD"]],
  ["observability", "设计日志、指标、追踪与运行时诊断能力。", "发布与运维", ["监控", "日志"]],
  ["pdf", "读取、生成与检查 PDF 文档。", "内容与研究", ["PDF", "文档"]],
  ["playwright", "编写可靠的浏览器自动化与端到端测试。", "质量保障", ["E2E", "浏览器"]],
  ["prototype", "用可丢弃程序验证难以在纸面确定的设计问题。", "Agent 工程", ["原型", "验证"]],
  ["security-hardening", "审计攻击面并强化应用与基础设施安全。", "安全与权限", ["安全", "审计"]],
  ["skill-creator", "设计、编写并校验结构清晰的 Agent Skill。", "Agent 工程", ["Skill", "创作"]],
  ["tdd", "用红绿重构循环实现可观察的具体行为。", "质量保障", ["测试", "实现"]],
];

const ROOTS = ["~/.agents/skills", "~/.codex/skills", "~/.claude/skills", "~/Library/Application Support/Hermes/skills"];
const CLIENT_PATTERNS = [
  ["Claude", "Codex"],
  ["Codex"],
  ["Gemini", "OpenCode"],
  ["Claude", "Codex", "Gemini"],
  ["Hermes"],
];

const skills = CATALOG.map(([name, description, group, tags], index) => ({
  id: `${name}-${index}`,
  name,
  description,
  group,
  tags,
  root: ROOTS[index % ROOTS.length],
  path: `${ROOTS[index % ROOTS.length]}/${name}`,
  clients: CLIENT_PATTERNS[index % CLIENT_PATTERNS.length],
  duplicate: "none",
  status: index === 8 ? "repair" : index === 21 ? "symlink" : "ok",
  updatedMinutes: 18 + index * 37,
  files: 2 + (index % 8),
  lines: 68 + index * 19,
}));

function addInstance(name, root, duplicate, clients, descriptionSuffix = "") {
  const original = skills.find((skill) => skill.name === name);
  skills.push({
    ...original,
    id: `${name}-${skills.length}`,
    root,
    path: `${root}/${name}`,
    clients,
    duplicate,
    description: `${original.description}${descriptionSuffix}`,
    updatedMinutes: original.updatedMinutes + 73,
  });
  original.duplicate = duplicate;
}

addInstance("access-control-rbac", "~/.codex/skills", "exact", ["Codex"]);
// Add a standalone fixture that is not part of the base catalog.
skills.push({
  id: "find-bugs-base",
  name: "find-bugs",
  description: "扫描代码差异并定位高置信度的行为缺陷。",
  group: "质量保障",
  tags: ["审查", "缺陷"],
  root: "~/.agents/skills",
  path: "~/.agents/skills/find-bugs",
  clients: ["Claude"],
  duplicate: "exact",
  status: "ok",
  updatedMinutes: 36,
  files: 4,
  lines: 188,
});
addInstance("find-bugs", "~/.codex/skills", "exact", ["Codex"]);
addInstance("frontend-design", "~/.codex/skills", "suspect", ["Codex"], " 包含额外的中文界面检查清单。");
addInstance("copywriting", "~/.claude/skills", "suspect", ["Claude"], " 针对中文产品增加语气模板。");
addInstance("skill-creator", "~/.codex/skills", "conflict", ["Codex"], " 使用另一套目录结构与校验规则。");
addInstance("tdd", "~/.claude/skills", "conflict", ["Claude"], " 面向后端服务的测试循环。");

const variantNames = {
  A: "日常管理工作台",
  B: "重复治理中心",
  C: "分组收藏工作区",
};

const state = {
  variant: new URLSearchParams(location.search).get("variant") || "A",
  query: "",
  filter: "all",
  sort: "name",
  selectedSkillId: skills[0].id,
  selectedDuplicateName: "frontend-design",
  selectedGroup: "全部分组",
  selectedIds: new Set(),
  detailTab: "概览",
  modal: null,
  lastAction: "等待操作",
};

if (!variantNames[state.variant]) state.variant = "A";

const app = document.querySelector("#app");
const announcer = document.querySelector("#announcer");

function escapeHtml(value = "") {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

function statusMeta(skill) {
  if (skill.status === "repair") return { label: "需要修复", className: "danger" };
  if (skill.status === "symlink") return { label: "符号链接", className: "neutral" };
  if (skill.duplicate === "exact") return { label: "完全重复", className: "success" };
  if (skill.duplicate === "suspect") return { label: "疑似重复", className: "warning" };
  if (skill.duplicate === "conflict") return { label: "同名冲突", className: "danger" };
  return { label: "正常", className: "quiet" };
}

function duplicateLabel(value) {
  return { exact: "完全重复", suspect: "疑似重复", conflict: "同名冲突", none: "无重复" }[value];
}

function clientChips(clients, compact = false) {
  return `<div class="client-list">${clients
    .map((client) => {
      const meta = CLIENT_META[client];
      return `<span class="client-chip ${meta.tone}" title="${meta.label}">${compact ? meta.short : meta.label}</span>`;
    })
    .join("")}</div>`;
}

function badge(label, className = "neutral") {
  return `<span class="badge ${className}">${escapeHtml(label)}</span>`;
}

function getFilteredSkills() {
  const query = state.query.trim().toLowerCase();
  let result = skills.filter((skill) => {
    const haystack = [skill.name, skill.description, skill.group, skill.root, skill.path, ...skill.tags].join(" ").toLowerCase();
    const matchesQuery = !query || haystack.includes(query);
    const matchesFilter =
      state.filter === "all" ||
      (state.filter === "duplicates" && skill.duplicate !== "none") ||
      (state.filter === "repair" && skill.status === "repair") ||
      skill.clients.includes(state.filter) ||
      skill.root === state.filter;
    const matchesGroup = state.selectedGroup === "全部分组" || skill.group === state.selectedGroup;
    return matchesQuery && matchesFilter && matchesGroup;
  });

  result = [...result].sort((a, b) => {
    if (state.sort === "updated") return a.updatedMinutes - b.updatedMinutes;
    if (state.sort === "duplicate") return a.duplicate.localeCompare(b.duplicate) || a.name.localeCompare(b.name);
    if (state.sort === "root") return a.root.localeCompare(b.root) || a.name.localeCompare(b.name);
    return a.name.localeCompare(b.name);
  });
  return result;
}

function duplicateGroups() {
  const grouped = new Map();
  skills
    .filter((skill) => skill.duplicate !== "none")
    .forEach((skill) => {
      if (!grouped.has(skill.name)) grouped.set(skill.name, []);
      grouped.get(skill.name).push(skill);
    });
  return [...grouped.entries()]
    .map(([name, instances]) => ({
      name,
      instances,
      status: instances[0].duplicate,
      confidence: instances[0].duplicate === "exact" ? 100 : instances[0].duplicate === "suspect" ? 82 : 46,
    }))
    .filter((group) => !state.query || [group.name, ...group.instances.map((item) => item.description)].join(" ").toLowerCase().includes(state.query.toLowerCase()));
}

function selectedSkill() {
  return skills.find((skill) => skill.id === state.selectedSkillId) || skills[0];
}

function formatAge(minutes) {
  if (minutes < 60) return `${minutes} 分钟前`;
  if (minutes < 1440) return `${Math.floor(minutes / 60)} 小时前`;
  return `${Math.floor(minutes / 1440)} 天前`;
}

function renderSearch(placeholder = "搜索名称、描述、正文、路径或标签") {
  return `<label class="search-box">
    <span aria-hidden="true">⌕</span>
    <input data-search value="${escapeHtml(state.query)}" placeholder="${placeholder}" aria-label="搜索 Skill" />
    ${state.query ? '<button class="clear-search" data-action="clear-search" aria-label="清空搜索">×</button>' : '<kbd>⌘ K</kbd>'}
  </label>`;
}

function renderSelects() {
  return `<label class="select-control"><span>筛选</span><select data-filter-select>
      <option value="all" ${state.filter === "all" ? "selected" : ""}>全部状态</option>
      <option value="duplicates" ${state.filter === "duplicates" ? "selected" : ""}>存在重复</option>
      <option value="repair" ${state.filter === "repair" ? "selected" : ""}>需要修复</option>
      ${Object.keys(CLIENT_META).map((client) => `<option value="${client}" ${state.filter === client ? "selected" : ""}>${client}</option>`).join("")}
    </select></label>
    <label class="select-control"><span>排序</span><select data-sort-select>
      <option value="name" ${state.sort === "name" ? "selected" : ""}>名称</option>
      <option value="updated" ${state.sort === "updated" ? "selected" : ""}>最近修改</option>
      <option value="duplicate" ${state.sort === "duplicate" ? "selected" : ""}>重复状态</option>
      <option value="root" ${state.sort === "root" ? "selected" : ""}>根目录</option>
    </select></label>`;
}

function renderAppMark() {
  return `<div class="app-mark" aria-label="Skill 管理器"><span>SM</span><i></i></div>`;
}

function renderVariantA() {
  const rows = getFilteredSkills();
  const skill = selectedSkill();
  const groups = [...new Set(skills.map((item) => item.group))];
  return `<div class="prototype variant-a">
    <header class="topbar-a">
      <div class="brand-line">${renderAppMark()}<div><strong>Skill 管理器</strong><span>本地档案 · ${skills.length} 个实例</span></div></div>
      <div class="topbar-actions">
        <button class="button ghost" data-action="simulate" data-message="已打开根目录设置">管理根目录</button>
        <button class="button ghost" data-action="simulate" data-message="已打开 ZIP 导入预览">从 ZIP 导入</button>
        <button class="button primary" data-action="simulate" data-message="已创建一个未命名 Skill 草稿">＋ 新建 Skill</button>
      </div>
    </header>
    <div class="workbench-a">
      <aside class="sidebar-a">
        <div class="sidebar-section">
          <p class="eyebrow">资料库</p>
          ${navItem("全部 Skill", skills.length, state.filter === "all" && state.selectedGroup === "全部分组", "all")}
          ${navItem("最近修改", 12, false, "all")}
          ${navItem("需要修复", skills.filter((s) => s.status === "repair").length, state.filter === "repair", "repair", "danger-dot")}
          ${navItem("重复检查", skills.filter((s) => s.duplicate !== "none").length, state.filter === "duplicates", "duplicates", "warning-dot")}
        </div>
        <div class="sidebar-section">
          <div class="section-heading"><p class="eyebrow">客户端</p><button data-action="simulate" data-message="已打开客户端目录设置">＋</button></div>
          ${Object.entries(CLIENT_META).map(([client, meta]) => navItem(client, skills.filter((s) => s.clients.includes(client)).length, state.filter === client, client, `client-nav ${meta.tone}`)).join("")}
        </div>
        <div class="sidebar-section grow">
          <div class="section-heading"><p class="eyebrow">分组</p><button data-action="simulate" data-message="已创建新分组">＋</button></div>
          ${groups.slice(0, 6).map((group) => groupNavItem(group)).join("")}
        </div>
        <button class="root-summary" data-action="simulate" data-message="4 个根目录均可访问">
          <span class="root-glyph">⌘</span><span><b>4 个根目录</b><small>上次扫描：刚刚</small></span><i>›</i>
        </button>
      </aside>
      <main class="library-a">
        <div class="library-heading">
          <div><p class="eyebrow">本地资料库</p><h1>${state.selectedGroup === "全部分组" ? "全部 Skill" : escapeHtml(state.selectedGroup)}</h1></div>
          <div class="health-note"><i></i><span>目录状态正常</span><button data-action="simulate" data-message="扫描完成，没有发现新的目录变化">重新扫描</button></div>
        </div>
        <div class="toolbar-a">${renderSearch()}<div class="toolbar-right">${renderSelects()}<button class="icon-button" title="显示设置" data-action="simulate" data-message="已打开列表显示设置">☷</button></div></div>
        <div class="selection-strip ${state.selectedIds.size ? "visible" : ""}"><b>已选择 ${state.selectedIds.size} 项</b><button data-action="simulate" data-message="已加入分组">加入分组</button><button data-action="simulate" data-message="已打开移动预览">移动</button><button data-action="simulate" data-message="已重新检查所选 Skill">检查重复</button><button class="danger-text" data-action="simulate" data-message="删除预览已打开，尚未修改文件">移到废纸篓</button></div>
        <div class="skill-table-wrap">
          <table class="skill-table">
            <thead><tr><th class="check-col"><span class="fake-check"></span></th><th>Skill</th><th>分组</th><th>安装位置</th><th>状态</th><th>修改时间</th><th></th></tr></thead>
            <tbody>${rows.map((item) => renderSkillRow(item)).join("") || renderEmptyRow()}</tbody>
          </table>
        </div>
        <footer class="list-footer"><span>显示 ${rows.length} / ${skills.length} 个实例</span><span>扫描耗时 184 ms · 索引已更新</span></footer>
      </main>
      ${renderInspector(skill)}
    </div>
  </div>`;
}

function navItem(label, count, active, filter, extra = "") {
  return `<button class="nav-item ${active ? "active" : ""} ${extra}" data-filter="${escapeHtml(filter)}"><span>${escapeHtml(label)}</span><b>${count}</b></button>`;
}

function groupNavItem(group) {
  const active = state.selectedGroup === group;
  const count = skills.filter((skill) => skill.group === group).length;
  return `<button class="nav-item group-nav ${active ? "active" : ""}" data-group="${escapeHtml(group)}"><i></i><span>${escapeHtml(group)}</span><b>${count}</b></button>`;
}

function renderSkillRow(skill) {
  const meta = statusMeta(skill);
  const selected = skill.id === state.selectedSkillId;
  const checked = state.selectedIds.has(skill.id);
  return `<tr class="skill-row ${selected ? "selected" : ""}" data-select-skill="${skill.id}">
    <td class="check-col"><button class="row-check ${checked ? "checked" : ""}" data-toggle-select="${skill.id}" aria-label="选择 ${skill.name}">${checked ? "✓" : ""}</button></td>
    <td><div class="skill-identity spine-${meta.className}"><strong>${escapeHtml(skill.name)}</strong><span>${escapeHtml(skill.description)}</span></div></td>
    <td><span class="group-label">${escapeHtml(skill.group)}</span></td>
    <td>${clientChips(skill.clients, true)}</td>
    <td>${badge(meta.label, meta.className)}</td>
    <td><span class="muted">${formatAge(skill.updatedMinutes)}</span></td>
    <td><button class="more-button" data-action="simulate" data-message="已打开 ${escapeHtml(skill.name)} 的更多操作" aria-label="更多操作">•••</button></td>
  </tr>`;
}

function renderEmptyRow() {
  return `<tr><td colspan="7"><div class="empty-state"><b>没有匹配的 Skill</b><span>尝试清空检索词或调整筛选条件。</span><button data-action="clear-search">清空条件</button></div></td></tr>`;
}

function renderInspector(skill) {
  const meta = statusMeta(skill);
  const tabs = ["概览", "文件", "版本"];
  return `<aside class="inspector-a">
    <div class="inspector-head"><span class="path-label">${escapeHtml(skill.root)}</span><button class="icon-button" data-action="simulate" data-message="详情面板已关闭">×</button></div>
    <div class="skill-title-block spine-${meta.className}">
      <div class="skill-avatar">${skill.name.slice(0, 2).toUpperCase()}</div>
      <div><h2>${escapeHtml(skill.name)}</h2><p>${escapeHtml(skill.description)}</p></div>
    </div>
    <div class="inspector-status">${badge(meta.label, meta.className)}${clientChips(skill.clients)}</div>
    <div class="tab-row">${tabs.map((tab) => `<button class="${state.detailTab === tab ? "active" : ""}" data-detail-tab="${tab}">${tab}</button>`).join("")}</div>
    ${renderInspectorTab(skill)}
    <div class="inspector-actions"><button class="button secondary" data-action="simulate" data-message="已打开 ${escapeHtml(skill.name)} 编辑器">编辑内容</button><button class="button ghost" data-action="simulate" data-message="已在访达中定位 ${escapeHtml(skill.name)}">在访达中显示</button></div>
  </aside>`;
}

function renderInspectorTab(skill) {
  if (state.detailTab === "文件") {
    return `<div class="file-tree"><p class="eyebrow">${skill.files} 个文件</p>${["SKILL.md", "references/", "  examples.md", "assets/", "  cover.png"].slice(0, Math.min(skill.files, 5)).map((file, index) => `<button data-action="simulate" data-message="已选择 ${file.trim()}"><span>${file.endsWith("/") ? "⌄" : "·"}</span>${file}<small>${index === 0 ? `${skill.lines} 行` : ""}</small></button>`).join("")}</div>`;
  }
  if (state.detailTab === "版本") {
    return `<div class="timeline"><div><i></i><span><b>今天 ${String(10 + (skill.updatedMinutes % 10)).padStart(2, "0")}:24</b><small>修改 SKILL.md</small></span></div><div><i></i><span><b>昨天 18:06</b><small>从 ${escapeHtml(skill.root)} 导入</small></span></div></div>`;
  }
  return `<div class="overview-panel">
    ${skill.duplicate !== "none" ? `<button class="duplicate-callout ${skill.duplicate}" data-action="jump-duplicate"><span>${badge(duplicateLabel(skill.duplicate), statusMeta(skill).className)}<b>发现 ${skills.filter((s) => s.name === skill.name).length} 个相关实例</b><small>查看路径与文件差异</small></span><i>›</i></button>` : ""}
    ${skill.status === "repair" ? `<button class="repair-callout" data-action="simulate" data-message="已定位 frontmatter 第 3 行的解析错误"><b>SKILL.md 无法解析</b><span>第 3 行缺少 description 字段。打开编辑器修复。</span></button>` : ""}
    <dl class="metadata-grid"><div><dt>分组</dt><dd>${escapeHtml(skill.group)}</dd></div><div><dt>文件</dt><dd>${skill.files} 个 · ${skill.lines} 行</dd></div><div><dt>路径</dt><dd class="mono">${escapeHtml(skill.path)}</dd></div><div><dt>标签</dt><dd>${skill.tags.map((tag) => `<span class="text-tag">${escapeHtml(tag)}</span>`).join("")}</dd></div></dl>
    <div class="content-preview"><div><span>SKILL.md</span><button data-action="simulate" data-message="已复制文件路径">复制路径</button></div><pre><code>---\nname: ${escapeHtml(skill.name)}\ndescription: ${escapeHtml(skill.description.slice(0, 55))}\n---\n\n# ${escapeHtml(skill.name)}\n\n使用此 Skill 完成清晰、可验证的任务。</code></pre></div>
  </div>`;
}

function renderVariantB() {
  const groups = duplicateGroups();
  const active = groups.find((group) => group.name === state.selectedDuplicateName) || groups[0];
  if (active) state.selectedDuplicateName = active.name;
  const exactCount = groups.filter((group) => group.status === "exact").length;
  const suspectCount = groups.filter((group) => group.status === "suspect").length;
  const conflictCount = groups.filter((group) => group.status === "conflict").length;
  return `<div class="prototype variant-b">
    <header class="audit-header">
      <div class="audit-brand">${renderAppMark()}<div><span>Skill 管理器 / 重复检查</span><h1>把相似，变成确定。</h1></div></div>
      <div class="audit-actions"><button class="button audit-ghost" data-action="simulate" data-message="重新扫描完成，共发现 ${groups.length} 组结果">重新扫描</button><button class="button audit-primary" data-action="simulate" data-message="检查规则面板已打开">检查规则</button></div>
    </header>
    <section class="audit-summary">
      <button class="audit-metric active" data-audit-filter="all"><span>待检查</span><b>${groups.length}</b><small>组相关实例</small></button>
      <button class="audit-metric exact" data-audit-filter="exact"><span>完全重复</span><b>${exactCount}</b><small>可以安全归并</small></button>
      <button class="audit-metric suspect" data-audit-filter="suspect"><span>疑似重复</span><b>${suspectCount}</b><small>建议人工确认</small></button>
      <button class="audit-metric conflict" data-audit-filter="conflict"><span>同名冲突</span><b>${conflictCount}</b><small>内容差异明显</small></button>
      <div class="audit-progress"><div><span>本地健康度</span><b>86</b><small>/ 100</small></div><div class="progress-track"><i style="width:86%"></i></div><p>处理 3 组可提升至 94</p></div>
    </section>
    <div class="audit-workspace">
      <aside class="review-queue">
        <div class="queue-head"><div><p class="eyebrow">审阅队列</p><h2>${groups.length} 组待处理</h2></div><button data-action="simulate" data-message="已按风险从高到低排列">风险优先 ↕</button></div>
        <div class="queue-search">${renderSearch("搜索重复组")}</div>
        <div class="queue-list">${groups.map((group, index) => renderDuplicateQueueItem(group, index, active?.name === group.name)).join("") || '<div class="empty-state"><b>没有匹配组</b><span>清空搜索后查看全部结果。</span></div>'}</div>
        <div class="queue-footer"><span><i></i> 扫描规则 v1</span><button data-action="simulate" data-message="已打开忽略列表">查看忽略项</button></div>
      </aside>
      ${active ? renderComparisonStage(active) : '<main class="comparison-stage"><div class="empty-stage">没有可比较的重复组</div></main>'}
    </div>
  </div>`;
}

function renderDuplicateQueueItem(group, index, active) {
  const cls = group.status === "exact" ? "success" : group.status === "suspect" ? "warning" : "danger";
  return `<button class="queue-item ${active ? "active" : ""}" data-duplicate-group="${group.name}">
    <span class="queue-index">${String(index + 1).padStart(2, "0")}</span>
    <span class="queue-main"><b>${escapeHtml(group.name)}</b><small>${group.instances.length} 个实例 · ${group.instances.map((item) => item.root.split("/").slice(-2).join("/")).join(" · ")}</small></span>
    <span class="confidence"><b>${group.confidence}%</b><small>相似度</small></span>
    ${badge(duplicateLabel(group.status), cls)}
  </button>`;
}

function renderComparisonStage(group) {
  const [left, right = group.instances[0]] = group.instances;
  const cls = group.status === "exact" ? "success" : group.status === "suspect" ? "warning" : "danger";
  return `<main class="comparison-stage">
    <div class="comparison-head"><div><span class="breadcrumb">重复检查 / ${duplicateLabel(group.status)}</span><h2>${escapeHtml(group.name)}</h2><p>${group.status === "exact" ? "目录有效内容完全一致，可以选择保留方式。" : group.status === "suspect" ? "名称一致，正文与附属文件存在少量差异。" : "名称一致，但用途描述和正文结构差异明显。"}</p></div><div class="comparison-head-actions"><button class="button ghost-light" data-action="simulate" data-message="已将 ${escapeHtml(group.name)} 标记为不同 Skill">不是重复</button><button class="button warning-solid" data-action="merge-preview">预览归并</button></div></div>
    <div class="instance-compare">
      ${renderInstanceCard(left, "A", true)}
      <div class="compare-axis"><span>对比</span><i></i><b>${group.confidence}%</b><small>内容相似</small></div>
      ${renderInstanceCard(right, "B", false)}
    </div>
    <div class="diff-panel">
      <div class="diff-toolbar"><div><b>文件差异</b>${badge(`${left.files + right.files} 项已比较`, "neutral")}</div><div><button class="active">全部</button><button>已修改 2</button><button>仅一侧 1</button></div></div>
      <div class="diff-file-tabs"><button class="active"><i class="modified"></i>SKILL.md <span>± 8</span></button><button><i></i>references/examples.md</button><button><i class="added"></i>assets/cover.png <span>仅 B</span></button></div>
      <div class="diff-code"><div class="diff-col-label"><span>A · ${escapeHtml(left.root)}</span><span>B · ${escapeHtml(right.root)}</span></div>${renderDiffLines(group)}</div>
    </div>
    <footer class="decision-bar"><div><span class="status-pulse ${cls}"></span><p><b>尚未做出决定</b><small>所有操作都会先生成本地备份，并展示最终文件变化。</small></p></div><div><button class="button ghost-light" data-action="simulate" data-message="已暂时忽略 ${escapeHtml(group.name)}">稍后处理</button><button class="button warning-solid" data-action="merge-preview">选择主实例并归并</button></div></footer>
  </main>`;
}

function renderInstanceCard(skill, label, primary) {
  return `<article class="instance-card ${primary ? "primary" : ""}"><div class="instance-label"><span>${label}</span>${primary ? badge("建议主实例", "success") : ""}</div><h3>${escapeHtml(skill.name)}</h3><p>${escapeHtml(skill.description)}</p><dl><div><dt>根目录</dt><dd class="mono">${escapeHtml(skill.root)}</dd></div><div><dt>文件</dt><dd>${skill.files} 个 · ${skill.lines} 行</dd></div><div><dt>修改</dt><dd>${formatAge(skill.updatedMinutes)}</dd></div></dl><div class="instance-footer">${clientChips(skill.clients)}<label><input type="radio" name="primary" ${primary ? "checked" : ""} /> 设为主实例</label></div></article>`;
}

function renderDiffLines(group) {
  const lines = group.status === "exact"
    ? [[" ", "name: " + group.name, "name: " + group.name], [" ", "description: 管理本地 Skill", "description: 管理本地 Skill"], [" ", "# 使用方式", "# 使用方式"]]
    : [[" ", "name: " + group.name, "name: " + group.name], ["-", "description: 构建有辨识度的界面", ""], ["+", "", "description: 构建有辨识度的中文界面"], [" ", "# 工作流程", "# 工作流程"], ["-", "先明确视觉方向", ""], ["+", "", "先明确产品语境与视觉方向"], ["+", "", "最后检查中文文案与密度"]];
  return lines.map(([type, left, right], index) => `<div class="diff-line ${type === "+" ? "add" : type === "-" ? "remove" : ""}"><span class="line-no">${index + 1}</span><code>${escapeHtml(left)}</code><span class="line-no">${index + 1}</span><code>${escapeHtml(right)}</code></div>`).join("");
}

function renderVariantC() {
  const visible = getFilteredSkills();
  const groups = [...new Set(skills.map((skill) => skill.group))];
  const displayGroups = state.selectedGroup === "全部分组" ? groups.slice(0, 6) : [state.selectedGroup];
  const skill = selectedSkill();
  return `<div class="prototype variant-c">
    <header class="collections-header">
      <div class="collections-brand">${renderAppMark()}<div><span>本地 Skill 档案</span><h1>把能力放回顺手的位置</h1></div></div>
      <div class="collections-command">${renderSearch("搜索 35 个本地 Skill")}</div>
      <div class="collections-actions"><button class="round-action" data-action="simulate" data-message="已创建新分组">＋</button><button class="avatar-action" data-action="simulate" data-message="已打开应用设置">YC</button></div>
    </header>
    <nav class="collection-tabs">
      <button class="${state.selectedGroup === "全部分组" ? "active" : ""}" data-group="全部分组"><span>全部分组</span><b>${skills.length}</b></button>
      ${groups.map((group) => `<button class="${state.selectedGroup === group ? "active" : ""}" data-group="${escapeHtml(group)}"><span>${escapeHtml(group)}</span><b>${skills.filter((s) => s.group === group).length}</b></button>`).join("")}
    </nav>
    <div class="collection-toolbar"><div><button class="active" data-filter="all">全部</button><button data-filter="duplicates">待整理 ${skills.filter((s) => s.duplicate !== "none").length}</button><button data-filter="repair">需要修复 ${skills.filter((s) => s.status === "repair").length}</button></div><div>${renderSelects()}<button class="button collection-button" data-action="simulate" data-message="已进入自定义排列模式">自定义排列</button></div></div>
    <main class="shelf-canvas">
      <div class="shelf-intro"><p class="eyebrow">${state.selectedGroup === "全部分组" ? "你的工作空间" : "当前分组"}</p><h2>${state.selectedGroup === "全部分组" ? "按工作语境拿取 Skill" : escapeHtml(state.selectedGroup)}</h2><span>${visible.length} 个实例 · ${new Set(visible.map((item) => item.name)).size} 个名称</span></div>
      <div class="shelves">${displayGroups.map((group) => renderShelf(group, visible.filter((skill) => skill.group === group))).join("")}</div>
    </main>
    <aside class="peek-drawer">
      <button class="peek-handle" data-action="simulate" data-message="详情抽屉已收起"><span></span></button>
      <div class="peek-heading"><div class="skill-avatar">${skill.name.slice(0, 2).toUpperCase()}</div><div><span>${escapeHtml(skill.group)} / ${escapeHtml(skill.root)}</span><h2>${escapeHtml(skill.name)}</h2></div><button class="icon-button" data-action="simulate" data-message="详情抽屉已关闭">×</button></div>
      <p>${escapeHtml(skill.description)}</p>
      <div class="peek-meta"><div><span>安装位置</span>${clientChips(skill.clients)}</div><div><span>状态</span>${badge(statusMeta(skill).label, statusMeta(skill).className)}</div><div><span>标签</span><p>${skill.tags.map((tag) => `<button data-action="simulate" data-message="已筛选标签 ${escapeHtml(tag)}">#${escapeHtml(tag)}</button>`).join("")}</p></div></div>
      <div class="peek-actions"><button class="button collection-primary" data-action="simulate" data-message="已打开 ${escapeHtml(skill.name)} 编辑器">打开并编辑</button><button class="button collection-button" data-action="simulate" data-message="已打开移动到分组面板">移动到…</button><button class="icon-button danger-text" data-action="simulate" data-message="已打开废纸篓预览">⌫</button></div>
    </aside>
  </div>`;
}

function renderShelf(group, groupSkills) {
  if (!groupSkills.length) return "";
  return `<section class="shelf"><header><div><i></i><h3>${escapeHtml(group)}</h3><span>${groupSkills.length}</span></div><div><button data-action="simulate" data-message="已将 ${escapeHtml(group)} 设为常用分组">☆</button><button data-group="${escapeHtml(group)}">查看全部 →</button></div></header><div class="shelf-track">${groupSkills.slice(0, 8).map((skill) => renderSkillCard(skill)).join("")}<button class="add-card" data-action="simulate" data-message="已打开添加 Skill 到 ${escapeHtml(group)}"><span>＋</span><b>添加到分组</b></button></div></section>`;
}

function renderSkillCard(skill) {
  const meta = statusMeta(skill);
  return `<button class="skill-card ${skill.id === state.selectedSkillId ? "selected" : ""} spine-${meta.className}" data-select-skill="${skill.id}"><div class="card-top"><span class="mini-file">SK</span>${badge(meta.label, meta.className)}</div><strong>${escapeHtml(skill.name)}</strong><p>${escapeHtml(skill.description)}</p><div class="card-tags">${skill.tags.map((tag) => `<span>#${escapeHtml(tag)}</span>`).join("")}</div><div class="card-foot">${clientChips(skill.clients, true)}<span>${formatAge(skill.updatedMinutes)}</span></div></button>`;
}

function renderSwitcher() {
  const variants = Object.keys(variantNames);
  const index = variants.indexOf(state.variant);
  const prev = variants[(index - 1 + variants.length) % variants.length];
  const next = variants[(index + 1) % variants.length];
  return `<div class="prototype-switcher" role="navigation" aria-label="原型方案切换"><button data-variant="${prev}" aria-label="上一个方案">←</button><div><span>可丢弃 UI 原型</span><b>${state.variant} — ${variantNames[state.variant]}</b></div><button data-variant="${next}" aria-label="下一个方案">→</button></div>`;
}

function renderStateBar() {
  const filterLabel = state.filter === "all" ? "全部" : state.filter === "duplicates" ? "存在重复" : state.filter === "repair" ? "需要修复" : state.filter;
  return `<div class="prototype-state"><i></i><b>当前状态</b><span>方案 ${state.variant}</span><span>检索：${state.query ? `“${escapeHtml(state.query)}”` : "无"}</span><span>筛选：${escapeHtml(filterLabel)}</span><span>排序：${escapeHtml(state.sort)}</span><span>选中：${state.selectedIds.size}</span><span>${escapeHtml(state.lastAction)}</span></div>`;
}

function renderModal() {
  if (state.modal !== "merge") return "";
  const group = duplicateGroups().find((item) => item.name === state.selectedDuplicateName);
  if (!group) return "";
  return `<div class="modal-backdrop" data-action="close-modal"><section class="merge-modal" role="dialog" aria-modal="true" aria-labelledby="merge-title" onclick="event.stopPropagation()">
    <header><div><span class="step-label">归并预览 · 3 / 3</span><h2 id="merge-title">确认 ${escapeHtml(group.name)} 的文件变化</h2></div><button class="icon-button" data-action="close-modal">×</button></header>
    <div class="merge-flow"><div class="done"><b>1</b><span>选择主实例</span></div><i></i><div class="done"><b>2</b><span>选择目标</span></div><i></i><div class="active"><b>3</b><span>预览并确认</span></div></div>
    <div class="backup-note"><span>✓</span><div><b>执行前自动备份</b><p>备份会保存在本地操作记录中，可以随时撤销。</p></div></div>
    <div class="change-list"><div class="change-head"><span>目标实例</span><span>变化</span></div>${group.instances.slice(1).map((instance) => `<div><span><b>${escapeHtml(instance.root)}</b><small class="mono">${escapeHtml(instance.path)}</small></span><span class="change-chips">${badge("覆盖 2", "warning")}${badge("新增 1", "success")}${group.status === "conflict" ? badge("删除 1", "danger") : ""}</span></div>`).join("") || `<div><span><b>${escapeHtml(group.instances[0].root)}</b><small>当前实例无需变化</small></span>${badge("无变化", "quiet")}</div>`}</div>
    <label class="confirm-check"><input type="checkbox" checked /> 我已查看文件差异，确认使用主实例内容</label>
    <footer><button class="button ghost" data-action="close-modal">返回检查</button><button class="button warning-solid" data-action="apply-merge">生成备份并归并</button></footer>
  </section></div>`;
}

function render() {
  app.innerHTML = `${state.variant === "A" ? renderVariantA() : state.variant === "B" ? renderVariantB() : renderVariantC()}${renderStateBar()}${renderSwitcher()}${renderModal()}`;
  document.title = `${state.variant} · ${variantNames[state.variant]} · Skill 管理器`;
}

function setVariant(variant) {
  state.variant = variant;
  const url = new URL(location.href);
  url.searchParams.set("variant", variant);
  history.replaceState({}, "", url);
  state.lastAction = `已切换到${variantNames[variant]}`;
  render();
  announce(state.lastAction);
}

function announce(message) {
  announcer.textContent = message;
}

function simulate(message) {
  state.lastAction = message || "模拟操作完成";
  render();
  announce(state.lastAction);
}

document.addEventListener("click", (event) => {
  const target = event.target.closest("button, [data-select-skill]");
  if (!target) return;
  if (target.dataset.variant) return setVariant(target.dataset.variant);
  if (target.dataset.selectSkill) {
    state.selectedSkillId = target.dataset.selectSkill;
    state.lastAction = `已选择 ${selectedSkill().name}`;
    return render();
  }
  if (target.dataset.toggleSelect) {
    event.stopPropagation();
    const id = target.dataset.toggleSelect;
    state.selectedIds.has(id) ? state.selectedIds.delete(id) : state.selectedIds.add(id);
    state.lastAction = `已选择 ${state.selectedIds.size} 项`;
    return render();
  }
  if (target.dataset.filter) {
    state.filter = target.dataset.filter;
    state.selectedGroup = "全部分组";
    state.lastAction = `筛选已更新`;
    return render();
  }
  if (target.dataset.group) {
    state.selectedGroup = target.dataset.group;
    state.filter = "all";
    state.lastAction = `已打开 ${state.selectedGroup}`;
    return render();
  }
  if (target.dataset.duplicateGroup) {
    state.selectedDuplicateName = target.dataset.duplicateGroup;
    state.lastAction = `正在比较 ${state.selectedDuplicateName}`;
    return render();
  }
  if (target.dataset.detailTab) {
    state.detailTab = target.dataset.detailTab;
    return render();
  }
  const action = target.dataset.action;
  if (action === "clear-search") {
    state.query = "";
    state.filter = "all";
    state.selectedGroup = "全部分组";
    state.lastAction = "已清空检索条件";
    return render();
  }
  if (action === "jump-duplicate") {
    state.selectedDuplicateName = selectedSkill().name;
    return setVariant("B");
  }
  if (action === "merge-preview") {
    state.modal = "merge";
    state.lastAction = "正在预览归并变化";
    return render();
  }
  if (action === "close-modal") {
    state.modal = null;
    state.lastAction = "已返回差异检查";
    return render();
  }
  if (action === "apply-merge") {
    state.modal = null;
    state.lastAction = `已模拟归并 ${state.selectedDuplicateName}，未修改真实文件`;
    return render();
  }
  if (action === "simulate") return simulate(target.dataset.message);
});

document.addEventListener("input", (event) => {
  if (!event.target.matches("[data-search]")) return;
  const cursor = event.target.selectionStart;
  state.query = event.target.value;
  state.lastAction = state.query ? `正在检索 ${state.query}` : "已清空检索";
  render();
  const input = document.querySelector("[data-search]");
  input?.focus();
  input?.setSelectionRange(cursor, cursor);
});

document.addEventListener("change", (event) => {
  if (event.target.matches("[data-filter-select]")) {
    state.filter = event.target.value;
    state.lastAction = "筛选条件已更新";
    render();
  }
  if (event.target.matches("[data-sort-select]")) {
    state.sort = event.target.value;
    state.lastAction = "排序方式已更新";
    render();
  }
});

document.addEventListener("keydown", (event) => {
  const editing = event.target.matches("input, textarea, select, [contenteditable]");
  if (editing) return;
  if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
    event.preventDefault();
    const variants = Object.keys(variantNames);
    const current = variants.indexOf(state.variant);
    const direction = event.key === "ArrowRight" ? 1 : -1;
    setVariant(variants[(current + direction + variants.length) % variants.length]);
  }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    document.querySelector("[data-search]")?.focus();
  }
  if (event.key === "Escape" && state.modal) {
    state.modal = null;
    render();
  }
});

render();
