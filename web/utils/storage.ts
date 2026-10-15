import {
  MOD_TABLE_COLUMNS_STORAGE_KEY,
  MOD_VIEW_MODE_STORAGE_KEY,
  TRANSLATION_SESSION_STORAGE_KEY,
  TRANSLATION_SOURCE_STORAGE_KEY,
} from "../constants";
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

/**
 * The source language resource each mod was last translated from, so opening
 * the mod again keeps comparing against the same source instead of switching
 * to the language with the most keys.
 */
export function readStoredTranslationSource(modKey: string): string {
  try {
    const raw = window.localStorage.getItem(TRANSLATION_SOURCE_STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    const value = parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>)[modKey] : undefined;
    return typeof value === "string" ? value : "";
  } catch {
    return "";
  }
}

export function writeStoredTranslationSource(modKey: string, resourcePath: string) {
  try {
    const raw = window.localStorage.getItem(TRANSLATION_SOURCE_STORAGE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    const stored = parsed && typeof parsed === "object" && !Array.isArray(parsed) ? (parsed as Record<string, string>) : {};
    stored[modKey] = resourcePath;
    window.localStorage.setItem(TRANSLATION_SOURCE_STORAGE_KEY, JSON.stringify(stored));
  } catch {
    // Remembering the source language is a convenience; the default still works.
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

export function readStoredModTableColumns<T extends Record<string, number>>(
  view: "detail" | "simple",
  defaults: T,
): T {
  try {
    const raw = window.localStorage.getItem(MOD_TABLE_COLUMNS_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) as Partial<Record<"detail" | "simple", Record<string, unknown>>> : {};
    const stored = parsed[view] ?? {};
    return mergeNumericColumns(defaults, stored);
  } catch {
    return defaults;
  }
}

export function writeStoredModTableColumns(
  view: "detail" | "simple",
  columns: Record<string, number>,
) {
  try {
    const raw = window.localStorage.getItem(MOD_TABLE_COLUMNS_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) as Partial<Record<"detail" | "simple", Record<string, number>>> : {};
    window.localStorage.setItem(
      MOD_TABLE_COLUMNS_STORAGE_KEY,
      JSON.stringify({
        ...parsed,
        [view]: columns,
      }),
    );
  } catch {
    // Local storage can be unavailable in restricted preview contexts.
  }
}

function mergeNumericColumns<T extends Record<string, number>>(
  defaults: T,
  stored: Record<string, unknown>,
): T {
  const next = { ...defaults };
  for (const key of Object.keys(defaults) as Array<keyof T>) {
    const value = stored[String(key)];
    if (typeof value === "number" && Number.isFinite(value)) {
      next[key] = value as T[keyof T];
    }
  }
  return next;
}
