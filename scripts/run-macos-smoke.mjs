import { spawn } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

if (process.platform !== "darwin") {
  throw new Error("macOS 桌面冒烟只能在 macOS 上运行。");
}

const sandbox = await mkdtemp(join(tmpdir(), "skill-management-desktop-smoke-"));
const root = join(sandbox, "root");
const database = join(sandbox, "app-data", "skill-management.sqlite3");
const home = join(sandbox, "home");
const resultFile = join(sandbox, "result.txt");
const developerHome = process.env.HOME;
if (!developerHome) throw new Error("无法定位开发环境 HOME。");

try {
  await mkdir(home, { recursive: true });
  await writeSkill(
    join(root, "release-main"),
    "release-notes",
    "整理版本发布说明。",
    "# 发布说明\n\n桌面验收主实例，标记 desktop-smoke-source。\n",
  );
  await writeSkill(
    join(root, "release-copy"),
    "release-notes",
    "旧版发布流程。",
    "# 发布说明\n\n桌面验收目标旧内容，标记 desktop-smoke-target。\n",
  );
  await writeFile(join(root, "release-copy", "legacy.txt"), "归并前目标附件。\n");
  await mkdir(join(root, "needs-repair"), { recursive: true });
  await writeFile(
    join(root, "needs-repair", "SKILL.md"),
    "---\nname: needs-repair\n---\n\n# 缺少描述\n",
  );

  await run("npm", ["run", "build"]);
  await run(
    "cargo",
    [
      "run",
      "--release",
      "-p",
      "skill-management-desktop",
      "--features",
      "desktop-smoke,custom-protocol",
    ],
    {
      ...process.env,
      HOME: home,
      CARGO_HOME: process.env.CARGO_HOME ?? join(developerHome, ".cargo"),
      RUSTUP_HOME: process.env.RUSTUP_HOME ?? join(developerHome, ".rustup"),
      SKILL_MANAGEMENT_SMOKE_DATABASE: database,
      SKILL_MANAGEMENT_SMOKE_ROOT: root,
      SKILL_MANAGEMENT_SMOKE_RESULT: resultFile,
    },
    120_000,
  );
  const result = await readFile(resultFile, "utf8").catch(() => "未生成验收结果");
  if (result.trim() !== "ok") throw new Error(result.trim());
} finally {
  await rm(sandbox, { recursive: true, force: true });
}

async function writeSkill(directory, name, description, body) {
  await mkdir(directory, { recursive: true });
  await writeFile(
    join(directory, "SKILL.md"),
    `---\nname: ${name}\ndescription: ${description}\n---\n\n${body}`,
  );
}

function run(command, args, environment = process.env, timeout = 30_000) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { env: environment, stdio: "inherit" });
    const timer = setTimeout(() => {
      child.kill("SIGTERM");
      reject(new Error(`命令运行超时：${command} ${args.join(" ")}`));
    }, timeout);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      if (code === 0) resolve();
      else reject(new Error(`命令失败：${command}，退出码 ${code ?? signal}`));
    });
  });
}
