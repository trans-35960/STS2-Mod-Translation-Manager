import type { LogTone } from "../types";

export function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

export function logTone(log: string): LogTone {
  const text = log.toLowerCase();
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

export function shouldToastLog(log: string): boolean {
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
