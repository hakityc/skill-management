const GENERIC_ERROR = "操作失败，请重试；如果问题持续，请重新扫描 Skill 根目录。";
const PLATFORM_ERROR =
  /\b(?:os error|no such file|permission denied|operation not permitted|database is|syntax error|unexpected token|failed to|cannot|could not)\b/i;

export function readableError(reason: unknown) {
  const message = reason instanceof Error ? reason.message : String(reason ?? "");
  const normalized = message.trim();
  if (!normalized || PLATFORM_ERROR.test(normalized) || !/[\u3400-\u9fff]/u.test(normalized)) {
    return GENERIC_ERROR;
  }
  return normalized;
}
