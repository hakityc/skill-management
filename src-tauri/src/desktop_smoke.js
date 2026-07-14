(() => {
  if (sessionStorage.getItem("skill-management-smoke") === "running") return;
  const invoke = window.__TAURI_INTERNALS__.invoke;
  const root = window.__SKILL_MANAGEMENT_SMOKE_ROOT__;
  const wait = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));
  const until = async (read, label, timeout = 5_000) => {
    const started = performance.now();
    while (performance.now() - started < timeout) {
      const value = read();
      if (value) return value;
      await wait(50);
    }
    throw new Error(`等待界面超时：${label}`);
  };
  const mark = (label) => invoke("create_skill_group", {
    name: `桌面验收阶段-${label}`,
  });
  const button = (text, startsWith = false) =>
    [...document.querySelectorAll("button")].find((candidate) => {
      const content = candidate.textContent.trim();
      return startsWith ? content.startsWith(text) : content === text;
    });
  const clickButton = async (text, startsWith = false) => {
    const target = await until(() => button(text, startsWith), `按钮 ${text}`);
    if (target.disabled) {
      await until(() => !target.disabled, `按钮 ${text} 可用`);
    }
    target.click();
  };
  const setValue = (element, value) => {
    const prototype = element instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : element instanceof HTMLSelectElement
        ? HTMLSelectElement.prototype
        : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, "value").set.call(element, value);
    element.dispatchEvent(new Event(element instanceof HTMLSelectElement ? "change" : "input", {
      bubbles: true,
    }));
  };

  const run = async () => {
    if (!sessionStorage.getItem("skill-management-smoke")) {
      sessionStorage.setItem("skill-management-smoke", "authorized");
      await invoke("create_skill_group", { name: "桌面验收阶段-开始" });
      await invoke("authorize_skill_root", { path: root });
      location.reload();
      return;
    }
    sessionStorage.setItem("skill-management-smoke", "running");
    await invoke("create_skill_group", { name: "桌面验收阶段-界面" });
    const search = await until(
      () => document.querySelector('input[aria-label="搜索 Skill"]'),
      "Skill 检索框",
    );
    await mark("找到检索框");
    await until(
      () => document.querySelector('[aria-label="needs-repair，需要修复"]'),
      "需要修复的 Skill 实例",
    );
    await mark("找到需要修复实例");

    setValue(search, "desktop-smoke-source");
    const sourceRow = await until(() => {
      const rows = document.querySelectorAll('[aria-label="本地 Skill"] > li');
      return rows.length === 1 ? rows[0] : null;
    }, "检索主实例");
    await mark("检索完成");
    sourceRow.click();
    await until(() => {
      const detail = document.querySelector('[aria-label="Skill 详情"]');
      return detail && [...detail.querySelectorAll("dd")]
        .some((element) => element.textContent.trim() === "release-main")
        ? detail
        : null;
    }, "主实例详情");
    await clickButton("编辑 Skill");
    const markdown = await until(
      () => document.querySelector('textarea[aria-label="Markdown 正文"]'),
      "Markdown 编辑器",
    );
    setValue(markdown, "# 发布说明\n\n桌面验收已编辑，标记 desktop-smoke-edited。\n");
    await until(
      () => [...document.querySelectorAll(".markdown-preview pre")]
        .find((element) => element.textContent.includes("desktop-smoke-edited")),
      "编辑内容进入 React 状态",
    );
    await clickButton("预览变化");
    const previewOutcome = await until(
      () => document.querySelector(".change-plan")
        ?? document.querySelector('.skill-editor [role="alert"]'),
      "变化计划",
    );
    if (previewOutcome.getAttribute("role") === "alert") {
      throw new Error(`编辑预览失败：${previewOutcome.textContent.trim()}`);
    }
    await clickButton("确认保存");
    await until(() => !document.querySelector("#editor-title"), "编辑器关闭");
    await mark("编辑完成");

    setValue(search, "");
    await until(
      () => document.querySelectorAll('[aria-label="本地 Skill"] > li').length === 3,
      "恢复全部 Skill 实例",
    );
    await clickButton("管理 Skill 组");
    const groupName = await until(
      () => document.querySelector('input[aria-label="新 Skill 组名称"]'),
      "Skill 组名称",
    );
    setValue(groupName, "桌面验收");
    await clickButton("创建 Skill 组");
    await until(
      () => document.querySelector('input[aria-label="Skill 组名称 桌面验收"]'),
      "已创建的 Skill 组",
    );
    document.querySelector('[role="dialog"][aria-label="管理 Skill 组"] button').click();
    await until(
      () => !document.querySelector('[role="dialog"][aria-label="管理 Skill 组"]'),
      "关闭 Skill 组管理",
    );
    await mark("创建组完成");

    const skillCheckboxes = await until(() => {
      const candidates = document.querySelectorAll('input[aria-label="选择 release-notes"]');
      return candidates.length === 2 ? [...candidates] : null;
    }, "两个 release-notes 实例");
    for (const checkbox of skillCheckboxes) checkbox.click();
    await clickButton("批量整理");
    const acceptanceGroup = await until(
      () => document.querySelector('input[aria-label="Skill 组 桌面验收"]'),
      "整理对话框中的 Skill 组",
    );
    acceptanceGroup.click();
    await clickButton("应用整理");
    await until(
      () => !document.querySelector('[role="dialog"][aria-label^="整理 "]'),
      "完成批量整理",
    );
    await mark("批量整理完成");

    document.querySelector('button[aria-label="重复检查"]').click();
    await until(() => button("预览安全归并"), "安全归并区域");
    await mark("重复检查完成");
    const master = await until(() =>
      [...document.querySelectorAll('input[type="radio"][aria-label^="主实例 "]')]
        .find((candidate) => candidate.closest("label").textContent.includes("release-main")),
    "release-main 主实例");
    master.click();
    const target = await until(() =>
      [...document.querySelectorAll('input[type="checkbox"][aria-label^="归并目标 "]')]
        .find((candidate) => candidate.closest("label").textContent.includes("release-copy")),
    "release-copy 归并目标");
    if (!target.checked) target.click();
    await clickButton("预览安全归并");
    await clickButton("确认归并", true);
    const undo = await until(
      () => button("撤销归并 #", true),
      "可撤销的归并记录",
    );
    undo.click();
    await until(
      () => [...document.querySelectorAll(".merge-history small")]
        .some((element) => element.textContent.trim() === "已撤销"),
      "归并已经撤销",
    );
    await mark("归并撤销完成");
    await invoke("create_skill_group", { name: "桌面验收完成" });
  };

  run().catch(async (error) => {
    console.error(error);
    try {
      await invoke("create_skill_group", {
        name: `桌面验收错误-${String(error?.message ?? error).slice(0, 80)}`,
      });
      await invoke("create_skill_group", { name: "桌面验收失败" });
    } catch (_) {
      // Rust 侧超时会报告失败。
    }
  });
})();
