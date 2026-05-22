import type { LogTone } from "../types";

export function formatError(error: unknown): string {
  if (error instanceof Error) {
    return [error.message, error.stack].filter(Boolean).join(" | ");
  }
  if (typeof error === "string") {
    return error;
  }
  if (error && typeof error === "object") {
    const record = error as Record<string, unknown>;
    const message = [record.message, record.error, record.reason]
      .filter((value): value is string => typeof value === "string" && value.length > 0)
      .join(" / ");
    if (message) {
      return message;
    }
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

export function formatCommandError(command: string, args: unknown, error: unknown): string {
  const details = summarizeCommandArgs(args);
  return `${command} 실패: ${formatError(error)}${details ? ` (${details})` : ""}`;
}

export function summarizeCommandArgs(args: unknown): string {
  if (!args || typeof args !== "object") {
    return "";
  }
  return Object.entries(args as Record<string, unknown>)
    .map(([key, value]) => summarizeArg(key, value))
    .filter(Boolean)
    .join(", ");
}

function summarizeArg(key: string, value: unknown): string {
  if (value === undefined || value === null || value === "") {
    return "";
  }
  if (typeof value === "string") {
    return `${key}=${truncate(value, 180)}`;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return `${key}=${String(value)}`;
  }
  if (Array.isArray(value)) {
    const sample = value
      .slice(0, 3)
      .map((item) => (typeof item === "string" ? item : summarizeCompactValue(item)))
      .filter(Boolean)
      .join(", ");
    return `${key}=${value.length}개${sample ? ` [${truncate(sample, 180)}]` : ""}`;
  }
  if (typeof value === "object") {
    const record = value as Record<string, unknown>;
    if (Array.isArray(record.entries)) {
      return `${key}=entries ${record.entries.length}개`;
    }
    return `${key}=${summarizeCompactValue(value)}`;
  }
  return "";
}

function summarizeCompactValue(value: unknown): string {
  try {
    return truncate(JSON.stringify(value), 180);
  } catch {
    return truncate(String(value), 180);
  }
}

function truncate(value: string, maxLength: number): string {
  return value.length <= maxLength ? value : `${value.slice(0, maxLength - 1)}…`;
}

export function logTone(log: string): LogTone {
  const text = normalizeSuccessCounts(log.toLowerCase());
  if (text.includes("완료") && !text.includes("실패") && !text.includes("failed") && !text.includes("error:")) {
    return "info";
  }
  if (
    [
      "오류",
      "실패",
      "exception",
      "error",
      "failed",
      "failure",
      "찾을 수 없습니다",
      "올바르지",
      "거부",
      "not found",
    ].some((keyword) => text.includes(keyword))
  ) {
    return "error";
  }
  if (
    [
      "경고",
      "수정 필요",
      "필요합니다",
      "없습니다",
      "미매칭",
      "중단",
      "warning",
      "warn",
      "missing",
      "skipped",
    ].some((keyword) => text.includes(keyword))
  ) {
    return "warn";
  }
  return "info";
}

function normalizeSuccessCounts(text: string): string {
  return text
    .replace(/실패\s*0\s*개/g, "")
    .replace(/failed\s*0\b/g, "")
    .replace(/0\s*failed\b/g, "");
}

export function shouldToastLog(log: string): boolean {
  if (log.includes("내보내기 완료")) {
    return true;
  }
  if (
    log.includes("진행 중 런 정리를 시작")
    || log.includes("진행 중 런 정리 완료")
    || log.includes("진행 중 런 정리를 시도")
    || log.includes("정리할 진행 중 런")
  ) {
    return true;
  }
  return logTone(log) !== "info";
}

export function jsonCommandLabel(command: string): string {
  switch (command) {
    case "create_json_translation_sheet":
      return "번역 시트 생성";
    case "recalculate_json_translation_sheet":
      return "번역 시트 재계산";
    case "load_json_translation_sheet":
      return "번역 시트 불러오기";
    case "validate_json_translation_sheet":
      return "번역 시트 검증";
    case "apply_json_translation_sheet":
      return "번역 저장/적용";
    case "export_translation_patch_mod":
      return "번역 모드 내보내기";
    default:
      return command;
  }
}
