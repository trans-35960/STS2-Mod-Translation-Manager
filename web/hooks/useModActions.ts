import React from "react";
import { openDialog } from "../api/tauri";
import {
  activeSiblingMods,
  isDownloadingMod,
  modGroupName,
  presetPreviewSummary,
} from "../features/mods/modUtils";
import type { Dashboard, ModRow, Page } from "../types";
import { isPreviewRuntime } from "../utils/runtime";
import { blockingSetupIssues, setupIssueSummary } from "../utils/setup";

type RunActionResult = { dashboard: Dashboard; message: string } | null;
type RunAction = (command: string, args?: Record<string, unknown>) => Promise<RunActionResult>;
type ToggleChange = { key: string; active: boolean; force?: boolean };
export type ModConfirmAnswer = "cancel" | "secondary" | "confirm";
export type ModConfirmDialogState = {
  title: string;
  body: string[];
  items: string[];
  confirmLabel: string;
  cancelLabel: string;
  secondaryLabel?: string;
};

export function useModActions({
  appendLog,
  dashboard,
  selectedPreset,
  runAction,
  setDashboard,
  setPage,
}: {
  appendLog: (message: string) => void;
  dashboard: Dashboard | null;
  selectedPreset: string;
  runAction: RunAction;
  setDashboard: React.Dispatch<React.SetStateAction<Dashboard | null>>;
  setPage: React.Dispatch<React.SetStateAction<Page>>;
}) {
  const [pendingExtract, setPendingExtract] = React.useState<ModRow | null>(null);
  const [extractOutputDir, setExtractOutputDir] = React.useState("");
  const [togglingModKeys, setTogglingModKeys] = React.useState<Set<string>>(() => new Set());
  const [modConfirmDialog, setModConfirmDialog] = React.useState<ModConfirmDialogState | null>(null);
  const confirmResolverRef = React.useRef<((value: ModConfirmAnswer) => void) | null>(null);
  const queuedToggleChangesRef = React.useRef<Map<string, ToggleChange>>(new Map());
  const queuedToggleLabelsRef = React.useRef<string[]>([]);
  const toggleRollbackDashboardRef = React.useRef<Dashboard | null>(null);
  const toggleFlushTimerRef = React.useRef<number | null>(null);
  const toggleFlushRunningRef = React.useRef(false);

  React.useEffect(() => () => {
    confirmResolverRef.current?.("cancel");
    if (toggleFlushTimerRef.current !== null) {
      window.clearTimeout(toggleFlushTimerRef.current);
    }
  }, []);

  async function refreshAll() {
    await runAction("scan_updates");
  }

  async function launchWithSetupCheck(command: "launch_current" | "launch_vanilla") {
    if (dashboard?.launch.running) {
      window.alert("게임이 이미 실행 중입니다. 게임을 종료한 뒤 다시 실행하세요.");
      return;
    }
    const issues = blockingSetupIssues(dashboard);
    if (issues.length > 0) {
      setPage("settings");
      appendLog(`실행 전 설정 필요: ${setupIssueSummary(issues)}`);
      return;
    }
    if (command === "launch_current") {
      const backupCount = dashboard?.save_backups.length ?? 0;
      if (backupCount === 0) {
        const confirmed = window.confirm(
          [
            "현재 모드로 실행하기 전에 안전 상태를 확인하세요.",
            backupCount === 0 ? "아직 복원 가능한 세이브 백업이 없습니다." : `복원 가능한 세이브 백업 ${backupCount}개가 있습니다.`,
            "",
            "계속 실행할까요?",
          ].filter(Boolean).join("\n"),
        );
        if (!confirmed) {
          setPage("settings");
          return;
        }
      }
    }
    setPage("mods");
    await runAction(command);
  }

  async function toggleModWithDependencies(mod: ModRow) {
    const mods = dashboard?.mods ?? [];
    if (togglingModKeys.has(mod.key)) {
      return;
    }
    if (!mod.active && isDownloadingMod(mod)) {
      window.alert("Vortex 다운로드가 완료된 뒤 활성화할 수 있습니다.");
      return;
    }
    if (mod.active) {
      const activeDependents = collectActiveDependents(mods, mod);
      if (activeDependents.length > 0) {
        const disableAnswer = await askModConfirm({
          title: "연결된 모드 비활성화",
          body: [
            `${mod.name} 모드를 비활성화하면 아래 활성 모드의 선행 조건이 꺼집니다.`,
            "해당 모드들도 함께 비활성화할까요?",
            "선택한 모드만 끄면 남은 모드에는 선행 모드 경고가 표시됩니다.",
          ],
          items: activeDependents.map((item) => item.name),
          confirmLabel: "함께 비활성화",
          cancelLabel: "선택한 모드만 비활성화",
        });
        if (disableAnswer === "confirm") {
          await runToggleBatch(
            [...activeDependents].reverse().map((dependent) => ({ key: dependent.key, active: true })).concat({ key: mod.key, active: true }),
            `${mod.name}와 연결된 하위 모드 비활성화`,
          );
          return;
        }
      }
      await runToggleBatch([{ key: mod.key, active: true }], `${mod.name} 비활성화`);
      return;
    }
    const missing = collectUnavailableDependencies(mods, mod);
    if (missing.length > 0) {
      window.alert(`선행 모드가 없습니다: ${missing.map((dependency) => dependency.id).join(", ")}`);
      return;
    }
    const inactive = collectInactiveDependencyClosure(mods, mod);
    if (inactive.length > 0) {
      const enableAnswer = await askModConfirm({
        title: "선행 모드 활성화",
        body: [
          `${mod.name} 모드에 비활성 선행 모드가 있습니다.`,
          "선행 모드도 함께 활성화할까요?",
        ],
        items: inactive.map((dependency) => dependency.name),
        confirmLabel: "함께 활성화",
        cancelLabel: "취소",
        secondaryLabel: "단독 활성화",
      });
      if (enableAnswer === "cancel") {
        return;
      }
      if (enableAnswer === "secondary") {
        const activeSiblings = activeSiblingMods(mods, mod);
        await runToggleBatch(
          [
            ...activeSiblings.map((sibling) => ({ key: sibling.key, active: true })),
            { key: mod.key, active: false, force: true },
          ],
          `${mod.name} 단독 활성화`,
        );
        return;
      }
    }
    const inactiveTranslations = collectInactiveTranslationPatches(mods, mod);
    let translationsToEnable: ModRow[] = [];
    if (inactiveTranslations.length > 0) {
      const translationAnswer = await askModConfirm({
        title: "번역 모드 활성화",
        body: [
          `${mod.name} 원본 모드에 연결된 번역 모드가 있습니다.`,
          "번역 모드도 함께 활성화할까요?",
        ],
        items: inactiveTranslations.map(translationActivationLabel),
        confirmLabel: "함께 활성화",
        cancelLabel: "취소",
        secondaryLabel: "원본만 활성화",
      });
      if (translationAnswer === "cancel") {
        return;
      }
      if (translationAnswer === "confirm") {
        translationsToEnable = inactiveTranslations;
      }
    }
    const activeSiblings = activeSiblingMods(mods, mod);
    const changes = [
      ...inactive.map((dependency) => ({ key: dependency.key!, active: false })),
      ...activeSiblings.map((sibling) => ({ key: sibling.key, active: true })),
      { key: mod.key, active: false },
      ...translationsToEnable.map((translation) => ({ key: translation.key, active: false })),
    ];
    await runToggleBatch(changes, `${mod.name} 활성화`);
  }

  async function deleteMod(mod: ModRow) {
    await runAction("delete_mod", { key: mod.key, path: mod.path });
  }

  async function applySelectedPresetWithPreview() {
    if (!dashboard || !selectedPreset) {
      return;
    }
    const preset = dashboard.presets.find((item) => item.name === selectedPreset);
    if (!preset) {
      appendLog(`프리셋을 찾지 못했습니다: ${selectedPreset}`);
      return;
    }
    const summary = presetPreviewSummary(preset, dashboard.mods);
    const confirmed = window.confirm(summary);
    if (!confirmed) {
      return;
    }
    await runAction("apply_preset", { name: selectedPreset });
  }

  async function chooseExtractOutputDir() {
    if (isPreviewRuntime()) {
      setExtractOutputDir(dashboard?.paths.translation_work ?? "translation_work");
      return;
    }
    const selected = await openDialog({
      title: "추출 대상 폴더",
      directory: true,
      multiple: false,
    });
    if (typeof selected === "string") {
      setExtractOutputDir(selected);
    }
  }

  return {
    pendingExtract,
    setPendingExtract,
    extractOutputDir,
    setExtractOutputDir,
    refreshAll,
    launchWithSetupCheck,
    toggleModWithDependencies,
    deleteMod,
    applySelectedPresetWithPreview,
    chooseExtractOutputDir,
    togglingModKeys,
    modConfirmDialog,
    answerModConfirm,
  };

  function askModConfirm(request: ModConfirmDialogState): Promise<ModConfirmAnswer> {
    confirmResolverRef.current?.("cancel");
    setModConfirmDialog(request);
    return new Promise((resolve) => {
      confirmResolverRef.current = resolve;
    });
  }

  function answerModConfirm(value: ModConfirmAnswer) {
    const resolve = confirmResolverRef.current;
    confirmResolverRef.current = null;
    setModConfirmDialog(null);
    resolve?.(value);
  }

  async function runToggleBatch(changes: ToggleChange[], label: string) {
    const uniqueChanges = dedupeToggleChanges(changes);
    if (uniqueChanges.length === 0) {
      return;
    }
    if (!toggleRollbackDashboardRef.current) {
      toggleRollbackDashboardRef.current = dashboard;
    }
    setDashboard((current) => applyOptimisticToggleChanges(current, uniqueChanges));
    setTogglingModKeys((current) => {
      const next = new Set(current);
      for (const change of uniqueChanges) {
        next.add(change.key);
      }
      return next;
    });
    for (const change of uniqueChanges) {
      queuedToggleChangesRef.current.set(change.key, change);
    }
    queuedToggleLabelsRef.current.push(label);
    scheduleToggleFlush();
  }

  function scheduleToggleFlush() {
    if (toggleFlushTimerRef.current !== null) {
      window.clearTimeout(toggleFlushTimerRef.current);
    }
    toggleFlushTimerRef.current = window.setTimeout(() => {
      toggleFlushTimerRef.current = null;
      void flushQueuedToggleChanges();
    }, 220);
  }

  async function flushQueuedToggleChanges() {
    if (toggleFlushRunningRef.current) {
      scheduleToggleFlush();
      return;
    }
    const uniqueChanges = Array.from(queuedToggleChangesRef.current.values());
    if (uniqueChanges.length === 0) {
      return;
    }
    queuedToggleChangesRef.current.clear();
    const labels = queuedToggleLabelsRef.current.splice(0);
    toggleFlushRunningRef.current = true;
    try {
      const command = uniqueChanges.length === 1 ? "toggle_mod" : "toggle_mods";
      const args = uniqueChanges.length === 1 ? uniqueChanges[0] : { changes: uniqueChanges };
      const result = await runAction(command, args);
      if (!result) {
        setDashboard(toggleRollbackDashboardRef.current);
        appendLog(`${labels[labels.length - 1] ?? "모드 전환"} 실패`);
        queuedToggleChangesRef.current.clear();
        queuedToggleLabelsRef.current = [];
        toggleRollbackDashboardRef.current = null;
        setTogglingModKeys(new Set());
      } else if (queuedToggleChangesRef.current.size > 0) {
        setDashboard((current) => applyOptimisticToggleChanges(current, Array.from(queuedToggleChangesRef.current.values())));
      } else {
        toggleRollbackDashboardRef.current = null;
      }
    } finally {
      setTogglingModKeys((current) => {
        const next = new Set(current);
        for (const change of uniqueChanges) {
          next.delete(change.key);
        }
        return next;
      });
      toggleFlushRunningRef.current = false;
      if (queuedToggleChangesRef.current.size > 0) {
        scheduleToggleFlush();
      }
    }
  }
}

