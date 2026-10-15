import type { ModRow } from "../../types";
import { languageFolderCode, normalizeLanguageTag } from "./modUtils";

/**
 * Where a mod's text in the target language comes from. A mod can have more
 * than one (a bridge translation on top of a patch mod); `sources` lists all
 * of them, strongest first.
 */
export type TranslationSource =
  /** The mod ships the target language itself. */
  | "native"
  /** The app wrote the translation into the mod's own files. */
  | "package"
  /** The bridge applies a stored translation in game. */
  | "bridge"
  /** A separate translation mod (patch) is turned on. */
  | "patch";

export type TranslationStatus =
  | { kind: "system" }
  /** The row is itself a translation mod for another mod. */
  | { kind: "patchMod" }
  | { kind: "translated"; sources: TranslationSource[]; patchOff: boolean }
  /** Has text (language files or DLL strings) but none in the target language. */
  | { kind: "untranslated"; patchOff: boolean }
  /** No language files found (yet); may have no text at all. */
  | { kind: "noText" };

export type TranslationSection = "system" | "translated" | "untranslated";

export function translationStatus(mod: ModRow, targetLanguage: string): TranslationStatus {
  if (mod.system_mod) {
    return { kind: "system" };
  }
  if (mod.is_translation_patch) {
    return { kind: "patchMod" };
  }
  const target = normalizeLanguageTag(targetLanguage);
  const sources: TranslationSource[] = [];
  if (mod.translation_applied) {
    sources.push("package");
  } else if (mod.language_preview.some((language) => normalizeLanguageTag(languageFolderCode(language)) === target)) {
    sources.push("native");
  }
  if ((mod.bridge_stored ?? []).some((stored) => normalizeLanguageTag(stored.language) === target && stored.entries + stored.dll_strings > 0)) {
    sources.push("bridge");
  }
  if (mod.translation_patch_active_count > 0) {
    sources.push("patch");
  }
  // A patch mod that is off translates nothing, but is one switch away.
  const patchOff = mod.translation_patch_count > 0 && mod.translation_patch_active_count === 0;
  if (sources.length > 0) {
    return { kind: "translated", sources, patchOff };
  }
  if (mod.language_preview.length > 0 || (mod.dll_files ?? []).length > 0 || patchOff) {
    return { kind: "untranslated", patchOff };
  }
  return { kind: "noText" };
}

export function translationSection(status: TranslationStatus): TranslationSection {
  if (status.kind === "system") return "system";
  if (status.kind === "translated") return "translated";
  return "untranslated";
}

/**
 * A version group's status: the loaded version speaks for it (any version
 * when none is loaded), and a translated one wins over the rest.
 */
export function groupTranslationStatus(mods: ModRow[], targetLanguage: string): TranslationStatus {
  const own = mods.filter((mod) => !mod.is_translation_patch);
  const active = own.filter((mod) => mod.active);
  const statuses = (active.length > 0 ? active : own).map((mod) => translationStatus(mod, targetLanguage));
  return (
    statuses.find((status) => status.kind === "translated") ??
    statuses.find((status) => status.kind === "system") ??
    statuses[0] ?? { kind: "patchMod" }
  );
}

export function isTranslated(mod: ModRow, targetLanguage: string): boolean {
  return translationStatus(mod, targetLanguage).kind === "translated";
}

export const TRANSLATION_SOURCE_LABELS: Record<TranslationSource, string> = {
  native: "자체 번역",
  package: "패키지 수정",
  bridge: "브리지 번역",
  patch: "번역 모드",
};

export const TRANSLATION_SOURCE_HINTS: Record<TranslationSource, string> = {
  native: "모드에 처음부터 들어 있는 번역입니다",
  package: "이 프로그램이 모드 파일에 직접 번역을 넣었습니다. 모드가 업데이트되면 다시 적용해야 합니다",
  bridge: "번역 브리지가 게임 안에서 번역을 적용합니다. 모드 파일은 그대로입니다",
  patch: "별도로 설치한 번역 모드가 켜져 있습니다",
};
