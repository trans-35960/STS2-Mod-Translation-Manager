import React from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";
import { AppMenuBar, BusyProgressModal, LoadingScreen, StatsGrid } from "./components/AppShell";
import { TooltipLayer } from "./components/TooltipLayer";
import { LOADING_STEPS } from "./constants";
import { DroppedModConfirmModal, type DroppedModDecision } from "./features/mods/DroppedModConfirmModal";
import { ExtractConfirmModal, ModsPage } from "./features/mods/ModsPage";
import { ModConfirmModal } from "./features/mods/ModConfirmModal";
import { SettingsPage } from "./features/settings/SettingsPage";
import { LogToasts } from "./features/translation/LogToasts";
import { TranslationToolsPage } from "./features/translation/TranslationToolsPage";
import { useTranslationSessionState } from "./features/translation/useTranslationSessionState";
import { useAppActions } from "./hooks/useAppActions";
import { useAppLogs } from "./hooks/useAppLogs";
import { useDashboardLoader } from "./hooks/useDashboardLoader";
import { useModActions } from "./hooks/useModActions";
import { useSettingsActions } from "./hooks/useSettingsActions";
import { useTranslationActions } from "./hooks/useTranslationActions";
import { labels } from "./i18n";
import type {
  ActiveFilter,
  ChangeFilter,
  DashboardStatFilter,
  DroppedModPreview,
  Locale,
  ModRow,
  ModSort,
  Page,
  TranslationApplyFilter,
} from "./types";
import { getAppWindow, invokeCommand } from "./api/tauri";
import { isPreviewRuntime } from "./utils/runtime";