function applyOptimisticToggleChanges(dashboard: Dashboard | null, changes: ToggleChange[]): Dashboard | null {
  if (!dashboard) {
    return dashboard;
  }
  const targetActiveByKey = new Map(changes.map((change) => [change.key, !change.active]));
  const toggledMods = dashboard.mods.map((mod) => {
    const targetActive = targetActiveByKey.get(mod.key);
    return targetActive === undefined ? mod : { ...mod, active: targetActive };
  });
  const activeByKey = new Map(toggledMods.map((mod) => [mod.key, mod.active]));
  const mods = toggledMods.map((mod) => ({
    ...mod,
    dependencies: mod.dependencies.map((dependency) => {
      if (!dependency.key || !activeByKey.has(dependency.key)) {
        return dependency;
      }
      return { ...dependency, active: Boolean(activeByKey.get(dependency.key)) };
    }),
  }));
  const activeCount = mods.filter((mod) => mod.active).length;
  return {
    ...dashboard,
    mods,
    stats: {
      ...dashboard.stats,
      active_mods: activeCount,
      inactive_mods: mods.length - activeCount,
      vanilla_safe: activeCount === 0,
    },
  };
}

function collectActiveDependents(mods: ModRow[], target: ModRow): ModRow[] {
  const output: ModRow[] = [];
  const visited = new Set<string>([target.key]);
  const queue = [target];
  while (queue.length > 0) {
    const current = queue.shift();
    if (!current) {
      continue;
    }
    const dependents = mods.filter((mod) => mod.active && !visited.has(mod.key) && mod.dependencies.some((dependency) => dependencyTargetsMod(dependency, current)));
    for (const dependent of dependents) {
      visited.add(dependent.key);
      output.push(dependent);
      queue.push(dependent);
    }
  }
  return output;
}

