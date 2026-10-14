import React from "react";
import { Loader2 } from "lucide-react";
import type { PromptFocus } from "./AiPromptModal";
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
  const [aiPromptFocus, setAiPromptFocus] = React.useState<PromptFocus | null>(null);
  const applyingPck =
    props.busy === "apply_json_translation_sheet" ||
    props.busy === "export_translation_patch_mod" ||
    props.busy === "save_json_translation_sheet";

  async function validateAndShowWarnings() {
    await props.onValidate();
    ui.showEntryCondition("warning");
  }

  async function closeWithConfirmation() {
    // Pending edits are saved first; ask only when that fails.
    if (props.unsavedChanges > 0 && !(await props.onFlushSave())) {
      if (!window.confirm(`변경 ${props.unsavedChanges}개를 저장하지 못했습니다. 저장하지 않고 닫을까요?`)) {
        return;
      }
    }
    props.onCloseSession();
  }

  function showHistoryMove(move: ReturnType<typeof props.onUndo>) {
    if (move?.changedKeys[0]) {
      ui.showChangedEntry(move.changedKeys[0]);
    }
  }

  const undo = () => showHistoryMove(props.onUndo());
  const redo = () => showHistoryMove(props.onRedo());
  function revertToStart() {
    if (window.confirm("이번 작업을 시작할 때 상태로 시트를 되돌릴까요? (되돌리기로 다시 취소할 수 있습니다)")) {
      showHistoryMove(props.onRevertToStart());
    }
  }

  // Ctrl+S saves, Ctrl+F jumps to the sheet search. Ctrl+Z / Ctrl+Y undo and
  // redo sheet edits, except inside text fields (which undo their own typing)
  // and while a dialog is open.
  const shortcutState = React.useRef({ props, canSave: false, undo, redo });
  shortcutState.current = { props, canSave: Boolean(props.sheet && ui.sheetPath && !props.busy), undo, redo };
  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey) {
        return;
      }
      const key = event.key.toLowerCase();
      if (key === "z" || key === "y") {
        if (isTextField(event.target) || document.querySelector(".modal-backdrop")) {
          return;
        }
        event.preventDefault();
        if (key === "y" || event.shiftKey) {
          shortcutState.current.redo();
        } else {
          shortcutState.current.undo();
        }
        return;
      }
      if (event.shiftKey) {
        return;
      }
      if (key === "s") {
        event.preventDefault();
        if (shortcutState.current.canSave) {
          shortcutState.current.props.onSave();
        }
      } else if (key === "f") {
        const search = document.getElementById("translation-entry-search") as HTMLInputElement | null;
        if (search) {
          event.preventDefault();
          search.focus();
          search.select();
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="translation-tool-page">
      <TranslationWorkHeader
        props={{ ...props, onValidate: validateAndShowWarnings, onCloseSession: () => void closeWithConfirmation() }}
        onUndo={undo}
        onRedo={redo}
        onRevertToStart={revertToStart}
        sheetPath={ui.sheetPath}
        filledTranslations={ui.filledTranslations}
        onRevealEntry={ui.revealEntryKey}
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
          dllTextKeys={ui.dllTextKeys}
          filledTranslations={ui.filledTranslations}
          copyTreeJson={ui.copyTreeJson}
          aiPromptFocus={aiPromptFocus}
          setAiPromptFocus={setAiPromptFocus}
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
          dllTextOnly={ui.dllTextOnly}
          setDllTextOnly={ui.setDllTextOnly}
          dllTextCount={ui.dllTextCount}
          dllAnalyzedAt={ui.dllUses?.analyzed_at ?? null}
          dllSeenKeys={ui.dllSeenKeys}
          dllPartialKeys={ui.dllPartialKeys}
          columns={ui.columns}
          sheetSourceLanguage={ui.sheetSourceLanguage}
          selectedCompareLanguages={ui.selectedCompareLanguages}
          pasteCandidateCount={ui.pasteCandidateCount}
          validationWarningCount={ui.validationWarningCount}
          validationIssueKindFilter={ui.validationIssueKindFilter}
          showCompareColumn={ui.showCompareColumn}
          slotIdByEntryKey={ui.slotIdByEntryKey}
          filteredEntries={ui.filteredEntries}
          replaceMatchCount={ui.replaceMatchCount}
          startColumnResize={ui.startColumnResize}
          showEntryCondition={ui.showEntryCondition}
          showValidationIssueKind={ui.showValidationIssueKind}
          replaceTranslatedValues={ui.replaceTranslatedValues}
          focusEntryKey={ui.focusEntryKey}
          focusRequest={ui.focusRequest}
          revealWarningEntry={ui.revealWarningEntry}
          focusEntryInput={ui.focusEntryInput}
          revealEntryKey={ui.revealEntryKey}
          clearFocusEntryKey={ui.clearFocusEntryKey}
          onOpenFixPrompt={() => setAiPromptFocus("warnings")}
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

function isTextField(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || target.matches("input:not([type=checkbox]):not([type=radio]):not([type=button]), textarea, select"))
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
