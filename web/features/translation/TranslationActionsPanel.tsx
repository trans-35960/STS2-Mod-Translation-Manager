import React from "react";
import { AiPromptModal } from "./AiPromptModal";
import { GlossaryPanel } from "./GlossaryPanel";
import {
  AlertTriangle,
  Bot,
  MessageSquareText,
  CheckCircle2,
  FileDown,
  FileJson,
  FileUp,
  FolderOpen,
  History,
  Loader2,
  Redo2,
  Undo2,
  Package,
  RefreshCw,
  Save,
  UploadCloud,
  X,
} from "lucide-react";
import type { LanguagePreview } from "../../types";
import {
  languageKeyCount,
  parentPath,
  recommendedSourceLanguage,
} from "../mods/modUtils";
import type { TranslationToolsPageProps } from "./TranslationToolsTypes";
import { TranslationProjectTree } from "./TranslationProjectTree";
import { ProjectSummary } from "./TranslationWidgets";
import { SaveTargetSelect } from "./SaveTargetSelect";
import {
  languageFolderCode,
  projectNameFromPath,
  translationLanguagesMatch,
  translationProjectKey,
  type TranslationProjectNode,
} from "./translationUtils";

type TargetLanguageOption = {
  code: string;
  label: string;
};

function SaveStateBadge({ props }: { props: TranslationToolsPageProps }) {
  if (props.saveState === "error") {
    return (
      <em className="save-state error" title={props.saveError}>
        자동 저장 실패{props.unsavedChanges > 0 ? ` · ${props.unsavedChanges}개 변경` : ""}
      </em>
    );
  }
  if (props.saveState === "saving") {
    return <em className="save-state pending">저장 중…</em>;
  }
  if (props.unsavedChanges > 0) {
    return <em className="save-state pending">{props.unsavedChanges}개 변경 · 곧 자동 저장</em>;
  }
  return <em className="save-state">자동 저장됨</em>;
}

