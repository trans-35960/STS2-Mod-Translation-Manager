export type SourceDiffSegment = {
  kind: "same" | "removed" | "added";
  text: string;
};

// Markup tokens are kept whole so a changed tag or placeholder reads as one
// edit instead of a scatter of punctuation changes.
const TOKEN_PATTERN =
  /\[[^\[\]\s]{1,40}\]|\{[^{}]{0,80}\}|![A-Za-z0-9_]{1,40}!|<[^<>\s][^<>]{0,40}>|[+-]?\d+(?:\.\d+)?%?|[\p{L}\p{M}\p{N}_']+|\s+|[^]/gu;

/** Above this many token pairs the changed middle is shown as one replacement. */
const MAX_LCS_CELLS = 250_000;

export function tokenizeSource(value: string): string[] {
  return value.match(TOKEN_PATTERN) ?? [];
}

/**
 * Token-level diff between the source a translation was written against and
 * the current source, e.g. `Deal 5 damage.` → `Deal 8 damage.`.
 */
export function diffSourceValues(previous: string, current: string): SourceDiffSegment[] {
  if (previous === current) {
    return previous ? [{ kind: "same", text: previous }] : [];
  }
  const before = tokenizeSource(previous);
  const after = tokenizeSource(current);

  let prefix = 0;
  while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) {
    prefix += 1;
  }
  let suffix = 0;
  while (
    suffix < before.length - prefix &&
    suffix < after.length - prefix &&
    before[before.length - 1 - suffix] === after[after.length - 1 - suffix]
  ) {
    suffix += 1;
  }

  const segments: SourceDiffSegment[] = [];
  push(segments, "same", before.slice(0, prefix).join(""));
  for (const segment of diffMiddle(before.slice(prefix, before.length - suffix), after.slice(prefix, after.length - suffix))) {
    push(segments, segment.kind, segment.text);
  }
  push(segments, "same", before.slice(before.length - suffix).join(""));
  return absorbWhitespace(segments);
}

function diffMiddle(before: string[], after: string[]): SourceDiffSegment[] {
  const rows = before.length;
  const cols = after.length;
  if (rows === 0 || cols === 0 || rows * cols > MAX_LCS_CELLS) {
    return [
      { kind: "removed", text: before.join("") },
      { kind: "added", text: after.join("") },
    ];
  }

  // lengths[i][j] = LCS length of before[i..] and after[j..]
  const width = cols + 1;
  const lengths = new Uint32Array((rows + 1) * width);
  for (let i = rows - 1; i >= 0; i -= 1) {
    for (let j = cols - 1; j >= 0; j -= 1) {
      lengths[i * width + j] =
        before[i] === after[j]
          ? lengths[(i + 1) * width + j + 1] + 1
          : Math.max(lengths[(i + 1) * width + j], lengths[i * width + j + 1]);
    }
  }

  const segments: SourceDiffSegment[] = [];
  let i = 0;
  let j = 0;
  while (i < rows && j < cols) {
    if (before[i] === after[j]) {
      push(segments, "same", before[i]);
      i += 1;
      j += 1;
    } else if (lengths[(i + 1) * width + j] >= lengths[i * width + j + 1]) {
      push(segments, "removed", before[i]);
      i += 1;
    } else {
      push(segments, "added", after[j]);
      j += 1;
    }
  }
  for (; i < rows; i += 1) push(segments, "removed", before[i]);
  for (; j < cols; j += 1) push(segments, "added", after[j]);
  return segments;
}

/**
 * Folds whitespace that sits between two edits into both sides, so
 * `A B` → `C D` shows as one replacement instead of alternating fragments.
 */
function absorbWhitespace(segments: SourceDiffSegment[]): SourceDiffSegment[] {
  const output: SourceDiffSegment[] = [];
  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    const previous = output[output.length - 1];
    const next = segments[index + 1];
    if (segment.kind === "same" && /^\s+$/.test(segment.text) && previous && previous.kind !== "same" && next && next.kind !== "same") {
      push(output, "removed", segment.text);
      push(output, "added", segment.text);
      continue;
    }
    push(output, segment.kind, segment.text);
  }
  // Keep removals before additions inside each changed run.
  const ordered: SourceDiffSegment[] = [];
  let run: SourceDiffSegment[] = [];
  const flush = () => {
    for (const kind of ["removed", "added"] as const) {
      for (const item of run) {
        if (item.kind === kind) push(ordered, kind, item.text);
      }
    }
    run = [];
  };
  for (const segment of output) {
    if (segment.kind === "same") {
      flush();
      push(ordered, "same", segment.text);
    } else {
      run.push(segment);
    }
  }
  flush();
  return ordered;
}

function push(segments: SourceDiffSegment[], kind: SourceDiffSegment["kind"], text: string) {
  if (!text) {
    return;
  }
  const last = segments[segments.length - 1];
  if (last && last.kind === kind) {
    last.text += text;
  } else {
    segments.push({ kind, text });
  }
}