function collectInactiveDependencyClosure(mods: ModRow[], target: ModRow): ModRow["dependencies"] {
  const output: ModRow["dependencies"] = [];
  const added = new Set<string>();
  const visiting = new Set<string>();

  function visit(mod: ModRow) {
    if (visiting.has(mod.key)) {
      return;
    }
    visiting.add(mod.key);
    for (const dependency of mod.dependencies) {
      if (!dependency.available || dependency.active || !dependency.key || added.has(dependency.key)) {
        continue;
      }
      const dependencyMod = mods.find((item) => item.key === dependency.key);
      if (dependencyMod) {
        visit(dependencyMod);
      }
      added.add(dependency.key);
      output.push(dependency);
    }
    visiting.delete(mod.key);
  }

  visit(target);
  return output;
}

function collectUnavailableDependencies(mods: ModRow[], target: ModRow): ModRow["dependencies"] {
  const output: ModRow["dependencies"] = [];
  const added = new Set<string>();
  const visiting = new Set<string>();

  function visit(mod: ModRow) {
    if (visiting.has(mod.key)) {
      return;
    }
    visiting.add(mod.key);
    for (const dependency of mod.dependencies) {
      if (!dependency.available) {
        const id = dependency.id || dependency.name;
        if (!added.has(id)) {
          added.add(id);
          output.push(dependency);
        }
        continue;
      }
      if (dependency.key) {
        const dependencyMod = mods.find((item) => item.key === dependency.key);
        if (dependencyMod) {
          visit(dependencyMod);
        }
      }
    }
    visiting.delete(mod.key);
  }

  visit(target);
  return output;
}

