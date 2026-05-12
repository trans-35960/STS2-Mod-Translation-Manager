import React from "react";
import {
  AlertTriangle,
  CheckCircle2,
  FileDown,
  FileJson,
  FileUp,
  FolderOpen,
  Loader2,
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
import {
  languageFolderCode,
  projectNameFromPath,
  translationLanguagesMatch,
  type TranslationProjectNode,
} from "./translationUtils";

type TargetLanguageOption = {
  code: string;
  label: string;
};

export function TranslationWorkHeader({
  props,
  sheetPath,
  filledTranslations,
}: {
  props: TranslationToolsPageProps;
  sheetPath: string;
  filledTranslations: number;
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
        <button className="toolbar-icon-button dark" aria-label="저장" data-tooltip="저장" onClick={props.onSave} disabled={!canUseCurrentSheet || !sheetPath || Boolean(props.busy)}>
          <Save size={15} />
        </button>
        <button
          className="toolbar-icon-button"
          aria-label={validating ? "검증 중..." : t.validateSheet}
          data-tooltip={validating ? "검증 중..." : t.validateSheet}
          onClick={props.onValidate}
          disabled={!sheetPath || !currentSheetMatchesTarget || Boolean(props.busy)}
        >
          {validating ? <Loader2 size={15} className="spin-icon" /> : <CheckCircle2 size={15} />}
        </button>
        <button
          className="toolbar-icon-button primary"
          aria-label={props.busy === "apply_json_translation_sheet" || props.busy === "save_json_translation_sheet" ? "번역 파일 적용 중..." : "번역 저장/적용"}
          data-tooltip={props.busy === "apply_json_translation_sheet" || props.busy === "save_json_translation_sheet" ? "번역 파일 적용 중..." : "번역 저장/적용"}
          onClick={props.onApply}
          disabled={!sheetPath || !props.translatedOutputPath || !currentSheetMatchesTarget || filledTranslations === 0 || Boolean(props.busy)}
        >
          {props.busy === "apply_json_translation_sheet" || props.busy === "save_json_translation_sheet" ? <Loader2 size={15} className="spin-icon" /> : <UploadCloud size={15} />}
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
    (props.validation?.format_issues?.length ?? 0);
  const changeEntryCount = props.sheet?.entries.filter((entry) => entry.status === "new" || entry.status === "updated").length ?? 0;
  const currentSheetMatchesTarget = !props.sheet || translationLanguagesMatch(props.sheet.target_language, props.targetLanguage);
  const canUseCurrentSheet = Boolean(props.sheet && currentSheetMatchesTarget);
  return (
    <section className="tool-form">
      <div className="tool-control-stack">
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
                onClick={props.onToggleCompareView}
                disabled={selectedCompareLanguages.length === 0 || Boolean(props.busy)}
              >
                비교 보기
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
        {canUseCurrentSheet && filledTranslations === 0 && (
          <p className="inline-warning">번역값이 0개입니다. 표의 translated_value에 직접 입력하거나 CSV/JSON 매칭 후 적용할 수 있습니다.</p>
        )}
        {props.toolError && (
          <article className="validation-card warn tool-error-card">
            <strong>오류</strong>
            <span>{props.toolError}</span>
          </article>
        )}
        <details className="action-details">
          <summary>가져오기/내보내기</summary>
          <label className="inline-check export-option">
            <input type="checkbox" checked={exportOnlyEmpty} onChange={(event) => setExportOnlyEmpty(event.target.checked)} />
            <span>번역용 JSON은 빈 값만 내보내기</span>
          </label>
          <div className="compact-action-row">
            <button className="toolbar-icon-button" aria-label="CSV/JSON 매칭" data-tooltip="CSV/JSON 매칭" onClick={props.onImportValues} disabled={!canUseCurrentSheet || Boolean(props.busy)}>
              <FileUp size={15} />
            </button>
            <button className="toolbar-icon-button" aria-label="CSV 내보내기" data-tooltip="CSV 내보내기" onClick={props.onExportCsv} disabled={!canUseCurrentSheet || Boolean(props.busy)}>
              <FileDown size={15} />
            </button>
            <button className="toolbar-icon-button" aria-label="번역용 JSON" data-tooltip="번역용 JSON" onClick={() => props.onExportShortJson(exportOnlyEmpty)} disabled={!canUseCurrentSheet || Boolean(props.busy)}>
              <FileJson size={15} />
            </button>
            <button
              className="toolbar-icon-button"
              aria-label="선택한 프로젝트 파일 JSON"
              data-tooltip="선택한 프로젝트 파일 JSON"
              onClick={() => props.onExportShortJson({ scopePath: activeProjectPath ?? undefined })}
              disabled={!canUseCurrentSheet || !activeProjectPath || Boolean(props.busy)}
            >
              <FileJson size={15} />
            </button>
            <button
              className="toolbar-icon-button"
              aria-label="검증 오류 JSON"
              data-tooltip="검증 오류 JSON"
              onClick={() => props.onExportShortJson({ warningOnly: true })}
              disabled={!canUseCurrentSheet || validationWarningCount === 0 || Boolean(props.busy)}
            >
              <AlertTriangle size={15} />
            </button>
            <button
              className="toolbar-icon-button"
              aria-label="신규/변경 JSON"
              data-tooltip="신규/변경 JSON"
              onClick={() => props.onExportShortJson({ changeOnly: true })}
              disabled={!canUseCurrentSheet || changeEntryCount === 0 || Boolean(props.busy)}
            >
              <FileJson size={15} />
            </button>
          </div>
        </details>
      </div>
      <TranslationProjectTree
        tree={projectTree}
        selectedPath={activeProjectPath}
        onSelect={setActiveProjectPath}
        onCopyJson={(node) => void copyTreeJson(node)}
      />
    </section>
  );
}
