import { invokeCommand } from "../../api/tauri";
import type { JsonValidation, ProjectTranslationSettings } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import { normalizeSlotId } from "./translationUtils";

export const MAX_CHARS_STORAGE_KEY = "sts2.aiPrompt.maxChars.v1";
export const DEFAULT_MAX_CHARS = 5000;

export function slotKey(file: string, id: string) {
  return `${file}\u0000${normalizeSlotId(id) ?? id}`;
}

export function budgetOf(value: number) {
  return Math.max(500, Math.round(value) || DEFAULT_MAX_CHARS);
}

export function readStoredMaxChars() {
  try {
    const stored = Number(window.localStorage.getItem(MAX_CHARS_STORAGE_KEY));
    return Number.isFinite(stored) && stored >= 500 ? stored : DEFAULT_MAX_CHARS;
  } catch {
    return DEFAULT_MAX_CHARS;
  }
}

export async function loadPromptSettings(projectKey: string): Promise<ProjectTranslationSettings> {
  if (!projectKey || isPreviewRuntime()) {
    return { mod_description: "", tone: "official", terms: [], extra_instructions: "" };
  }
  return invokeCommand("load_project_translation_settings", { projectKey });
}

/**
 * Format and term problems by entry, the warnings a re-ask can fix. Empty
 * and changed-source entries have their own options (빈 값만, 원문 바뀐 항목).
 */
export function validationIssuesByKey(
  validation: JsonValidation | null,
  kinds: ReadonlySet<string> | null = null,
): Record<string, string[]> {
  const issues: Record<string, string[]> = {};
  for (const issue of fixableValidationIssues(validation)) {
    if (kinds === null || kinds.has(issue.kind)) {
      (issues[issue.key] ??= []).push(issue.message);
    }
  }
  return issues;
}

/** Structure and term warnings, the ones a fix prompt can ask about. */
export function fixableValidationIssues(validation: JsonValidation | null) {
  return [...(validation?.format_issues ?? []), ...(validation?.glossary_issues ?? [])];
}