function dedupeToggleChanges(changes: ToggleChange[]): ToggleChange[] {
  const seen = new Set<string>();
  const output: ToggleChange[] = [];
  for (const change of changes) {
    if (seen.has(change.key)) {
      continue;
    }
    seen.add(change.key);
    output.push(change);
  }
  return output;
}

function dependencyTargetsMod(dependency: ModRow["dependencies"][number], mod: ModRow): boolean {
  if (dependency.key) {
    return dependency.key === mod.key;
  }
  const dependencyToken = normalizeDependencyToken(dependency.id || dependency.name);
  return dependencyToken === normalizeDependencyToken(mod.key) || dependencyToken === normalizeDependencyToken(mod.name);
}

function collectInactiveTranslationPatches(mods: ModRow[], target: ModRow): ModRow[] {
  return mods.filter((mod) => !mod.active && mod.key !== target.key && mod.is_translation_patch && translationTargetsMod(mod, target));
}

function translationTargetsMod(translation: ModRow, target: ModRow): boolean {
  if (translation.translation_target_key) {
    return translation.translation_target_key === target.key;
  }
  const targetId = normalizeDependencyToken(translation.translation_target_id ?? "");
  if (targetId && (targetId === normalizeDependencyToken(target.key) || targetId === normalizeDependencyToken(target.name))) {
    return true;
  }
  return translation.dependencies.some((dependency) => dependencyTargetsMod(dependency, target));
}

function translationActivationLabel(mod: ModRow): string {
  const versionWarning = mod.dependencies.find((dependency) => dependency.version_matches === false);
  if (!versionWarning) {
    return mod.name;
  }
  return `${mod.name} (원본 ${versionWarning.version_current ?? "-"} / 번역 기준 ${versionWarning.version_required ?? "-"})`;
}

function normalizeDependencyToken(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9가-힣]/g, "");
}
