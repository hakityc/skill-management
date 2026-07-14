import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const indexHtml = readFileSync(join(root, "index.html"), "utf8");
const styles = readFileSync(join(root, "src/styles.css"), "utf8");
const failures = [];

if (!indexHtml.includes('<html lang="zh-CN">')) {
  failures.push("默认界面语言必须是 zh-CN");
}
for (const selector of [
  "button:focus-visible",
  "input:focus-visible",
  "select:focus-visible",
  "textarea:focus-visible",
  "[tabindex]:focus-visible",
]) {
  if (!styles.includes(selector)) failures.push(`缺少可见键盘焦点样式：${selector}`);
}

if (failures.length > 0) {
  console.error("中文 UI 契约检查失败：");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("中文 UI 契约检查通过：默认语言和键盘焦点样式均已固定。");
