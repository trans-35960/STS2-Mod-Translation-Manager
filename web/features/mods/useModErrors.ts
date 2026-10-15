import React from "react";
import { invokeCommand } from "../../api/tauri";
import { previewModErrors } from "../../previewData";
import type { ModError, ModErrorReport, ModRow } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import { modErrorTokens } from "./modUtils";

/** How often the log is re-read while the game runs. */
const RUNNING_POLL_MS = 15_000;

export type ModErrorState = {
  report: ModErrorReport | null;
  /** Errors per mod key. */
  byMod: Map<string, ModError[]>;
  unattributed: ModError[];
  loading: boolean;
  error: string | null;
  reload: () => void;
};

/**
 * Errors of the latest game run, traced to mods. Read when the list opens,
 * when the game starts or stops (its log is complete then), and every few
 * seconds while it runs.
 */
export function useModErrors(mods: ModRow[], launchRunning: boolean): ModErrorState {
  const [report, setReport] = React.useState<ModErrorReport | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const identities = React.useMemo(
    () => mods.map((mod) => ({ key: mod.key, active: mod.active, tokens: modErrorTokens(mod) })),
    [mods],
  );
  // Identities change on every dashboard reload; the log is re-read on its own schedule.
  const identitiesRef = React.useRef(identities);
  identitiesRef.current = identities;
  const identitySignature = React.useMemo(() => identities.map((item) => `${item.key}:${item.active}:${item.tokens.join("|")}`).join("\n"), [identities]);

  const reload = React.useCallback(() => {
    if (isPreviewRuntime()) {
      setReport(previewModErrors);
      return;
    }
    setLoading(true);
    invokeCommand("read_mod_errors", { mods: identitiesRef.current })
      .then((next) => {
        setReport(next);
        setError(null);
      })
      .catch((reason) => setError(String(reason)))
      .finally(() => setLoading(false));
  }, []);

  React.useEffect(() => {
    reload();
  }, [reload, launchRunning, identitySignature]);

  React.useEffect(() => {
    if (!launchRunning) {
      return;
    }
    const timer = window.setInterval(reload, RUNNING_POLL_MS);
    return () => window.clearInterval(timer);
  }, [launchRunning, reload]);

  const { byMod, unattributed } = React.useMemo(() => {
    const grouped = new Map<string, ModError[]>();
    const loose: ModError[] = [];
    for (const item of report?.errors ?? []) {
      if (!item.mod_key) {
        loose.push(item);
        continue;
      }
      grouped.set(item.mod_key, [...(grouped.get(item.mod_key) ?? []), item]);
    }
    return { byMod: grouped, unattributed: loose };
  }, [report]);

  return { report, byMod, unattributed, loading, error, reload };
}

export function errorOccurrences(errors: ModError[]): number {
  return errors.reduce((sum, item) => sum + item.count, 0);
}
