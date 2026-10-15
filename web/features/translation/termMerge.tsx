import React from "react";
import type { GlossaryTerm } from "../../types";

/** A pasted term whose source is already registered with another translation. */
export type TermConflict = { source: string; current: string[]; next: string[] };

export type TermConflictChoice = "next" | "keep" | "both";

export type TermMergeResult = {
  terms: GlossaryTerm[];
  added: number;
  /** Registered terms that had no translation yet and got one. */
  filled: number;
  same: number;
  conflicts: TermConflict[];
};

/**
 * Adds terms to a list right away: new sources are appended, a source still
 * without a translation is filled, and the same translation is skipped. A
 * different translation is left out and returned as a conflict to ask about.
 */
export function mergeTerms(existing: GlossaryTerm[], incoming: GlossaryTerm[]): TermMergeResult {
  const terms = existing.map((term) => ({ ...term }));
  const indexBySource = new Map(terms.map((term, index) => [term.source.toLowerCase(), index]));
  const result: TermMergeResult = { terms, added: 0, filled: 0, same: 0, conflicts: [] };
  for (const term of incoming) {
    const key = term.source.toLowerCase();
    const index = indexBySource.get(key);
    if (index === undefined) {
      indexBySource.set(key, terms.length);
      terms.push({ source: term.source, targets: [...term.targets] });
      result.added += 1;
      continue;
    }
    const current = terms[index];
    if (term.targets.length === 0 || term.targets.every((target) => current.targets.includes(target))) {
      result.same += 1;
    } else if (current.targets.length === 0) {
      terms[index] = { ...current, targets: [...term.targets] };
      result.filled += 1;
    } else {
      result.conflicts.push({ source: current.source, current: current.targets, next: term.targets });
    }
  }
  return result;
}

export function resolveTermConflicts(terms: GlossaryTerm[], conflicts: TermConflict[], choice: TermConflictChoice): GlossaryTerm[] {
  if (choice === "keep") {
    return terms;
  }
  const bySource = new Map(conflicts.map((conflict) => [conflict.source.toLowerCase(), conflict]));
  return terms.map((term) => {
    const conflict = bySource.get(term.source.toLowerCase());
    if (!conflict) {
      return term;
    }
    const targets = choice === "next" ? conflict.next : [...new Set([...term.targets, ...conflict.next])];
    return { ...term, targets };
  });
}

export function termMergeSummary(result: TermMergeResult) {
  const parts = [
    result.added > 0 ? `${result.added}개 추가` : "",
    result.filled > 0 ? `${result.filled}개 번역 채움` : "",
    result.same > 0 ? `${result.same}개 같아서 건너뜀` : "",
    result.conflicts.length > 0 ? `${result.conflicts.length}개는 번역이 달라 확인 필요` : "",
  ].filter(Boolean);
  return parts.length > 0 ? `용어 ${parts.join(" · ")}` : "추가할 용어가 없습니다.";
}

/**
 * Terms pasted with a translation different from the registered one: each
 * can take the new translation, keep the current one, or keep both.
 */
export function TermConflictList({
  conflicts,
  onResolve,
}: {
  conflicts: TermConflict[];
  /** `sources` null resolves every conflict. */
  onResolve: (sources: string[] | null, choice: TermConflictChoice) => void;
}) {
  if (conflicts.length === 0) {
    return null;
  }
  return (
    <div className="term-conflicts" role="group" aria-label="번역이 다른 용어">
      <div className="term-conflicts-head">
        <strong>이미 있는 용어 {conflicts.length}개의 번역이 다릅니다</strong>
        <button type="button" onClick={() => onResolve(null, "next")}>모두 새 번역으로</button>
        <button type="button" onClick={() => onResolve(null, "keep")}>모두 기존 유지</button>
      </div>
      {conflicts.map((conflict) => (
        <div className="term-conflict" key={conflict.source}>
          <b title={conflict.source}>{conflict.source}</b>
          <span>
            <del>{conflict.current.join(" / ")}</del>
            {" → "}
            <ins>{conflict.next.join(" / ")}</ins>
          </span>
          <div>
            <button type="button" onClick={() => onResolve([conflict.source], "next")}>새 번역</button>
            <button type="button" onClick={() => onResolve([conflict.source], "keep")}>기존 유지</button>
            <button type="button" title="두 번역을 모두 허용합니다 (검증에서 둘 다 맞는 번역으로 봅니다)." onClick={() => onResolve([conflict.source], "both")}>
              둘 다
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}

/** Pending conflicts for a term list, resolved against its latest terms. */
export function useTermConflicts(terms: GlossaryTerm[], onChange: (terms: GlossaryTerm[]) => void) {
  const [conflicts, setConflicts] = React.useState<TermConflict[]>([]);

  function add(incoming: GlossaryTerm[]): TermMergeResult {
    const result = mergeTerms(terms, incoming);
    if (result.added + result.filled > 0) {
      onChange(result.terms);
    }
    const fresh = new Set(result.conflicts.map((conflict) => conflict.source.toLowerCase()));
    setConflicts((current) => [...current.filter((conflict) => !fresh.has(conflict.source.toLowerCase())), ...result.conflicts]);
    return result;
  }

  function resolve(sources: string[] | null, choice: TermConflictChoice) {
    const picked = conflicts.filter((conflict) => sources === null || sources.includes(conflict.source));
    if (choice !== "keep") {
      onChange(resolveTermConflicts(terms, picked, choice));
    }
    setConflicts((current) => current.filter((conflict) => !picked.includes(conflict)));
  }

  return { conflicts, add, resolve, clear: () => setConflicts([]) };
}
