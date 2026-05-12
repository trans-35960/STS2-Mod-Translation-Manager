import { invokeCommand } from "../../api/tauri";
import {
  defaultTranslatableResourcePath,
  isHardcodedResourcePath,
  languageResourceName,
  languageResourceRoot,
} from "../../features/mods/modUtils";
import {
  inferPckTargetPath,
  replaceLocalizationLanguageInPath,
  retargetTranslationSheetPath,
} from "../../features/translation/translationUtils";
import { previewJsonReport, previewJsonSheet } from "../../previewData";
import type { ExtractionTreeNode, JsonSheetAction, ModRow, NodeTranslationResult } from "../../types";
import { formatError } from "../../utils/logging";
import { isPreviewRuntime } from "../../utils/runtime";
import type { TranslationActionsParams } from "./types";

export function useTranslationSessionActions({
  appendLog,
  dashboard,
  jsonSheet,
  jsonTargetLanguage,
  settingsDraft,
  translationProject,
  clearStoredSession,
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
  setJsonSource,
  setJsonTargetLanguage,
  setJsonToolError,
  setJsonTranslatedOutput,
  setJsonValidation,
  setPage,
  setPasteCandidatesByKey,
  setPendingExtract,
  setSelectedRows,
  setTranslationProject,
}: TranslationActionsParams) {
  function closeTranslationSession() {
    setJsonSource("");
    setJsonExistingSheet("");
    setJsonOutputSheet("");
    setJsonTranslatedOutput("");
    setJsonPckTargetPath("");
    setJsonSheet(null);
    setJsonReport(null);
    setJsonValidation(null);
    setJsonApplyResult(null);
    setJsonToolError("");
    setTranslationProject(null);
    setJsonTargetLanguage(settingsDraft?.target_language || dashboard?.settings.target_language || "kor");
    setCompareSamplePaths([]);
    setCompareValuesByLanguage({});
    setCompareViewEnabled(false);
    setSelectedRows([]);
    setPasteCandidatesByKey({});
    clearStoredSession();
    setPage("mods");
    appendLog("번역 작업을 닫았습니다.");
  }

  async function prepareTreeNode(mod: ModRow, node: ExtractionTreeNode, outputDir?: string, force = false) {
    const sourceMod = baseModForTranslationPatch(mod);
    if (isPreviewRuntime()) {
      const resourcePath = node.path || node.name;
      const sourceBase = outputDir || `${dashboard?.paths.translation_work ?? "translation_work"}/selected/${sourceMod.key}`;
      const sourcePath = `${sourceBase}/${node.name}.json`;
      return {
        message: `Preview extraction: ${resourcePath}`,
        source_path: sourcePath,
        existing_sheet_path: "",
        output_sheet_path: `${dashboard?.paths.translation_work ?? "translation_work"}/translation_memory/${sourceMod.key}/${node.name}.kor.translation.json`,
        translated_output_path: `${dashboard?.paths.translation_work ?? "translation_work"}/selected/${sourceMod.key}/translated/${node.name}.json`,
        copied_files: 1,
        mod_key: sourceMod.key,
        mod_path: sourceMod.path,
        mod_name: sourceMod.name,
        mod_version: sourceMod.version_hint ?? "-",
        mod_author: "",
        mod_description: "",
        available_languages: sourceMod.language_preview,
        can_export_patch_mod: true,
      };
    }
    return await invokeCommand<NodeTranslationResult>("prepare_translation_node", {
      key: sourceMod.key,
      resourcePath: node.path || node.name,
      outputDir: outputDir ?? null,
      force,
    });
  }

  async function extractTreeNode(mod: ModRow, node: ExtractionTreeNode, outputDir?: string, force = false) {
    const sourceMod = baseModForTranslationPatch(mod);
    setBusy("extract_translation");
    try {
      const selected = outputDir || (dashboard?.paths.translation_work ?? "");
      if (!selected) {
        appendLog("추출 대상 폴더를 먼저 선택하세요.");
        return;
      }
      const result = await invokeCommand<{ message: string }>("extract_translation", {
        key: sourceMod.key,
        outputDir: selected,
        resourcePath: node.path || node.name,
        force,
      });
      appendLog(result.message);
    } catch (error) {
      appendLog(`항목 추출 실패: ${formatError(error)}`);
    } finally {
      setBusy(null);
    }
  }

  async function openTreeNodeInTranslationTools(mod: ModRow, node: ExtractionTreeNode, outputDir?: string, force = false) {
    const sourceMod = baseModForTranslationPatch(mod);
    const sourceNode = sourceMod.key === mod.key ? node : defaultTranslationNode(sourceMod);
    setBusy(`prepare_translation_node:${sourceMod.key}`);
    setJsonToolError("");
    try {
      const prepared = await prepareTreeNode(sourceMod, sourceNode, outputDir, force);
      setJsonSource(prepared.source_path);
      setJsonExistingSheet(prepared.existing_sheet_path || prepared.output_sheet_path);
      setJsonOutputSheet(prepared.output_sheet_path);
      setJsonTranslatedOutput(prepared.translated_output_path);
      const targetLanguage = jsonTargetLanguage || settingsDraft?.target_language || dashboard?.settings.target_language || "kor";
      setJsonTargetLanguage(targetLanguage);
      setCompareSamplePaths([]);
      setCompareValuesByLanguage({});
      setCompareViewEnabled(false);
      setTranslationProject({
        modKey: prepared.mod_key,
        modPath: prepared.mod_path,
        modName: prepared.mod_name,
        version: prepared.mod_version,
        author: prepared.mod_author,
        description: prepared.mod_description,
        languages: prepared.available_languages,
        canExportPatchMod: prepared.can_export_patch_mod,
      });
      setSelectedRows([]);
      setPasteCandidatesByKey({});
      setJsonApplyResult(null);
      setPendingExtract(null);
      setPage("translationTools");
      appendLog(prepared.message);
      appendLog(`번역 시트 생성 시작: ${prepared.source_path}`);
      if (isPreviewRuntime()) {
        setJsonSheet(previewJsonSheet);
        setJsonReport(previewJsonReport);
        setJsonValidation(null);
        setPasteCandidatesByKey({});
        return;
      }
      const sheet = await invokeCommand<JsonSheetAction>("create_json_translation_sheet", {
        sourcePath: prepared.source_path,
        existingSheetPath: prepared.existing_sheet_path || prepared.output_sheet_path,
        outputPath: prepared.output_sheet_path,
        targetLanguage,
      });
      setJsonSheet(sheet.sheet);
      setJsonReport(sheet.report);
      setJsonPckTargetPath(inferPckTargetPath(sheet.sheet));
      setJsonValidation(null);
      setJsonApplyResult(null);
      setPasteCandidatesByKey({});
      appendLog(sheet.message);
    } catch (error) {
      const message = `번역 도구 작업 실패: ${formatError(error)}`;
      setJsonToolError(message);
      appendLog(message);
    } finally {
      setBusy(null);
    }
  }

  async function openModLanguageInTranslationTools(mod: ModRow, resourcePath?: string) {
    const sourceMod = baseModForTranslationPatch(mod);
    const path = sourceMod.key === mod.key
      ? resourcePath || defaultTranslatableResourcePath(sourceMod)
      : defaultTranslatableResourcePath(sourceMod);
    if (!path) {
      appendLog(`${sourceMod.name}: 감지된 언어/하드코딩 후보가 없습니다. 수동 경로를 입력해 주세요.`);
      return;
    }
    await openTreeNodeInTranslationTools(sourceMod, {
      name: languageResourceName(path),
      path,
      source_path: path,
      kind: isHardcodedResourcePath(path) ? "hardcoded" : "language",
      children: [],
    });
  }

  function baseModForTranslationPatch(mod: ModRow): ModRow {
    if (!mod.is_translation_patch) {
      return mod;
    }
    const mods = dashboard?.mods ?? [];
    const byKey = mod.translation_target_key
      ? mods.find((item) => item.key === mod.translation_target_key)
      : null;
    if (byKey) {
      return byKey;
    }
    const targetToken = normalizeModToken(mod.translation_target_id || mod.translation_target_name || "");
    const byTarget = targetToken
      ? mods.find((item) => !item.is_translation_patch && (normalizeModToken(item.key) === targetToken || normalizeModToken(item.name) === targetToken))
      : null;
    if (byTarget) {
      return byTarget;
    }
    const byDependency = mod.dependencies
      .map((dependency) => dependency.key)
      .filter((key): key is string => Boolean(key))
      .map((key) => mods.find((item) => item.key === key && !item.is_translation_patch))
      .find(Boolean);
    return byDependency ?? mod;
  }

  function defaultTranslationNode(mod: ModRow): ExtractionTreeNode {
    const path = defaultTranslatableResourcePath(mod);
    return {
      name: languageResourceName(path || mod.key),
      path,
      source_path: path,
      kind: isHardcodedResourcePath(path) ? "hardcoded" : "language",
      children: [],
    };
  }

  function normalizeModToken(value: string): string {
    return value.toLowerCase().replace(/[^a-z0-9가-힣]/g, "");
  }

  async function switchTranslationSourceLanguage(samplePath: string) {
    const modKey = translationProject?.modKey;
    const fallbackMod: ModRow | null = translationProject?.modKey
      ? {
          key: translationProject.modKey,
          name: translationProject.modName,
          manifest_id: null,
          group_name: null,
          active: false,
          managed: true,
          external: false,
          source_label: "",
          kind: "",
          version_hint: translationProject.version || null,
          bytes: 0,
          modified_epoch: null,
          registered_epoch: null,
          updated_epoch: null,
          path: translationProject.modPath || "",
          update_state: "",
          change_reasons: [],
          translation_state: "",
          translation_applied: false,
          translation_applied_epoch: null,
          translation_patch_count: 0,
          translation_patch_active_count: 0,
          translation_patch_names: [],
          needs_recheck: false,
          translation_review_required: false,
          safety_warnings: [],
          extraction_hint: "",
          extraction_source_path: "",
          extraction_target: "",
          is_translation_patch: false,
          translation_target_id: null,
          translation_target_key: null,
          translation_target_name: null,
          translation_target_version: null,
          dependencies: [],
          language_preview: translationProject.languages,
          extraction_tree: [],
        }
      : null;
    const mod = (dashboard?.mods ?? []).find((item) => item.key === modKey) ?? fallbackMod;
    if (!mod) {
      appendLog("현재 번역 작업에 모드 연결 정보가 없어 원본 언어를 변경할 수 없습니다. 모드 목록에서 번역 도구를 다시 열어 주세요.");
      return;
    }
    const resourcePath = languageResourceRoot(samplePath);
    if (!resourcePath) {
      appendLog("선택한 언어의 localization 경로를 계산하지 못했습니다.");
      return;
    }
    await openModLanguageInTranslationTools(mod, resourcePath);
  }

  function setTargetLanguage(value: string) {
    setJsonTargetLanguage(value);
    setJsonOutputSheet((current) => retargetTranslationSheetPath(current, value));
    setJsonExistingSheet((current) => retargetTranslationSheetPath(current, value));
    setJsonPckTargetPath((current) => replaceLocalizationLanguageInPath(current || inferPckTargetPath(jsonSheet), value));
    setCompareSamplePaths([]);
    setCompareValuesByLanguage({});
    setCompareViewEnabled(false);
    setJsonValidation(null);
    setJsonApplyResult(null);
    setSelectedRows([]);
    setPasteCandidatesByKey({});
  }

  return {
    closeTranslationSession,
    extractTreeNode,
    openModLanguageInTranslationTools,
    openTreeNodeInTranslationTools,
    setTargetLanguage,
    switchTranslationSourceLanguage,
  };
}
