import React from "react";
import { invokeCommand } from "../../api/tauri";
import type { CommandArgs, JsonToolCommand } from "../../api/tauri";
import type { JsonSheetAction, TranslatorSave } from "../../types";
import { isAlertApplyResult } from "../../features/translation/LogToasts";
import { inferPckTargetPath } from "../../features/translation/translationUtils";
import {
  previewJsonReport,
  previewJsonSheet,
  previewJsonValidation,
  previewTranslationProject,
} from "../../previewData";
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

  return async function runJsonTool(
    command: JsonToolCommand,
    args: CommandArgs[JsonToolCommand],
  ): Promise<boolean> {
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
      if (command === "create_json_translation_sheet" || command === "recalculate_json_translation_sheet") {
        const result = await invokeCommand(command, args as CommandArgs[typeof command]) as JsonSheetAction;
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
        const result = await invokeCommand("load_json_translation_sheet", args as CommandArgs["load_json_translation_sheet"]);
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
        const result = await invokeCommand("validate_json_translation_sheet", args as CommandArgs["validate_json_translation_sheet"]);
        setJsonValidation(result);
        appendLog(result.valid ? "검증 통과" : `수정 필요한 항목이 있습니다. 구조 경고 ${result.format_issues?.length ?? 0}개`);
      } else if (command === "apply_json_translation_sheet") {
        const result = await invokeCommand("apply_json_translation_sheet", args as CommandArgs["apply_json_translation_sheet"]);
        if (result.translator) {
          const summary = translatorSaveSummary(result.translator);
          setJsonApplyResult({ ...result, message: summary.message, error: summary.blocked });
          summary.details.forEach((line) => appendLog(line));
          return !summary.blocked;
        }
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

/** Toast message and log lines for a sheet saved into Sts2ModTranslator overrides. */
function translatorSaveSummary(save: TranslatorSave): { message: string; blocked: boolean; details: string[] } {
  if (save.conflicts.length > 0) {
    return {
      blocked: true,
      message: `Translator에 저장하지 않음: 게임 안 번역과 다른 항목 ${save.conflicts.length}개`,
      details: [
        `Translator 저장 중단: ${save.mod_id}/${save.language}에서 게임 안에서 따로 번역한 값과 시트 값이 다른 항목이 ${save.conflicts.length}개 있습니다. 아무것도 쓰지 않았습니다.`,
        ...save.conflicts
          .slice(0, 5)
          .map((conflict) => `- ${conflict.table}/${conflict.key}: 시트 "${conflict.ours}" / 게임 "${conflict.theirs}"`),
        ...(save.conflicts.length > 5 ? [`- 외 ${save.conflicts.length - 5}개`] : []),
      ],
    };
  }
  const notes = [
    save.kept_in_game_entries > 0 ? `게임 안에서 고친 ${save.kept_in_game_entries}개는 유지` : "",
    save.unsupported_entries > 0 ? `Translator 미지원 ${save.unsupported_entries}개` : "",
    save.disabled_patch_mods.length > 0 ? `기존 번역 모드 비활성화: ${save.disabled_patch_mods.join(", ")}` : "",
    !save.translator_active ? "Translator가 비활성이라 게임에 반영되지 않음" : save.game_running ? "게임 안 Translator 패널에서 Reload 필요" : "",
  ].filter(Boolean);
  return {
    blocked: false,
    message: `Translator 저장 완료: ${save.written_entries}개 변경${notes.length > 0 ? ` · ${notes.join(" · ")}` : ""}`,
    details: [
      `Translator 저장 완료: ${save.mod_id}/${save.language} ${save.written_entries}개 변경, ${save.unchanged_entries}개 그대로 (${save.overrides_dir})`,
      ...notes.map((note) => `- ${note}`),
    ],
  };
}
