import React from "react";
import { invokeCommand } from "../../api/tauri";
import { isAlertApplyResult } from "../../features/translation/LogToasts";
import { inferPckTargetPath } from "../../features/translation/translationUtils";
import {
  previewJsonReport,
  previewJsonSheet,
  previewJsonValidation,
  previewTranslationProject,
} from "../../previewData";
import type {
  JsonApply,
  JsonSheetAction,
  JsonTranslationSheet,
  JsonValidation,
} from "../../types";
import { formatError, jsonCommandLabel } from "../../utils/logging";
import { isPreviewRuntime } from "../../utils/runtime";
import type { RunJsonTool, TranslationActionsParams } from "./types";

export function useJsonToolRunner({
  appendLog,
  jsonApplyResult,
  setBusy,
  setCompareSamplePaths,
  setCompareValuesByLanguage,
  setCompareViewEnabled,
  setJsonApplyResult,
  setJsonExistingSheet,
  setJsonOutputSheet,
  setJsonPckTargetPath,
  setJsonReport,
  setJsonSheet,
  setJsonToolError,
  setJsonValidation,
  setPasteCandidatesByKey,
  setSelectedRows,
  setTranslationProject,
}: TranslationActionsParams): RunJsonTool {
  React.useEffect(() => {
    if (!jsonApplyResult || isAlertApplyResult(jsonApplyResult)) {
      return;
    }
    const timeoutId = window.setTimeout(() => {
      setJsonApplyResult((current) => (current && !isAlertApplyResult(current) ? null : current));
    }, 5500);
    return () => window.clearTimeout(timeoutId);
  }, [jsonApplyResult, setJsonApplyResult]);

  return async function runJsonTool(command: string, args: Record<string, unknown>): Promise<boolean> {
    setBusy(command);
    setJsonToolError("");
    try {
      if (isPreviewRuntime()) {
        setJsonSheet(previewJsonSheet);
        setJsonReport(previewJsonReport);
        setTranslationProject(previewTranslationProject);
        setPasteCandidatesByKey({});
        setJsonValidation(previewJsonValidation);
        appendLog(`Preview action: ${command}`);
        return true;
      }
      if (command === "create_json_translation_sheet") {
        const result = await invokeCommand<JsonSheetAction>(command, args);
        setJsonSheet(result.sheet);
        setJsonReport(result.report);
        setJsonOutputSheet(result.report.sheet_path);
        setJsonExistingSheet(result.report.sheet_path);
        setJsonPckTargetPath(inferPckTargetPath(result.sheet));
        setJsonValidation(null);
        setJsonApplyResult(null);
        setCompareSamplePaths([]);
        setCompareValuesByLanguage({});
        setCompareViewEnabled(false);
        setSelectedRows([]);
        setPasteCandidatesByKey({});
        appendLog(result.message);
      } else if (command === "load_json_translation_sheet") {
        const result = await invokeCommand<JsonTranslationSheet>(command, args);
        setJsonSheet(result);
        setJsonPckTargetPath(inferPckTargetPath(result));
        setJsonValidation(null);
        setJsonApplyResult(null);
        setCompareSamplePaths([]);
        setCompareValuesByLanguage({});
        setCompareViewEnabled(false);
        setSelectedRows([]);
        setPasteCandidatesByKey({});
        appendLog("번역 시트 불러오기 완료");
      } else if (command === "validate_json_translation_sheet") {
        const result = await invokeCommand<JsonValidation>(command, args);
        setJsonValidation(result);
        appendLog(result.valid ? "검증 통과" : `수정 필요한 항목이 있습니다. 구조 경고 ${result.format_issues?.length ?? 0}개`);
      } else if (command === "apply_json_translation_sheet") {
        const result = await invokeCommand<JsonApply>(command, args);
        const message = result.installed_mod_path
          ? result.packed_pck_path
            ? `PCK 반영 완료: ${result.applied_entries}개 항목 적용`
            : `번역 파일 반영 완료: ${result.applied_entries}개 항목 적용`
          : result.packed_pck_path
          ? `PCK 생성 완료: ${result.applied_entries}개 항목 적용`
          : `번역 JSON 저장 완료: ${result.applied_entries}개 항목 적용`;
        setJsonApplyResult({ ...result, message });
        appendLog(
          result.installed_mod_path
            ? result.packed_pck_path
              ? `번역 적용 완료: ${result.applied_entries}개 항목 / 활성 모드 PCK 반영 (${result.installed_mod_path})`
              : `번역 적용 완료: ${result.applied_entries}개 항목 / 활성 모드 파일 반영 (${result.installed_mod_path})`
            : result.packed_pck_path
            ? `번역 적용 완료: ${result.applied_entries}개 항목 / PCK 생성 (${result.packed_pck_path})`
            : `번역 JSON 적용 완료: ${result.applied_entries}개 항목 (${result.language_output_path || result.output_path})`,
        );
      }
      return true;
    } catch (error) {
      const message = `${jsonCommandLabel(command)} 실패: ${formatError(error)}`;
      if (command === "apply_json_translation_sheet") {
        setJsonApplyResult({
          output_path: "",
          applied_entries: 0,
          message,
          error: true,
        });
      }
      setJsonToolError(message);
      appendLog(message);
      return false;
    } finally {
      setBusy(null);
    }
  };
}
