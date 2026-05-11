import React from "react";
import { invokeCommand } from "../api/tauri";
import { translationLanguagesMatch } from "../features/translation/translationUtils";
import type { JsonValidation } from "../types";
import { formatError } from "../utils/logging";
import { isPreviewRuntime } from "../utils/runtime";
import { useJsonToolRunner } from "./translation/useJsonToolRunner";
import { useTranslationCompareActions } from "./translation/useTranslationCompareActions";
import { useTranslationIoActions } from "./translation/useTranslationIoActions";
import { useTranslationSessionActions } from "./translation/useTranslationSessionActions";
import { useTranslationSheetActions } from "./translation/useTranslationSheetActions";
import type { TranslationActionsParams } from "./translation/types";

export function useTranslationActions(params: TranslationActionsParams) {
  const runJsonTool = useJsonToolRunner(params);
  const sheetActions = useTranslationSheetActions(params);
  const ioActions = useTranslationIoActions(params);
  const compareActions = useTranslationCompareActions(params);
  const sessionActions = useTranslationSessionActions(params);
  const validationRequestId = React.useRef(0);
  const hasValidation = Boolean(params.jsonValidation);

  async function validateCurrentTranslationSheet(options: { busy?: boolean; log?: boolean } = {}) {
    if (!params.jsonSheet) {
      return null;
    }
    const requestId = ++validationRequestId.current;
    if (options.busy) {
      params.setBusy("validate_json_translation_sheet_data");
      params.setJsonToolError("");
    }
    try {
      const result = await invokeCommand<JsonValidation>("validate_json_translation_sheet_data", {
        sheet: params.jsonSheet,
      });
      if (requestId === validationRequestId.current) {
        params.setJsonValidation(result);
      }
      if (options.log) {
        const formatIssues = result.format_issues?.length ?? 0;
        params.appendLog(
          result.valid
            ? "현재 편집 내용 검증 통과"
            : `현재 편집 내용 검증 완료: 빈 값 ${result.missing_entries.length} / 원본 변경 ${result.updated_entries.length} / 구조 경고 ${formatIssues}`,
        );
      }
      return result;
    } catch (error) {
      if (requestId === validationRequestId.current && options.busy) {
        const message = `현재 편집 내용 검증 실패: ${formatError(error)}`;
        params.setJsonToolError(message);
        params.appendLog(message);
      }
      return null;
    } finally {
      if (options.busy) {
        params.setBusy(null);
      }
    }
  }

  React.useEffect(() => {
    if (isPreviewRuntime() || params.page !== "translationTools" || !params.jsonSheet || !hasValidation) {
      return;
    }
    const timeoutId = window.setTimeout(() => {
      void validateCurrentTranslationSheet();
    }, 450);
    return () => window.clearTimeout(timeoutId);
  }, [params.page, params.jsonSheet, hasValidation]);

  async function createTranslationSheet() {
    return runJsonTool("create_json_translation_sheet", {
      sourcePath: params.jsonSource,
      existingSheetPath: params.jsonExistingSheet || null,
      outputPath: params.jsonOutputSheet || null,
      targetLanguage: params.jsonTargetLanguage || params.settingsDraft?.target_language || params.dashboard?.settings.target_language,
    });
  }

  async function loadTranslationSheet() {
    return runJsonTool("load_json_translation_sheet", {
      sheetPath: params.jsonExistingSheet || params.jsonOutputSheet,
    });
  }

  async function validateTranslationSheet() {
    if (params.jsonSheet && !isPreviewRuntime()) {
      return Boolean(await validateCurrentTranslationSheet({ busy: true, log: true }));
    }
    return runJsonTool("validate_json_translation_sheet", {
      sheetPath: params.jsonOutputSheet || params.jsonExistingSheet,
    });
  }

  async function confirmPckValidation(): Promise<boolean> {
    if (
      params.jsonSheet &&
      params.jsonTargetLanguage &&
      !translationLanguagesMatch(params.jsonSheet.target_language, params.jsonTargetLanguage)
    ) {
      params.appendLog(
        `현재 시트는 ${params.jsonSheet.target_language}용입니다. ${params.jsonTargetLanguage} 작업은 별도 시트를 생성/불러온 뒤 적용하세요.`,
      );
      return false;
    }
    const sheetPath = params.jsonOutputSheet || params.jsonExistingSheet;
    if (!sheetPath) {
      params.appendLog("검증할 번역 시트 경로가 없습니다.");
      return false;
    }
    params.setBusy("validate_json_translation_sheet");
    params.setJsonToolError("");
    try {
      const result = await invokeCommand<JsonValidation>("validate_json_translation_sheet", {
        sheetPath,
      });
      params.setJsonValidation(result);
      if (result.valid) {
        params.appendLog("번역 적용 전 검증 통과");
        return true;
      }
      const formatIssues = result.format_issues?.length ?? 0;
      const message = [
        "번역 검증에서 수정 필요한 항목이 발견되었습니다.",
        "",
        `빈 값: ${result.missing_entries.length}`,
        `원본 변경: ${result.updated_entries.length}`,
        `삭제됨: ${result.removed_entries.length}`,
        `태그/줄바꿈 구조 경고: ${formatIssues}`,
        "",
        "그래도 번역 적용을 계속할까요?",
      ].join("\n");
      const confirmed = window.confirm(message);
      params.appendLog(
        confirmed
          ? `검증 경고를 확인하고 번역 적용을 계속합니다. 구조 경고 ${formatIssues}개`
          : "검증 경고로 번역 적용을 취소했습니다.",
      );
      return confirmed;
    } catch (error) {
      const message = `번역 적용 전 검증 실패: ${formatError(error)}`;
      params.setJsonToolError(message);
      params.appendLog(message);
      return window.confirm(`${message}\n\n그래도 번역 적용을 계속할까요?`);
    } finally {
      params.setBusy(null);
    }
  }

  async function applyTranslationSheet(): Promise<boolean> {
    const saved = await ioActions.saveEditedTranslationSheet();
    if (!saved) {
      return false;
    }
    const confirmed = await confirmPckValidation();
    if (!confirmed) {
      return false;
    }
    const applied = await runJsonTool("apply_json_translation_sheet", {
      sheetPath: params.jsonOutputSheet || params.jsonExistingSheet,
      outputPath: params.jsonTranslatedOutput,
      pckTargetPath: params.jsonPckTargetPath || null,
    });
    if (applied) {
      await params.load();
      params.appendLog("적용 후 모드 목록을 자동 새로고침했습니다.");
    }
    return applied;
  }

  async function exportTranslationPatchMod() {
    const saved = await ioActions.saveEditedTranslationSheet();
    if (!saved) {
      return;
    }
    const confirmed = await confirmPckValidation();
    if (!confirmed) {
      return;
    }
    await ioActions.exportTranslationPatchMod({ skipSave: true });
  }

  return {
    applyAllPasteCandidates: sheetActions.applyAllPasteCandidates,
    applyPasteCandidate: sheetActions.applyPasteCandidate,
    applyTranslationSheet,
    closeTranslationSession: sessionActions.closeTranslationSession,
    copySelectedTranslations: sheetActions.copySelectedTranslations,
    createTranslationSheet,
    dismissAllPasteCandidates: sheetActions.dismissAllPasteCandidates,
    dismissPasteCandidate: sheetActions.dismissPasteCandidate,
    exportTranslationCsv: ioActions.exportTranslationCsv,
    exportTranslationPatchMod,
    exportTranslationShortJson: ioActions.exportTranslationShortJson,
    extractTreeNode: sessionActions.extractTreeNode,
    importTranslationValues: ioActions.importTranslationValues,
    loadTranslationSheet,
    openModLanguageInTranslationTools: sessionActions.openModLanguageInTranslationTools,
    openTreeNodeInTranslationTools: sessionActions.openTreeNodeInTranslationTools,
    pasteStructuredTranslationJson: sheetActions.pasteStructuredTranslationJson,
    pasteTranslationValues: sheetActions.pasteTranslationValues,
    replaceTranslationEntries: sheetActions.replaceTranslationEntries,
    saveEditedTranslationSheet: ioActions.saveEditedTranslationSheet,
    selectTranslationRow: sheetActions.selectTranslationRow,
    setTargetLanguage: sessionActions.setTargetLanguage,
    switchTranslationSourceLanguage: sessionActions.switchTranslationSourceLanguage,
    toggleCompareLanguage: compareActions.toggleCompareLanguage,
    updateTranslationEntry: sheetActions.updateTranslationEntry,
    validateTranslationSheet,
  };
}
