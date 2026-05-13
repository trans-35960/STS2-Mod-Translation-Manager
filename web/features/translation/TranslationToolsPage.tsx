import { Loader2 } from "lucide-react";
import { TranslationActionsPanel, TranslationWorkHeader } from "./TranslationActionsPanel";
import { TranslationSheetTable } from "./TranslationSheetTable";
import { TranslationPage } from "./TranslationStartPage";
import type { TranslationToolsPageProps } from "./TranslationToolsTypes";
import { useTranslationToolsUiState } from "./useTranslationToolsUiState";
import {
  compactTranslationFile,
  createCompareValueMap,
  hasTranslationValue,
  inferPckTargetPath,
  isTabularTranslationPaste,
  isTranslatableEntry,
  isTranslationSlotId,
  languageCodeFromSheetKey,
  languageFolderCode,
  looksLikeJsonPaste,
  parsePastedTranslationJson,
  replaceLocalizationLanguageInPath,
  retargetTranslationSheetPath,
  splitSheetKey,
  stripFileNameFromPckTarget,
  structuredTranslationEntries,
  translationSlotEntries,
  translationSlotKey,
} from "./translationUtils";

function TranslationToolsPage(props: TranslationToolsPageProps) {
  const ui = useTranslationToolsUiState(props);
  const applyingPck =
    props.busy === "apply_json_translation_sheet" ||
    props.busy === "export_translation_patch_mod" ||
    props.busy === "save_json_translation_sheet";

  async function validateAndShowWarnings() {
    await props.onValidate();
    ui.showEntryCondition("warning");
  }

  return (
    <div className="translation-tool-page">
      <TranslationWorkHeader
        props={{ ...props, onValidate: validateAndShowWarnings }}
        sheetPath={ui.sheetPath}
        filledTranslations={ui.filledTranslations}
      />
      <div className="tool-layout">
        <TranslationActionsPanel
          props={props}
          sheetPath={ui.sheetPath}
          projectTree={ui.projectTree}
          activeProjectPath={ui.activeProjectPath}
          setActiveProjectPath={ui.setActiveProjectPath}
          sourceLanguageOptions={ui.sourceLanguageOptions}
          selectedSourceLanguage={ui.selectedSourceLanguage}
          targetLanguageOptions={ui.targetLanguageOptions}
          compareLanguageOptions={ui.compareLanguageOptions}
          selectedCompareLanguages={ui.selectedCompareLanguages}
          exportOnlyEmpty={ui.exportOnlyEmpty}
          setExportOnlyEmpty={ui.setExportOnlyEmpty}
          filledTranslations={ui.filledTranslations}
          copyTreeJson={ui.copyTreeJson}
        />
        <TranslationSheetTable
          props={props}
          sheetStats={ui.sheetStats}
          activeProjectPath={ui.activeProjectPath}
          entryFilter={ui.entryFilter}
          setEntryFilter={ui.setEntryFilter}
          entrySearch={ui.entrySearch}
          setEntrySearch={ui.setEntrySearch}
          replaceSearch={ui.replaceSearch}
          setReplaceSearch={ui.setReplaceSearch}
          replaceWith={ui.replaceWith}
          setReplaceWith={ui.setReplaceWith}
          showIds={ui.showIds}
          setShowIds={ui.setShowIds}
          columns={ui.columns}
          sheetSourceLanguage={ui.sheetSourceLanguage}
          selectedCompareLanguages={ui.selectedCompareLanguages}
          pasteCandidateCount={ui.pasteCandidateCount}
          validationWarningCount={ui.validationWarningCount}
          validationIssueKindFilter={ui.validationIssueKindFilter}
          showCompareColumn={ui.showCompareColumn}
          slotIdByEntryKey={ui.slotIdByEntryKey}
          filteredEntries={ui.filteredEntries}
          visibleEntries={ui.visibleEntries}
          replaceMatchCount={ui.replaceMatchCount}
          startColumnResize={ui.startColumnResize}
          expandVisibleEntriesOnScroll={ui.expandVisibleEntriesOnScroll}
          loadMoreVisibleEntries={ui.loadMoreVisibleEntries}
          showEntryCondition={ui.showEntryCondition}
          showValidationIssueKind={ui.showValidationIssueKind}
          replaceTranslatedValues={ui.replaceTranslatedValues}
          focusEntryKey={ui.focusEntryKey}
          revealEntryKey={ui.revealEntryKey}
          clearFocusEntryKey={ui.clearFocusEntryKey}
        />
      </div>
      {applyingPck && (
        <div className="pck-apply-progress" role="status" aria-live="polite">
          <div>
            <Loader2 size={28} />
            <strong>{props.busy === "save_json_translation_sheet" ? "번역 시트 저장 중" : props.busy === "export_translation_patch_mod" ? "번역 모드 내보내는 중" : "번역 파일 반영 중"}</strong>
            <span>{props.busy === "export_translation_patch_mod" ? "json+pck 패치 모드를 만드는 중입니다." : "적용이 끝나면 모드 관리에서 해당 모드로 이동합니다."}</span>
          </div>
        </div>
      )}
    </div>
  );
}

export {
  TranslationPage,
  TranslationToolsPage,
  createCompareValueMap,
  parsePastedTranslationJson,
  looksLikeJsonPaste,
  isTabularTranslationPaste,
  structuredTranslationEntries,
  translationSlotEntries,
  translationSlotKey,
  compactTranslationFile,
  isTranslationSlotId,
  splitSheetKey,
  languageCodeFromSheetKey,
  languageFolderCode,
  hasTranslationValue,
  isTranslatableEntry,
  retargetTranslationSheetPath,
  inferPckTargetPath,
  replaceLocalizationLanguageInPath,
  stripFileNameFromPckTarget,
};
