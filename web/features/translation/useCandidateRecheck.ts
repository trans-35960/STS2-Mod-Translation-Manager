import React from "react";
import { invokeCommand } from "../../api/tauri";
import type { JsonTranslationSheet, JsonValidation, JsonValidationIssue, PasteCandidate } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";

export type CandidateCheckState = "fixed" | "clean" | "still" | "new";

export type CandidateCheck = {
  state: CandidateCheckState;
  /** Warnings the candidate value would have. */
  after: JsonValidationIssue[];
};

const RECHECK_DELAY_MS = 350;

/**
 * Validates every pasted candidate as if it were applied, so the review list
 * can tell whether an answer fixes the entry's warnings or brings new ones.
 * Null until the first check finishes (and in the browser preview).
 */
export function useCandidateRecheck(
  sheet: JsonTranslationSheet | null,
  candidates: Record<string, PasteCandidate>,
  validation: JsonValidation | null,
  projectKey: string,
): Map<string, CandidateCheck> | null {
  const [checks, setChecks] = React.useState<Map<string, CandidateCheck> | null>(null);
  const requestRef = React.useRef(0);

  React.useEffect(() => {
    const keys = Object.keys(candidates);
    if (!sheet || keys.length === 0 || isPreviewRuntime()) {
      setChecks(null);
      return;
    }
    const request = ++requestRef.current;
    const timer = window.setTimeout(() => {
      const candidateSheet: JsonTranslationSheet = {
        ...sheet,
        entries: sheet.entries
          .filter((entry) => candidates[entry.key])
          .map((entry) => ({ ...entry, translated_value: candidates[entry.key].value })),
      };
      invokeCommand("validate_json_translation_sheet_data", { sheet: candidateSheet, projectKey: projectKey || null })
        .then((result) => {
          if (request !== requestRef.current) {
            return;
          }
          const before = issueKindsByKey(validation);
          const after = new Map<string, JsonValidationIssue[]>();
          for (const issue of [...(result.format_issues ?? []), ...(result.glossary_issues ?? [])]) {
            after.set(issue.key, [...(after.get(issue.key) ?? []), issue]);
          }
          const next = new Map<string, CandidateCheck>();
          for (const key of keys) {
            const beforeKinds = before.get(key) ?? new Set<string>();
            const afterIssues = after.get(key) ?? [];
            next.set(key, { state: checkState(beforeKinds, afterIssues), after: afterIssues });
          }
          setChecks(next);
        })
        .catch(() => request === requestRef.current && setChecks(null));
    }, RECHECK_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [candidates, projectKey, sheet, validation]);

  return checks;
}

function issueKindsByKey(validation: JsonValidation | null) {
  const kinds = new Map<string, Set<string>>();
  for (const issue of [...(validation?.format_issues ?? []), ...(validation?.glossary_issues ?? [])]) {
    kinds.set(issue.key, (kinds.get(issue.key) ?? new Set()).add(issue.kind));
  }
  return kinds;
}

function checkState(before: Set<string>, after: JsonValidationIssue[]): CandidateCheckState {
  if (after.length === 0) {
    return before.size > 0 ? "fixed" : "clean";
  }
  return after.some((issue) => !before.has(issue.kind)) ? "new" : "still";
}