export function TranslationWorkHeader({
  props,
  sheetPath,
  filledTranslations,
  onUndo,
  onRedo,
  onRevertToStart,
}: {
  props: TranslationToolsPageProps;
  sheetPath: string;
  filledTranslations: number;
  onUndo: () => void;
  onRedo: () => void;
  onRevertToStart: () => void;
}) {
  const t = props.labels;
  const workTitle = props.projectInfo?.modName || props.sheet?.source_path || t.translationTools;
  const validating =
    props.busy === "validate_json_translation_sheet" ||
    props.busy === "validate_json_translation_sheet_data";
  const currentSheetMatchesTarget = !props.sheet || translationLanguagesMatch(props.sheet.target_language, props.targetLanguage);
  const canUseCurrentSheet = Boolean(props.sheet && currentSheetMatchesTarget);
  const canExportPatchMod = props.projectInfo?.canExportPatchMod ?? Boolean(props.pckTargetPath);
  return (
    <section className="translation-work-header">
      <details className="work-summary-accordion">
        <summary>
          <span>현재 작업</span>
          <strong>{workTitle}</strong>
          {props.sheet && <SaveStateBadge props={props} />}
        </summary>
        <ProjectSummary
          title={props.projectInfo?.modName || projectNameFromPath(props.sheet?.source_path ?? t.translationTools)}
          version={props.projectInfo?.version}
          author={props.projectInfo?.author}
          targetLanguage={props.sheet?.target_language}
          languages={props.projectInfo?.languages ?? []}
          description={props.projectInfo?.description}
        />
      </details>
      <div className="translation-work-actions">
        <button
          className="toolbar-icon-button"
          aria-label={props.busy === "create_json_translation_sheet" ? "시트 생성 중..." : "열기/업데이트"}
          data-tooltip={props.busy === "create_json_translation_sheet" ? "시트 생성 중..." : "열기/업데이트"}
          onClick={props.onCreate}
          disabled={!props.sourcePath || Boolean(props.busy)}
        >
          <FileJson size={15} />
        </button>
        <button
          className="toolbar-icon-button"
          type="button"
          aria-label={props.busy === "recalculate_json_translation_sheet" ? "시트 재계산 중..." : "번역값 유지하고 시트 재계산"}
          data-tooltip={props.busy === "recalculate_json_translation_sheet" ? "시트 재계산 중..." : "번역값 유지하고 시트 재계산"}
          onClick={props.onRecalculate}
          disabled={!props.sourcePath || !sheetPath || Boolean(props.busy)}
        >
          {props.busy === "recalculate_json_translation_sheet" ? <Loader2 size={15} className="spin-icon" /> : <RefreshCw size={15} />}
        </button>
        <button className="toolbar-icon-button" aria-label="시트 열기" data-tooltip="시트 열기" onClick={props.onLoad} disabled={!sheetPath || Boolean(props.busy)}>
          <FolderOpen size={15} />
        </button>
        <span className="history-actions">
          <button
            className="toolbar-icon-button"
            type="button"
            aria-label="되돌리기"
            data-tooltip={props.history.undoLabel ? `되돌리기: ${props.history.undoLabel} (Ctrl+Z)` : "되돌릴 작업 없음"}
            onClick={onUndo}
            disabled={!props.history.undoLabel || Boolean(props.busy)}
          >
            <Undo2 size={15} />
          </button>
          <button
            className="toolbar-icon-button"
            type="button"
            aria-label="다시 하기"
            data-tooltip={props.history.redoLabel ? `다시 하기: ${props.history.redoLabel} (Ctrl+Y)` : "다시 할 작업 없음"}
            onClick={onRedo}
            disabled={!props.history.redoLabel || Boolean(props.busy)}
          >
            <Redo2 size={15} />
          </button>
          <button
            className="toolbar-icon-button"
            type="button"
            aria-label="작업 시작 시점으로 되돌리기"
            data-tooltip="작업 시작 시점으로 되돌리기"
            onClick={onRevertToStart}
            disabled={!props.history.changedSinceStart || Boolean(props.busy)}
          >
            <History size={15} />
          </button>
        </span>
        <button
          className="toolbar-icon-button dark labeled"
          aria-label="저장"
          data-tooltip="지금 저장 (Ctrl+S) · 변경은 자동으로 저장됩니다"
          onClick={props.onSave}
          disabled={!canUseCurrentSheet || !sheetPath || Boolean(props.busy)}
        >
          <Save size={15} />
          저장
        </button>
        <button
          className="toolbar-icon-button labeled"
          aria-label={validating ? "검증 중..." : t.validateSheet}
          data-tooltip={validating ? "검증 중..." : t.validateSheet}
          onClick={props.onValidate}
          disabled={!sheetPath || !currentSheetMatchesTarget || Boolean(props.busy)}
        >
          {validating ? <Loader2 size={15} className="spin-icon" /> : <CheckCircle2 size={15} />}
          검증
        </button>
        {sheetPath && currentSheetMatchesTarget && (
          <SaveTargetSelect
            sheetPath={sheetPath}
            sheet={props.sheet}
            busy={props.busy}
            onReplaceEntries={props.onReplaceEntries}
            onApply={props.onApply}
          />
        )}
        <button
          className="toolbar-icon-button primary labeled"
          aria-label={props.busy === "apply_json_translation_sheet" || props.busy === "save_json_translation_sheet" ? "번역 파일 적용 중..." : "번역 저장/적용"}
          data-tooltip={props.busy === "apply_json_translation_sheet" || props.busy === "save_json_translation_sheet" ? "번역 파일 적용 중..." : "번역 저장/적용"}
          onClick={() => props.onApply()}
          disabled={!sheetPath || !props.translatedOutputPath || !currentSheetMatchesTarget || filledTranslations === 0 || Boolean(props.busy)}
        >
          {props.busy === "apply_json_translation_sheet" || props.busy === "save_json_translation_sheet" ? <Loader2 size={15} className="spin-icon" /> : <UploadCloud size={15} />}
          적용
        </button>
        {canExportPatchMod && (
          <button
            className="toolbar-icon-button"
            type="button"
            aria-label="번역 모드로 내보내기"
            data-tooltip="번역 모드로 내보내기"
            onClick={props.onExportPatchMod}
            disabled={!canUseCurrentSheet || filledTranslations === 0 || Boolean(props.busy)}
          >
            <Package size={15} />
          </button>
        )}
        <button className="toolbar-icon-button" type="button" aria-label="닫기" data-tooltip="닫기" onClick={props.onCloseSession} disabled={Boolean(props.busy)}>
          <X size={15} />
        </button>
      </div>
    </section>
  );
}