function App() {
  const [locale, setLocale] = React.useState<Locale>("ko");
  const [page, setPage] = React.useState<Page>("mods");
  const [busy, setBusy] = React.useState<string | null>(null);
  const [search, setSearch] = React.useState("");
  const [statFilter, setStatFilter] = React.useState<DashboardStatFilter>("all");
  const [activeFilter, setActiveFilter] = React.useState<ActiveFilter>("all");
  const [changeFilter, setChangeFilter] = React.useState<ChangeFilter>("all");
  const [translationApplyFilter, setTranslationApplyFilter] = React.useState<TranslationApplyFilter>("all");
  const [sort, setSort] = React.useState<ModSort>("name");
  const [focusedModKey, setFocusedModKey] = React.useState<string | null>(null);
  const [presetName, setPresetName] = React.useState("");
  const [archivePath, setArchivePath] = React.useState("");
  const [selectedPreset, setSelectedPreset] = React.useState("");
  const [dragActive, setDragActive] = React.useState(false);
  const [dropBusyMessage, setDropBusyMessage] = React.useState<string | null>(null);
  const [droppedModPreviews, setDroppedModPreviews] = React.useState<DroppedModPreview[] | null>(null);
  const { logs, setLogs, appendLog } = useAppLogs();
  const contentRef = React.useRef<HTMLElement | null>(null);
  const t = labels[locale];

  const {
    jsonSource,
    setJsonSource,
    jsonExistingSheet,
    setJsonExistingSheet,
    jsonOutputSheet,
    setJsonOutputSheet,
    jsonTranslatedOutput,
    setJsonTranslatedOutput,
    jsonPckTargetPath,
    setJsonPckTargetPath,
    jsonSheet,
    setJsonSheet,
    jsonReport,
    setJsonReport,
    jsonValidation,
    setJsonValidation,
    jsonApplyResult,
    setJsonApplyResult,
    jsonToolError,
    setJsonToolError,
    translationProject,
    setTranslationProject,
    jsonTargetLanguage,
    setJsonTargetLanguage,
    compareSamplePaths,
    setCompareSamplePaths,
    compareValuesByLanguage,
    setCompareValuesByLanguage,
    compareViewEnabled,
    setCompareViewEnabled,
    selectedRows,
    setSelectedRows,
    pasteCandidatesByKey,
    setPasteCandidatesByKey,
    clearStoredSession,
  } = useTranslationSessionState({ appendLog, setPage });

  const {
    dashboard,
    setDashboard,
    loading,
    loadingStep,
    loadingMessage,
    settingsDraft,
    setSettingsDraft,
    load,
  } = useDashboardLoader({
    appendLog,
    busy,
    selectedPreset,
    setPage,
    setSelectedPreset,
    setJsonTargetLanguage,
  });

  const { runAction, openPath } = useAppActions({
    appendLog,
    contentRef,
    page,
    selectedPreset,
    setDashboard,
    setSettingsDraft,
    setSelectedPreset,
    setJsonApplyResult,
    setBusy,
  });

  const {
    gameLogs,
    gameLogsLoading,
    saveSettings,
    loadGameLogs,
    chooseTranslationWorkDir,
    chooseGameExePath,
    chooseGameLogPath,
    chooseSaveDir,
    chooseSaveBackupDir,
    repairInstallations,
    restoreDeletedMod,
    emptyDeletedMods,
  } = useSettingsActions({
    appendLog,
    runAction,
    settingsDraft,
    setSettingsDraft,
  });

  const {
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
  } = useModActions({
    appendLog,
    dashboard,
    selectedPreset,
    runAction,
    setDashboard,
    setPage,
  });

  React.useEffect(() => {
    if (page === "settings" && gameLogs.length === 0 && !gameLogsLoading) {
      void loadGameLogs(false);
    }
  }, [gameLogs.length, gameLogsLoading, page]);

  const {
    applyAllPasteCandidates,
    applyPasteCandidate,
    applyTranslationSheet,
    closeTranslationSession,
    copySelectedTranslations,
    createTranslationSheet,
    dismissAllPasteCandidates,
    dismissPasteCandidate,
    exportTranslationCsv,
    exportTranslationPatchMod,
    exportTranslationShortJson,
    extractTreeNode,
    importTranslationValues,
    loadTranslationSheet,
    openModLanguageInTranslationTools,
    openTreeNodeInTranslationTools,
    pasteStructuredTranslationJson,
    pasteTranslationValues,
    replaceTranslationEntries,
    saveEditedTranslationSheet,
    selectTranslationRow,
    setTargetLanguage,
    switchTranslationSourceLanguage,
    toggleCompareLanguage,
    updateTranslationEntry,
    validateTranslationSheet,
  } = useTranslationActions({
    appendLog,
    busy,
    compareSamplePaths,
    compareValuesByLanguage,
    compareViewEnabled,
    dashboard,
    jsonApplyResult,
    jsonExistingSheet,
    jsonOutputSheet,
    jsonPckTargetPath,
    jsonSheet,
    jsonSource,
    jsonTargetLanguage,
    jsonTranslatedOutput,
    jsonValidation,
    load,
    page,
    pasteCandidatesByKey,
    selectedRows,
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
  });

  React.useEffect(() => {
    if (isPreviewRuntime()) {
      return;
    }
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void getAppWindow().onDragDropEvent((event) => {
      if (event.payload.type === "enter" || event.payload.type === "over") {
        setDragActive(true);
        return;
      }
      if (event.payload.type === "leave") {
        setDragActive(false);
        return;
      }
      if (event.payload.type === "drop") {
        setDragActive(false);
        setPage("mods");
        void previewDroppedPaths(event.payload.paths);
      }
    }).then((value) => {
      if (disposed) {
        value();
        return;
      }
      unlisten = value;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function previewDroppedPaths(paths: string[]) {
    if (paths.length === 0) {
      return;
    }
    setBusy("preview_dropped_mods");
    setDropBusyMessage("드롭한 모드 확인 중...");
    try {
      const previews = await invokeCommand<DroppedModPreview[]>("preview_dropped_mods", { paths });
      setDroppedModPreviews(previews);
      appendLog(`드롭한 모드 ${previews.length}개 확인 완료`);
    } catch (error) {
      appendLog(String(error));
    } finally {
      setBusy(null);
      setDropBusyMessage(null);
    }
  }

  async function confirmDroppedMods(decisions: DroppedModDecision[]) {
    setDroppedModPreviews(null);
    const importDecisions = decisions.filter((decision) => decision.mode !== "skip");
    let applied = 0;
    for (const [index, decision] of importDecisions.entries()) {
      setDropBusyMessage(`드롭한 모드 등록 중... (${index + 1}/${importDecisions.length})`);
      const result = await runAction("import_dropped_mod", {
        path: decision.path,
        replacePath: decision.mode === "replace" ? decision.replacePath : null,
      });
      if (result) {
        applied += 1;
      }
    }
    setDropBusyMessage(null);
    if (applied === 0) {
      appendLog("드롭한 모드 추가를 건너뛰었습니다.");
      return;
    }
    setSearch("");
    setActiveFilter("all");
    setChangeFilter("all");
    setTranslationApplyFilter("all");
    setSort("registered");
  }

  async function clearPendingExtractCache(mod: ModRow) {
    const result = await runAction("clear_translation_extract_cache", { key: mod.key });
    const updated = result?.dashboard.mods.find((item) => item.key === mod.key);
    if (updated) {
      setPendingExtract(updated);
    }
  }

  return (
    <div className="app-shell">
      {dragActive && (
        <div className="drop-overlay">
          <div>
            <strong>모드 추가</strong>
            <span>폴더, ZIP, 7Z, RAR, JAR, PCK 파일을 놓으세요.</span>
          </div>
        </div>
      )}
      <main className="workspace">
        <AppMenuBar
          labels={t}
          page={page}
          loading={loading}
          busy={busy}
          setPage={setPage}
          onRefresh={() => void refreshAll()}
        />

        {loading && <LoadingScreen step={loadingMessage || LOADING_STEPS[loadingStep]} />}
        {dropBusyMessage && <LoadingScreen step={dropBusyMessage} />}
        <BusyProgressModal busy={busy} />

        {dashboard && page === "mods" && (
          <StatsGrid
            dashboard={dashboard}
            labels={t}
            selected={statFilter}
            onSelect={(filter) => {
              setStatFilter(filter);
              if (filter === "changed") {
                setSort("change");
              }
            }}
          />
        )}

        <section className="content" ref={contentRef}>
          {!loading && dashboard && page === "mods" && (
              <ModsPage
                labels={t}
                mods={dashboard.mods}
                presets={dashboard.presets}
                search={search}
                statFilter={statFilter}
                activeFilter={activeFilter}
                changeFilter={changeFilter}
                translationApplyFilter={translationApplyFilter}
                sort={sort}
                selectedPreset={selectedPreset}
                presetName={presetName}
                archivePath={archivePath}
                targetLanguage={jsonTargetLanguage || settingsDraft?.target_language || dashboard.settings.target_language}
                vanillaSafe={dashboard.stats.vanilla_safe}
                launchRunning={dashboard.launch.running}
                busy={busy}
                togglingModKeys={togglingModKeys}
                initialSimpleView={dashboard.settings.mod_view_mode === "simple"}
                focusedModKey={focusedModKey}
                setSearch={setSearch}
                setStatFilter={setStatFilter}
                setActiveFilter={setActiveFilter}
                setChangeFilter={setChangeFilter}
                setTranslationApplyFilter={setTranslationApplyFilter}
                setSort={setSort}
                setSelectedPreset={setSelectedPreset}
                setPresetName={setPresetName}
                setArchivePath={setArchivePath}
                onToggle={(mod) => toggleModWithDependencies(mod)}
                onOpenPath={(path) => void openPath(path)}
                onDelete={(mod) => void deleteMod(mod)}
                onExtract={(mod) => setPendingExtract(baseModForTranslationPatch(mod, dashboard.mods))}
                onStartModTranslation={(mod, resourcePath) => void openModLanguageInTranslationTools(mod, resourcePath)}
                onSavePreset={() => runAction("save_preset", { name: presetName })}
                onApplyPreset={() => void applySelectedPresetWithPreview()}
                onExportPreset={() => runAction("export_preset", { name: selectedPreset, archivePath })}
                onImportPreset={() => runAction("import_preset_archive", { archivePath })}
                onLaunch={() => void launchWithSetupCheck("launch_current")}
                onVanilla={() => void launchWithSetupCheck("launch_vanilla")}
            />
          )}
          {!loading && dashboard && page === "translationTools" && (
            <TranslationToolsPage
              labels={t}
              settings={settingsDraft ?? dashboard.settings}
              sourcePath={jsonSource}
              existingSheetPath={jsonExistingSheet}
              outputSheetPath={jsonOutputSheet}
              translatedOutputPath={jsonTranslatedOutput}
              pckTargetPath={jsonPckTargetPath}
              sheet={jsonSheet}
              report={jsonReport}
              validation={jsonValidation}
              applyResult={jsonApplyResult}
              toolError={jsonToolError}
              projectInfo={translationProject}
              targetLanguage={jsonTargetLanguage}
              availableLanguages={translationProject?.languages ?? []}
              compareSamplePaths={compareSamplePaths}
              compareViewEnabled={compareViewEnabled}
              compareValuesByLanguage={compareValuesByLanguage}
              pasteCandidatesByKey={pasteCandidatesByKey}
              selectedRows={selectedRows}
              busy={busy}
              setSourcePath={setJsonSource}
              setExistingSheetPath={setJsonExistingSheet}
              setOutputSheetPath={setJsonOutputSheet}
              setTranslatedOutputPath={setJsonTranslatedOutput}
              setPckTargetPath={setJsonPckTargetPath}
              setTargetLanguage={setTargetLanguage}
              onEditEntry={updateTranslationEntry}
              onReplaceEntries={replaceTranslationEntries}
              onPasteEntries={pasteTranslationValues}
              onPasteStructuredJson={pasteStructuredTranslationJson}
              onApplyPasteCandidate={applyPasteCandidate}
              onApplyAllPasteCandidates={applyAllPasteCandidates}
              onDismissPasteCandidate={dismissPasteCandidate}
              onDismissAllPasteCandidates={dismissAllPasteCandidates}
              onSelectRow={selectTranslationRow}
              onCopySelected={copySelectedTranslations}
              onSave={saveEditedTranslationSheet}
              onExportCsv={exportTranslationCsv}
              onExportPatchMod={exportTranslationPatchMod}
              onExportShortJson={exportTranslationShortJson}
              onImportValues={importTranslationValues}
              onToggleCompareLanguage={toggleCompareLanguage}
              onToggleCompareView={() => setCompareViewEnabled((value) => !value)}
              onSwitchSourceLanguage={(samplePath) => void switchTranslationSourceLanguage(samplePath)}
              onOpenPath={(path) => void openPath(path)}
              onCloseSession={closeTranslationSession}
              onCreate={() => void createTranslationSheet()}
              onLoad={() => void loadTranslationSheet()}
              onValidate={validateTranslationSheet}
              onApply={() => {
                const appliedModKey = translationProject?.modKey ?? null;
                void applyTranslationSheet().then((applied) => {
                  if (!applied || !appliedModKey) {
                    return;
                  }
    setSearch("");
    setStatFilter("all");
    setActiveFilter("all");
                  setChangeFilter("all");
                  setTranslationApplyFilter("all");
                  setFocusedModKey(appliedModKey);
                  setPage("mods");
                });
              }}
            />
          )}
          {!loading && dashboard && page === "settings" && (
            <SettingsPage
              labels={t}
              dashboard={dashboard}
              draft={settingsDraft ?? dashboard.settings}
              setupIssues={dashboard.setup_issues}
              diagnostics={dashboard.diagnostics}
              logs={logs}
              gameLogs={gameLogs}
              gameLogsLoading={gameLogsLoading}
              setDraft={setSettingsDraft}
              onChooseTranslationWorkDir={chooseTranslationWorkDir}
              onChooseGameExePath={chooseGameExePath}
              onChooseGameLogPath={chooseGameLogPath}
              onChooseSaveDir={chooseSaveDir}
              onChooseSaveBackupDir={chooseSaveBackupDir}
              onSave={saveSettings}
              onRefreshGameLogs={() => void loadGameLogs()}
              onOpenPath={(path) => void openPath(path)}
              onRepairInstallations={() => void repairInstallations()}
              onCleanupCaches={() => runAction("cleanup_orphan_caches")}
              onRestoreDeleted={(item) => void restoreDeletedMod(item)}
              onEmptyDeleted={() => void emptyDeletedMods()}
              onCreateSaveBackup={() => runAction("create_save_backup")}
              onRestoreSaveBackup={(item) => runAction("restore_save_backup", { id: item.id })}
              onDeleteSaveBackups={(items) => runAction("delete_save_backups", { ids: items.map((item) => item.id) })}
              busy={busy}
              locale={locale}
              setLocale={setLocale}
            />
          )}
        </section>
      </main>
      {pendingExtract && (
        <ExtractConfirmModal
          labels={t}
          mod={pendingExtract}
          busy={busy}
          outputDir={extractOutputDir || dashboard?.paths.translation_work || ""}
          setOutputDir={setExtractOutputDir}
          onChooseOutputDir={chooseExtractOutputDir}
          onCancel={() => setPendingExtract(null)}
          onClearCache={() => void clearPendingExtractCache(pendingExtract)}
          onConfirm={(force) => {
            const mod = pendingExtract;
            const outputDir = extractOutputDir || dashboard?.paths.translation_work || "";
            setPendingExtract(null);
            void runAction("extract_translation", { key: mod.key, outputDir, force });
          }}
          onExtractNode={(node) => {
            const mod = pendingExtract;
            setPendingExtract(null);
            void extractTreeNode(mod, node, extractOutputDir || dashboard?.paths.translation_work);
          }}
          onOpenNodeTools={(node) => void openTreeNodeInTranslationTools(pendingExtract, node, extractOutputDir || undefined)}
        />
      )}
      {droppedModPreviews && dashboard && (
        <DroppedModConfirmModal
          items={droppedModPreviews}
          mods={dashboard.mods}
          busy={busy}
          onCancel={() => setDroppedModPreviews(null)}
          onConfirm={(decisions) => void confirmDroppedMods(decisions)}
        />
      )}
      {modConfirmDialog && <ModConfirmModal dialog={modConfirmDialog} onAnswer={answerModConfirm} />}
      <LogToasts
        logs={logs.slice(0, 5)}
        onDismiss={(index) => setLogs((items) => items.filter((_, itemIndex) => itemIndex !== index))}
        applyResult={jsonApplyResult}
        onDismissApply={() => setJsonApplyResult(null)}
      />
      <TooltipLayer />
    </div>
  );
}


const rootElement = document.getElementById("root")!;
const rootStore = globalThis as typeof globalThis & {
  __sts2ModManagerRoot?: ReturnType<typeof ReactDOM.createRoot>;
};
const root = rootStore.__sts2ModManagerRoot ?? ReactDOM.createRoot(rootElement);
rootStore.__sts2ModManagerRoot = root;

root.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

function baseModForTranslationPatch(mod: ModRow, mods: ModRow[]): ModRow {
  if (!mod.is_translation_patch) {
    return mod;
  }
  if (mod.translation_target_key) {
    const byKey = mods.find((item) => item.key === mod.translation_target_key);
    if (byKey) {
      return byKey;
    }
  }
  const targetToken = normalizeModToken(mod.translation_target_id || mod.translation_target_name || "");
  if (targetToken) {
    const byTarget = mods.find((item) => !item.is_translation_patch && (normalizeModToken(item.key) === targetToken || normalizeModToken(item.name) === targetToken));
    if (byTarget) {
      return byTarget;
    }
  }
  return mod;
}

function normalizeModToken(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9가-힣]/g, "");
}
