import { describe, expect, test } from "vitest";

import { readableError } from "./errors";

describe("中文错误边界", () => {
  test("保留可理解的中文错误并隐藏平台英文诊断", () => {
    expect(readableError(new Error("新的归并预览失败"))).toBe("新的归并预览失败");
    expect(readableError(new Error("No such file or directory (os error 2)"))).toBe(
      "操作失败，请重试；如果问题持续，请重新扫描 Skill 根目录。",
    );
    expect(
      readableError(new Error("无法访问目录：Permission denied (os error 13)")),
    ).toBe("操作失败，请重试；如果问题持续，请重新扫描 Skill 根目录。");
  });
});