export function TranslationActionsPanel({
  props,
  sheetPath,
  projectTree,
  activeProjectPath,
  setActiveProjectPath,
  sourceLanguageOptions,
  selectedSourceLanguage,
  targetLanguageOptions,
  compareLanguageOptions,
  selectedCompareLanguages,
  exportOnlyEmpty,
  setExportOnlyEmpty,
  filledTranslations,
  copyTreeJson,
}: {
  props: TranslationToolsPageProps;
  sheetPath: string;
  projectTree: TranslationProjectNode | null;
  activeProjectPath: string | null;
  setActiveProjectPath: (path: string | null) => void;
  sourceLanguageOptions: LanguagePreview[];
  selectedSourceLanguage: LanguagePreview | undefined;
  targetLanguageOptions: TargetLanguageOption[];
  compareLanguageOptions: LanguagePreview[];
  selectedCompareLanguages: LanguagePreview[];
  exportOnlyEmpty: boolean;
  setExportOnlyEmpty: React.Dispatch<React.SetStateAction<boolean>>;
  filledTranslations: number;
  copyTreeJson: (node: TranslationProjectNode) => Promise<void>;
}) {
  const t = props.labels;
  const validationWarningCount =
    (props.validation?.missing_entries.length ?? 0) +
    (props.validation?.updated_entries.length ?? 0) +
    (props.validation?.removed_entries.length ?? 0) +
    (props.validation?.format_issues?.length ?? 0) +
    (props.validation?.glossary_issues?.length ?? 0);
  const changeEntryCount = props.sheet?.entries.filter((entry) => entry.status === "new" || entry.status === "updated").length ?? 0;
  const currentSheetMatchesTarget = !props.sheet || translationLanguagesMatch(props.sheet.target_language, props.targetLanguage);
  const canUseCurrentSheet = Boolean(props.sheet && currentSheetMatchesTarget);
  const [aiPromptOpen, setAiPromptOpen] = React.useState(false);
  // Bumped when the AI prompt window saves mod terms, so the glossary reloads them.
  const [modTermsVersion, setModTermsVersion] = React.useState(0);
  return (
    <section className="tool-form">
      <div className="tool-control-stack">
        <section className="work-tools-card" aria-label="작업 도구">
          <div className="work-tools-primary">
            <button
              type="button"
              className="primary icon-button-text compact"
              onClick={() => setAiPromptOpen(true)}
              disabled={!canUseCurrentSheet || Boolean(props.busy)}
              title="웹 AI 채팅용 프롬프트를 파트별로 만들고 답변을 받아 적용합니다."
            >
              <MessageSquareText size={15} />
              AI 프롬프트
            </button>
            <button
              type="button"
              className="icon-button-text compact"
              onClick={() => props.onExportShortJson(exportOnlyEmpty)}
              disabled={!canUseCurrentSheet || Boolean(props.busy)}
              title="번역할 원문을 id 기반 JSON으로 내보냅니다."
            >
              <FileJson size={15} />
              번역용 JSON
            </button>
            <button
              type="button"
              className="icon-button-text compact"
              onClick={props.onImportValues}
              disabled={!canUseCurrentSheet || Boolean(props.busy)}
              title="번역된 CSV/JSON 파일을 불러와 id로 매칭합니다."
            >
              <FileUp size={15} />
              가져오기
            </button>
          </div>
          <label className="inline-check export-option">
            <input type="checkbox" checked={exportOnlyEmpty} onChange={(event) => setExportOnlyEmpty(event.target.checked)} />
            <span>빈 값만 내보내기</span>
          </label>
          <div className="work-tools-secondary">
            <button type="button" onClick={props.onExportCsv} disabled={!canUseCurrentSheet || Boolean(props.busy)}>
              <FileDown size={13} />
              CSV 내보내기
            </button>
            <button
              type="button"
              onClick={() => props.onExportShortJson({ aiKit: true, onlyEmpty: exportOnlyEmpty })}
              disabled={!canUseCurrentSheet || Boolean(props.busy)}
              title="번역 JSON + 규칙 안내 + 용어집 파일을 함께 내보냅니다."
            >
              <Bot size={13} />
              AI 작업 키트
            </button>
            <button
              type="button"
              onClick={() => props.onExportShortJson({ changeOnly: true })}
              disabled={!canUseCurrentSheet || changeEntryCount === 0 || Boolean(props.busy)}
            >
              <FileJson size={13} />
              신규/변경 JSON
            </button>
            <button
              type="button"
              onClick={() => props.onExportShortJson({ warningOnly: true })}
              disabled={!canUseCurrentSheet || validationWarningCount === 0 || Boolean(props.busy)}
            >
              <AlertTriangle size={13} />
              검증 오류 JSON
            </button>
            <button
              type="button"
              onClick={() => props.onExportShortJson({ scopePath: activeProjectPath ?? undefined })}
              disabled={!canUseCurrentSheet || !activeProjectPath || Boolean(props.busy)}
              title="아래 프로젝트 파일 목록에서 선택한 파일만 내보냅니다."
            >
              <FileJson size={13} />
              선택 파일 JSON
            </button>
          </div>
        </section>
        {canUseCurrentSheet && filledTranslations === 0 && (
          <p className="inline-warning">번역값이 0개입니다. 표의 translated_value에 직접 입력하거나 CSV/JSON 매칭 후 적용할 수 있습니다.</p>
        )}
        {props.toolError && (
          <article className="validation-card warn tool-error-card">
            <strong>오류</strong>
            <span>{props.toolError}</span>
          </article>
        )}
        <article className="target-language-note language-route-card">
          <div className="language-route-header">
            <strong>언어 경로</strong>
            <button
              className="toolbar-icon-button"
              type="button"
              aria-label="대상 모드 폴더 열기"
              data-tooltip="대상 모드 폴더 열기"
              onClick={() => props.projectInfo?.modPath && props.onOpenPath(parentPath(props.projectInfo.modPath))}
              disabled={!props.projectInfo?.modPath || Boolean(props.busy)}
            >
              <FolderOpen size={15} />
            </button>
          </div>
          {sourceLanguageOptions.length > 0 && (
            <label>
              <span>원본 언어</span>
              <select
                value={selectedSourceLanguage?.sample_path ?? ""}
                onChange={(event) => props.onSwitchSourceLanguage(event.target.value)}
                disabled={Boolean(props.busy)}
              >
                {sourceLanguageOptions.map((language) => (
                  <option value={language.sample_path} key={`${language.code}-${language.sample_path}`}>
                    {languageFolderCode(language)} · {languageKeyCount(language)} keys
                    {recommendedSourceLanguage(sourceLanguageOptions)?.sample_path === language.sample_path ? " · MAIN" : ""}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label>
            <span>대상 언어</span>
            <select value={props.targetLanguage} onChange={(event) => props.setTargetLanguage(event.target.value)} disabled={Boolean(props.busy)}>
              {targetLanguageOptions.map((option) => (
                <option value={option.code} key={option.code}>
                  {option.label} ({option.code})
                </option>
              ))}
            </select>
          </label>
          <small>원본 언어는 가장 많은 key를 가진 MAIN 언어를 기본으로 선택합니다.</small>
        </article>
        {props.sheet && !currentSheetMatchesTarget && (
          <p className="inline-warning">현재 시트는 {props.sheet.target_language}용입니다. {props.targetLanguage} 번역은 별도 시트로 생성/업데이트됩니다.</p>
        )}
        {compareLanguageOptions.length > 0 && (
          <section className="language-compare-card">
            <div className="compare-card-header">
              <span>비교 언어</span>
              <button
                type="button"
                className={props.compareViewEnabled ? "language-toggle active compact" : "language-toggle compact"}
                onClick={() => {
                  if (selectedCompareLanguages.length === 0) {
                    const defaultLanguage = compareLanguageOptions[0];
                    if (defaultLanguage) {
                      props.onToggleCompareLanguage(defaultLanguage.sample_path);
                    }
                    return;
                  }
                  props.onToggleCompareView();
                }}
                disabled={Boolean(props.busy)}
              >
                {props.compareViewEnabled ? "비교 끄기" : "비교 보기"}
              </button>
            </div>
            <div className="language-toggle-group">
              {compareLanguageOptions.map((language) => {
                const active = props.compareSamplePaths.includes(language.sample_path);
                return (
                  <button
                    type="button"
                    className={active ? "language-toggle active" : "language-toggle"}
                    onClick={() => props.onToggleCompareLanguage(language.sample_path)}
                    disabled={Boolean(props.busy)}
                    key={`${language.code}-${language.sample_path}`}
                  >
                    <strong>{languageFolderCode(language)}</strong>
                    <small>{language.files}</small>
                  </button>
                );
              })}
            </div>
          </section>
        )}
        <GlossaryPanel
          sourcePath={props.sheet?.source_path ?? props.sourcePath}
          sampleKey={props.sheet?.entries[0]?.key ?? ""}
          targetLanguage={props.sheet?.target_language ?? props.targetLanguage}
          projectKey={translationProjectKey(props.projectInfo ?? null)}
          refreshKey={modTermsVersion}
          disabled={Boolean(props.busy)}
          onSaved={() => {
            if (canUseCurrentSheet) {
              void props.onValidate();
            }
          }}
        />
        <details className="path-details">
          <summary>경로/고급 설정</summary>
          <label>
            <span>{t.sourceJson}</span>
            <input value={props.sourcePath} onChange={(event) => props.setSourcePath(event.target.value)} placeholder=".../localization/eng" />
          </label>
          <label>
            <span>{t.existingSheet}</span>
            <input value={props.existingSheetPath} onChange={(event) => props.setExistingSheetPath(event.target.value)} placeholder=".../cards.kor.translation.json" />
          </label>
          <label>
            <span>{t.outputSheet}</span>
            <input value={props.outputSheetPath} onChange={(event) => props.setOutputSheetPath(event.target.value)} placeholder={`${props.settings.translation_work_dir}/json_sheets/cards.kor.translation.json`} />
          </label>
          <label>
            <span>{t.translatedOutput}</span>
            <input value={props.translatedOutputPath} onChange={(event) => props.setTranslatedOutputPath(event.target.value)} placeholder=".../translated" />
          </label>
          <label>
            <span>PCK 내부 삽입 경로</span>
            <input value={props.pckTargetPath} onChange={(event) => props.setPckTargetPath(event.target.value)} placeholder="AkiSister/localization/kor" />
          </label>
        </details>
      </div>
      {aiPromptOpen && props.sheet && (
        <AiPromptModal
          sheet={props.sheet}
          projectInfo={props.projectInfo ?? null}
          onlyEmptyDefault={exportOnlyEmpty}
          onClose={() => setAiPromptOpen(false)}
          onPasteAnswer={props.onPasteStructuredJson}
          onTermsSaved={() => {
            setModTermsVersion((version) => version + 1);
            if (props.validation) {
              void props.onValidate();
            }
          }}
        />
      )}
      <TranslationProjectTree
        tree={projectTree}
        selectedPath={activeProjectPath}
        onSelect={setActiveProjectPath}
        onCopyJson={(node) => void copyTreeJson(node)}
      />
    </section>
  );
}
