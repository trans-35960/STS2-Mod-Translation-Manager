import { MOD_VIEW_MODE_STORAGE_KEY, TRANSLATION_SESSION_STORAGE_KEY } from "../constants";
import type { ApplyResultState, TranslationSessionState } from "../types";

export function readStoredTranslationSession(): TranslationSessionState | null {
  try {
    const raw = window.localStorage.getItem(TRANSLATION_SESSION_STORAGE_KEY);
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as Partial<TranslationSessionState>;
    if (!parsed.sourcePath && !parsed.sheet) {
      return null;
    }
    if (isCompletedPckApplyResult(parsed.applyResult ?? null)) {
      window.localStorage.removeItem(TRANSLATION_SESSION_STORAGE_KEY);
      return null;
    }
    return {
      sourcePath: parsed.sourcePath ?? "",
      existingSheetPath: parsed.existingSheetPath ?? "",
      outputSheetPath: parsed.outputSheetPath ?? "",
      translatedOutputPath: parsed.translatedOutputPath ?? "",
      pckTargetPath: parsed.pckTargetPath ?? "",
      sheet: parsed.sheet ?? null,
      report: parsed.report ?? null,
      validation: parsed.validation ?? null,
      applyResult: parsed.applyResult ?? null,
      projectInfo: parsed.projectInfo ?? null,
      targetLanguage: parsed.targetLanguage ?? parsed.sheet?.target_language ?? "kor",
      compareSamplePaths: parsed.compareSamplePaths ?? [],
      compareValuesByLanguage: parsed.compareValuesByLanguage ?? {},
      compareViewEnabled: false,
    };
  } catch {
    return null;
  }
}

export function writeStoredTranslationSession(session: TranslationSessionState | null) {
  try {
    if (!session || (!session.sourcePath && !session.sheet) || isCompletedPckApplyResult(session.applyResult)) {
      window.localStorage.removeItem(TRANSLATION_SESSION_STORAGE_KEY);
      return;
    }
    window.localStorage.setItem(TRANSLATION_SESSION_STORAGE_KEY, JSON.stringify(session));
  } catch {
    // Local storage can be unavailable in restricted preview contexts.
  }
}

export function isCompletedPckApplyResult(result: ApplyResultState | null | undefined): boolean {
  return Boolean(
    result &&
    !result.error &&
    result.applied_entries > 0 &&
    (result.installed_mod_path || result.packed_pck_path || result.language_output_path),
  );
}

export function readStoredModViewMode(): boolean {
  try {
    return window.localStorage.getItem(MOD_VIEW_MODE_STORAGE_KEY) === "simple";
  } catch {
    return false;
  }
}

export function writeStoredModViewMode(simple: boolean) {
  try {
    window.localStorage.setItem(MOD_VIEW_MODE_STORAGE_KEY, simple ? "simple" : "detail");
  } catch {
    // Local storage can be unavailable in restricted preview contexts.
  }
}
