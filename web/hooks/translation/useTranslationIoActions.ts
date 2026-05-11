import { invokeCommand, openDialog, saveDialog } from "../../api/tauri";
import {
  hasTranslationValue,
  inferPckTargetPath,
  isTranslatableEntry,
  pathMatchesProjectNode,
  splitSheetKey,
  translationLanguagesMatch,
} from "../../features/translation/translationUtils";
import type {
  JsonCsvExport,
  JsonSheetAction,
  ShortJsonExport,
  TranslationPatchExport,
} from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import type { TranslationActionsParams } from "./types";

type ShortJsonExportOptions = boolean | {
  onlyEmpty?: boolean;
  scopePath?: string;
  warningOnly?: boolean;
};

export function useTranslationIoActions({
  appendLog,
  dashboard,
  jsonExistingSheet,
  jsonOutputSheet,
  jsonSheet,
  jsonTargetLanguage,
  jsonValidation,
  translationProject,
  setBusy,
  setCompareValuesByLanguage,
  setJsonApplyResult,
  setJsonPckTargetPath,
  setJsonReport,
  setJsonSheet,
  setJsonValidation,
  setPasteCandidatesByKey,
}: TranslationActionsParams) {
  function ensureCurrentSheetLanguage(action: string): boolean {
    if (!jsonSheet || !jsonTargetLanguage || translationLanguagesMatch(jsonSheet.target_language, jsonTargetLanguage)) {
      return true;
    }
    appendLog(`현재 시트는 ${jsonSheet.target_language}용입니다. ${jsonTargetLanguage} 작업은 별도 시트를 생성/불러온 뒤 ${action}하세요.`);
    return false;
  }

  async function saveEditedTranslationSheet() {
    if (!jsonSheet) {
      return false;
    }
    if (!ensureCurrentSheetLanguage("저장")) {
      return false;
    }
    const sheetPath = jsonOutputSheet || jsonExistingSheet;
    if (!sheetPath) {
      appendLog("저장할 번역 시트 경로가 없습니다.");
      return false;
    }
    setBusy("save_json_translation_sheet");
    try {
      if (isPreviewRuntime()) {
        appendLog("Preview action: save_json_translation_sheet");
        return true;
      }
      const result = await invokeCommand<JsonSheetAction>("save_json_translation_sheet", {
        sheetPath,
        sheet: jsonSheet,
      });
      setJsonSheet(result.sheet);
      setJsonReport(result.report);
      setJsonPckTargetPath(inferPckTargetPath(result.sheet));
      setJsonValidation(null);
      setJsonApplyResult(null);
      setCompareValuesByLanguage({});
      setPasteCandidatesByKey({});
      appendLog(result.message);
      return true;
    } catch (error) {
      appendLog(String(error));
      return false;
    } finally {
      setBusy(null);
    }
  }

  async function exportTranslationCsv() {
    if (!jsonSheet) {
      return;
    }
    if (!ensureCurrentSheetLanguage("내보내기")) {
      return;
    }
    const defaultName = `${translationProject?.modName || "translation"}.${jsonSheet.target_language || "kor"}.csv`;
    try {
      const outputPath = isPreviewRuntime()
        ? `${dashboard?.paths.translation_work ?? "translation_work"}/${defaultName}`
        : await saveDialog({
            title: "번역 CSV 내보내기",
            defaultPath: defaultName,
            filters: [{ name: "CSV", extensions: ["csv"] }],
          });
      if (!outputPath || Array.isArray(outputPath)) {
        return;
      }
      setBusy("export_json_translation_csv");
      if (isPreviewRuntime()) {
        appendLog(`Preview CSV export: ${outputPath}`);
        return;
      }
      const result = await invokeCommand<JsonCsvExport>("export_json_translation_csv", {
        outputPath,
        sheet: jsonSheet,
      });
      appendLog(`CSV 내보내기 완료: ${result.rows}행 (${result.output_path})`);
    } catch (error) {
      appendLog(String(error));
    } finally {
      setBusy(null);
    }
  }

  async function exportTranslationShortJson(options: ShortJsonExportOptions) {
    if (!jsonSheet) {
      return;
    }
    if (!ensureCurrentSheetLanguage("내보내기")) {
      return;
    }
    const normalizedOptions = typeof options === "boolean" ? { onlyEmpty: options } : options;
    const onlyEmpty = Boolean(normalizedOptions.onlyEmpty);
    const warningKeys = new Set([
      ...(jsonValidation?.missing_entries ?? []),
      ...(jsonValidation?.updated_entries ?? []),
      ...(jsonValidation?.format_issues ?? []).map((issue) => issue.key),
    ]);
    if (onlyEmpty && !jsonSheet.entries.some((entry) => isTranslatableEntry(entry) && !hasTranslationValue(entry.translated_value))) {
      appendLog("빈 번역 항목이 없어 번역용 JSON을 내보내지 않았습니다.");
      return;
    }
    if (normalizedOptions.warningOnly && warningKeys.size === 0) {
      appendLog("검증 오류 항목이 없어 오류 JSON을 내보내지 않았습니다. 먼저 검증을 실행해 주세요.");
      return;
    }
    const includeKeys = jsonSheet.entries
      .filter((entry) => {
        if (normalizedOptions.scopePath && !pathMatchesProjectNode(splitSheetKey(entry.key).file, normalizedOptions.scopePath)) {
          return false;
        }
        if (normalizedOptions.warningOnly && !warningKeys.has(entry.key)) {
          return false;
        }
        return true;
      })
      .map((entry) => entry.key);
    if (includeKeys.length === 0) {
      appendLog("내보낼 항목이 없습니다.");
      return;
    }
    const shouldLimitKeys = Boolean(normalizedOptions.scopePath || normalizedOptions.warningOnly);
    const suffix = [
      normalizedOptions.scopePath ? safeExportName(normalizedOptions.scopePath) : "",
      normalizedOptions.warningOnly ? "warning" : "",
      onlyEmpty ? "empty" : "",
    ].filter(Boolean).join(".");
    const defaultName = `${translationProject?.modName || "translation"}.${jsonSheet.target_language || "kor"}${suffix ? `.${suffix}` : ""}.short.json`;
    try {
      const outputPath = isPreviewRuntime()
        ? `${dashboard?.paths.translation_work ?? "translation_work"}/${defaultName}`
        : await saveDialog({
            title: normalizedOptions.warningOnly ? "검증 오류 JSON 내보내기" : "번역용 JSON 내보내기",
            defaultPath: defaultName,
            filters: [{ name: "JSON", extensions: ["json"] }],
          });
      if (!outputPath || Array.isArray(outputPath)) {
        return;
      }
      setBusy("export_json_translation_short_json");
      if (isPreviewRuntime()) {
        appendLog(normalizedOptions.warningOnly ? `Preview warning JSON export: ${outputPath}` : `Preview short JSON export: ${outputPath}`);
        return;
      }
      const result = normalizedOptions.warningOnly
        ? await invokeCommand<ShortJsonExport>("export_json_translation_warning_json", {
            outputPath,
            sheet: jsonSheet,
            includeKeys: shouldLimitKeys ? includeKeys : undefined,
          })
        : await invokeCommand<ShortJsonExport>("export_json_translation_short_json", {
            outputPath,
            sheet: jsonSheet,
            onlyEmpty,
            includeKeys: shouldLimitKeys ? includeKeys : undefined,
          });
      appendLog(`${normalizedOptions.warningOnly ? "검증 오류 JSON" : "번역용 JSON"} 내보내기 완료: ${result.rows}행 (${result.output_path})`);
    } catch (error) {
      appendLog(String(error));
    } finally {
      setBusy(null);
    }
  }

  async function importTranslationValues() {
    if (!jsonSheet) {
      return;
    }
    if (!ensureCurrentSheetLanguage("불러오기")) {
      return;
    }
    try {
      const inputPath = isPreviewRuntime()
        ? `${dashboard?.paths.translation_work ?? "translation_work"}/translated.csv`
        : await openDialog({
            title: "번역 CSV/JSON 불러오기",
            directory: false,
            multiple: false,
            filters: [
              { name: "Translation", extensions: ["csv", "json"] },
              { name: "CSV", extensions: ["csv"] },
              { name: "JSON", extensions: ["json"] },
            ],
          });
      if (!inputPath || Array.isArray(inputPath)) {
        return;
      }
      setBusy("import_json_translation_values");
      if (isPreviewRuntime()) {
        appendLog(`Preview translation import: ${inputPath}`);
        return;
      }
      const result = await invokeCommand<JsonSheetAction>("import_json_translation_values", {
        inputPath,
        sheet: jsonSheet,
      });
      setJsonSheet(result.sheet);
      setJsonReport(result.report);
      setJsonValidation(null);
      setJsonApplyResult(null);
      setPasteCandidatesByKey({});
      appendLog(result.message);
    } catch (error) {
      appendLog(String(error));
    } finally {
      setBusy(null);
    }
  }

  async function exportTranslationPatchMod(options?: { skipSave?: boolean }) {
    if (!jsonSheet) {
      return;
    }
    if (!ensureCurrentSheetLanguage("내보내기")) {
      return;
    }
    const sheetPath = jsonOutputSheet || jsonExistingSheet;
    if (!sheetPath) {
      appendLog("내보낼 번역 시트 경로가 없습니다.");
      return;
    }
    if (!options?.skipSave) {
      const saved = await saveEditedTranslationSheet();
      if (!saved) {
        return;
      }
    }
    try {
      const outputDir = isPreviewRuntime()
        ? `${dashboard?.paths.translation_work ?? "translation_work"}/patch_mods`
        : await openDialog({
            title: "번역 모드 내보낼 폴더 선택",
            directory: true,
            multiple: false,
            defaultPath: dashboard?.paths.game_mods || dashboard?.paths.translation_work,
          });
      if (!outputDir || Array.isArray(outputDir)) {
        return;
      }
      setBusy("export_translation_patch_mod");
      if (isPreviewRuntime()) {
        appendLog(`Preview translation patch export: ${outputDir}`);
        return;
      }
      const result = await invokeCommand<TranslationPatchExport>("export_translation_patch_mod", {
        sheetPath,
        outputDir,
      });
      setJsonApplyResult({
        output_path: result.output_dir,
        applied_entries: result.applied_entries,
        language_output_path: result.output_dir,
        packed_pck_path: result.pck_path,
        installed_mod_path: "",
        message: `번역 모드 내보내기 완료: ${result.package_id}`,
      });
      appendLog(
        `번역 모드 내보내기 완료: ${result.package_id} / ${result.files}개 JSON / 언어 ${result.languages.join(", ") || "-"} (${result.output_dir})`,
      );
    } catch (error) {
      appendLog(String(error));
    } finally {
      setBusy(null);
    }
  }

  return {
    exportTranslationCsv,
    exportTranslationPatchMod,
    exportTranslationShortJson,
    importTranslationValues,
    saveEditedTranslationSheet,
  };
}

function safeExportName(value: string) {
  return value
    .replace(/\\/g, "/")
    .split("/")
    .filter(Boolean)
    .slice(-2)
    .join("-")
    .replace(/[^A-Za-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    || "selection";
}
