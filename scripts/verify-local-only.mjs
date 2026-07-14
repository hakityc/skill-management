import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const failures = [];
const fail = (message) => failures.push(message);

if (existsSync(join(root, "prototype/skill-manager-ui"))) {
  fail("throwaway 原型目录仍然存在");
}

const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const allowedRuntimeDependencies = new Set([
  "@tauri-apps/api",
  "@tauri-apps/plugin-dialog",
  "react",
  "react-dom",
]);
for (const dependency of Object.keys(packageJson.dependencies ?? {})) {
  if (!allowedRuntimeDependencies.has(dependency)) {
    fail(`发现未批准的运行时依赖：${dependency}`);
  }
}

const capability = JSON.parse(
  readFileSync(join(root, "src-tauri/capabilities/default.json"), "utf8"),
);
if (capability.remote != null) {
  fail("Tauri capability 不得授权远程来源");
}
const allowedPermissions = new Set(["core:default", "dialog:allow-open"]);
for (const permission of capability.permissions ?? []) {
  if (!allowedPermissions.has(permission)) {
    fail(`发现未批准的 Tauri 权限：${permission}`);
  }
}

const tauriConfig = JSON.parse(
  readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"),
);
const csp = tauriConfig.app?.security?.csp;
if (typeof csp !== "string" || !csp.includes("default-src 'self'")) {
  fail("打包应用必须启用仅本地内容安全策略");
}
if (typeof csp === "string" && /https:|wss:|\*/.test(csp)) {
  fail("打包应用内容安全策略不得允许外部网络来源");
}

const productionRoots = [
  join(root, "src"),
  join(root, "src-tauri/src"),
  join(root, "crates/skill-workspace/src"),
];
const sourceExtensions = new Set([".ts", ".tsx", ".rs"]);
const forbiddenPatterns = [
  ["浏览器 fetch", /\bfetch\s*\(/],
  ["XMLHttpRequest", /\bXMLHttpRequest\b/],
  ["WebSocket", /\bWebSocket\b/],
  ["EventSource", /\bEventSource\b/],
  ["Rust HTTP 客户端", /\b(?:reqwest|ureq|hyper|tonic)\b/],
  ["Rust 网络套接字", /\b(?:TcpStream|TcpListener|UdpSocket)\b/],
  ["生产原型或 mock 开关", /\b(?:prototype|mock)\b/i],
];

for (const file of sourceFiles(productionRoots)) {
  const content = readFileSync(file, "utf8");
  for (const [label, pattern] of forbiddenPatterns) {
    if (pattern.test(content)) {
      fail(`${file.slice(root.length + 1)} 包含${label}`);
    }
  }
}

if (failures.length > 0) {
  console.error("本地离线边界检查失败：");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("本地离线边界检查通过：无账号、无外部服务、无生产原型入口。");

function sourceFiles(directories) {
  const files = [];
  for (const directory of directories) visit(directory, files);
  return files;
}

function visit(path, files) {
  for (const name of readdirSync(path)) {
    const entry = join(path, name);
    if (statSync(entry).isDirectory()) visit(entry, files);
    else if (sourceExtensions.has(extname(entry))) files.push(entry);
  }
}
