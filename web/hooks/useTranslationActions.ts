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

type ValidationOverrideAction = "apply" | "export";

type ValidationOverride = {
  action: ValidationOverrideAction;
  sheetPath: string;
  targetLanguage: string;
  sheetFingerprint: string;
  validationSignature: string;
};

export function useTranslationActions(params: TranslationActionsParams) {
  const runJsonTool = useJsonToolRunner(params);
  const sheetActions = useTranslationSheetActions(params);
  const ioActions = useTranslationIoActions(params);
  const compareActions = useTranslationCompareActions(params);
  const sessionActions = useTranslationSessionActions(params);
  const validationRequestId = React.useRef(0);
  const validationOverrideRef = React.useRef<ValidationOverride | null>(null);
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

  async function recalculateTranslationSheet() {
    const sheetPath = params.jsonOutputSheet || params.jsonExistingSheet;
    if (!sheetPath) {
      params.appendLog("재계산할 번역 시트 경로가 없습니다.");
      return false;
    }
    return runJsonTool("recalculate_json_translation_sheet", {
      sourcePath: params.jsonSource,
      currentSheetPath: sheetPath,
      outputPath: params.jsonOutputSheet || sheetPath,
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

  async function confirmPckValidation(action: ValidationOverrideAction, actionLabel: string): Promise<boolean> {
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
    const targetLanguage = params.jsonTargetLanguage || params.jsonSheet?.target_language || "";
    const sheetFingerprint = translationSheetFingerprint(params.jsonSheet);
    const override = validationOverrideRef.current;
    if (
      override &&
      override.action === action &&
      override.sheetPath === sheetPath &&
      override.targetLanguage === targetLanguage &&
      override.sheetFingerprint === sheetFingerprint
    ) {
      validationOverrideRef.current = null;
      params.appendLog(`확인된 검증 경고로 ${actionLabel}을 강행합니다. 경고 ${override.validationSignature}`);
      return true;
    }
    params.setBusy("validate_json_translation_sheet");
    params.setJsonToolError("");
    try {
      const result = await invokeCommand<JsonValidation>("validate_json_translation_sheet", {
        sheetPath,
      });
      params.setJsonValidation(result);
      if (result.valid) {
        validationOverrideRef.current = null;
        params.appendLog(`${actionLabel} 전 검증 통과`);
        return true;
      }
      const formatIssues = result.format_issues?.length ?? 0;
      const validationSignature = validationWarningSignature(result);
      const message = [
        "번역 검증에서 수정 필요한 항목이 발견되었습니다.",
        "",
        `빈 값: ${result.missing_entries.length}`,
        `원본 변경: ${result.updated_entries.length}`,
        `삭제됨: ${result.removed_entries.length}`,
        `태그/줄바꿈 구조 경고: ${formatIssues}`,
        "",
        `그래도 ${actionLabel}을 준비할까요?`,
        "확인 후 같은 버튼을 한 번 더 누르면 경고가 남아 있어도 계속합니다.",
      ].join("\n");
      const confirmed = window.confirm(message);
      if (confirmed) {
        validationOverrideRef.current = {
          action,
          sheetPath,
          targetLanguage,
          sheetFingerprint,
          validationSignature,
        };
        params.appendLog(`검증 경고를 확인했습니다. ${actionLabel}을 한 번 더 누르면 계속합니다. ${validationSignature}`);
      } else {
        validationOverrideRef.current = null;
        params.appendLog(`검증 경고로 ${actionLabel}을 취소했습니다.`);
      }
      return false;
    } catch (error) {
      const message = `${actionLabel} 전 검증 실패: ${formatError(error)}`;
      params.setJsonToolError(message);
      params.appendLog(message);
      const confirmed = window.confirm(`${message}\n\n그래도 ${actionLabel}을 준비할까요?\n확인 후 같은 버튼을 한 번 더 누르면 검증 실패 상태에서도 계속합니다.`);
      if (confirmed) {
        validationOverrideRef.current = {
          action,
          sheetPath,
          targetLanguage,
          sheetFingerprint,
          validationSignature: "검증 실패",
        };
        params.appendLog(`검증 실패를 확인했습니다. ${actionLabel}을 한 번 더 누르면 계속합니다.`);
      } else {
        validationOverrideRef.current = null;
      }
      return false;
    } finally {
      params.setBusy(null);
    }
  }

  async function applyTranslationSheet(): Promise<boolean> {
    const saved = await ioActions.saveEditedTranslationSheet();
    if (!saved) {
      return false;
    }
    const confirmed = await confirmPckValidation("apply", "번역 적용");
    if (!confirmed) {
      return false;
    }
    const applied = await runJsonTool("apply_json_translation_sheet", {
      sheetPath: params.jsonOutputSheet || params.jsonExistingSheet,
      outputPath: params.jsonTranslatedOutput,
      pckTargetPath: params.jsonPckTargetPath || null,
    });
    if (applied) {
      await ioActions.saveEditedTranslationSheet({ finalizeStatuses: true });
      await params.load();
      params.appendLog("적용 후 모드 목록을 자동 새로고침했습니다.");
      sessionActions.closeTranslationSession();
    }
    return applied;
  }

  async function exportTranslationPatchMod() {
    const canExportPatchMod = params.translationProject?.canExportPatchMod ?? Boolean(params.jsonPckTargetPath);
    if (!canExportPatchMod) {
      params.appendLog("PCK 기반 작업이 아니어서 번역 모드로 내보낼 수 없습니다. 번역 저장/적용을 사용하세요.");
      return;
    }
    const saved = await ioActions.saveEditedTranslationSheet();
    if (!saved) {
      return;
    }
    const confirmed = await confirmPckValidation("export", "번역 모드 내보내기");
    if (!confirmed) {
      return;
    }
    const exported = await ioActions.exportTranslationPatchMod({ skipSave: true });
    if (exported) {
      await ioActions.saveEditedTranslationSheet({ finalizeStatuses: true });
      await params.load();
      params.appendLog("내보내기 후 모드 목록을 자동 새로고침했습니다.");
      sessionActions.closeTranslationSession();
    }
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
    recalculateTranslationSheet,
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

function validationWarningSignature(validation: JsonValidation) {
  return [
    `빈 값 ${validation.missing_entries.length}`,
    `원본 변경 ${validation.updated_entries.length}`,
    `삭제됨 ${validation.removed_entries.length}`,
    `구조 경고 ${validation.format_issues?.length ?? 0}`,
  ].join(" / ");
}

function translationSheetFingerprint(sheet: TranslationActionsParams["jsonSheet"]) {
  if (!sheet) {
    return "";
  }
  return JSON.stringify({
    source_path: sheet.source_path,
    target_language: sheet.target_language,
    entries: sheet.entries.map((entry) => [
      entry.key,
      entry.slot_id ?? "",
      entry.source_value,
      entry.translated_value,
      entry.status,
    ]),
  });
}
