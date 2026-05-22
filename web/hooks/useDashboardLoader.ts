import React from "react";
import { invokeCommand } from "../api/tauri";
import { LOADING_STEPS } from "../constants";
import { previewDashboard } from "../previewData";
import type { Dashboard, Page, UiSettings } from "../types";
import { isPreviewRuntime } from "../utils/runtime";
import { blockingSetupIssues } from "../utils/setup";

const DASHBOARD_POLL_INTERVAL_MS = 30000;
const DASHBOARD_POLL_WHILE_GAME_RUNNING_MS = 3000;

export function useDashboardLoader({
  appendLog,
  busy,
  selectedPreset,
  setPage,
  setSelectedPreset,
  setJsonTargetLanguage,
}: {
  appendLog: (message: string) => void;
  busy: string | null;
  selectedPreset: string;
  setPage: React.Dispatch<React.SetStateAction<Page>>;
  setSelectedPreset: React.Dispatch<React.SetStateAction<string>>;
  setJsonTargetLanguage: React.Dispatch<React.SetStateAction<string>>;
}) {
  const [dashboard, setDashboard] = React.useState<Dashboard | null>(null);
  const [loading, setLoading] = React.useState(true);
  const [loadingStep, setLoadingStep] = React.useState(0);
  const [loadingMessage, setLoadingMessage] = React.useState(LOADING_STEPS[0]);
  const [settingsDraft, setSettingsDraft] = React.useState<UiSettings | null>(null);
  const pollRunningRef = React.useRef(false);

  const refreshDashboard = React.useCallback(async () => {
    if (isPreviewRuntime() || busy || loading || pollRunningRef.current) {
      return;
    }
    pollRunningRef.current = true;
    try {
      const data = await invokeCommand("load_dashboard");
      setDashboard(data);
      setSettingsDraft((current) => current ?? data.settings);
    } catch {
      // The regular action log already surfaces explicit command failures.
    } finally {
      pollRunningRef.current = false;
    }
  }, [busy, loading]);

  const load = React.useCallback(async () => {
    const startedAt = window.performance.now();
    setLoading(true);
    setLoadingMessage("초기 설정을 확인하고 있습니다");
    try {
      if (isPreviewRuntime()) {
        setDashboard(previewDashboard);
        setSettingsDraft(previewDashboard.settings);
        setJsonTargetLanguage((current) => current || previewDashboard.settings.target_language || "kor");
        setSelectedPreset(previewDashboard.presets[0]?.name ?? "");
        setLoadingMessage("브라우저 미리보기 데이터를 준비했습니다");
        appendLog("Browser preview mode: Tauri commands are replaced with sample data.");
        return;
      }
      setLoadingMessage("시스템 경로를 확인하고 있습니다");
      const data = await invokeCommand("load_dashboard");
      setLoadingMessage("세이브 백업 경로와 실행 준비 상태를 확인하고 있습니다");
      setDashboard(data);
      setSettingsDraft(data.settings);
      setJsonTargetLanguage((current) => current || data.settings.target_language || "kor");
      if (!selectedPreset && data.presets[0]) {
        setSelectedPreset(data.presets[0].name);
      }
      const blockingIssues = blockingSetupIssues(data);
      if (blockingIssues.length > 0) {
        setLoadingMessage("초기 설정 입력이 필요합니다. 설정 화면으로 이동합니다");
        setPage("settings");
        appendLog(`초기 설정 필요: ${blockingIssues.map((issue) => issue.message).join(" / ")}`);
      } else {
        setLoadingMessage("인터페이스를 정리하는 중");
      }
    } catch (error) {
      if (isPreviewRuntime()) {
        setDashboard(previewDashboard);
        setSettingsDraft(previewDashboard.settings);
        setJsonTargetLanguage((current) => current || previewDashboard.settings.target_language || "kor");
        setSelectedPreset(previewDashboard.presets[0]?.name ?? "");
        setLoadingMessage("브라우저 미리보기 데이터를 준비했습니다");
        appendLog("Browser preview mode: Tauri commands are replaced with sample data.");
      } else {
        appendLog(String(error));
      }
    } finally {
      const minimumLoadingMs = isPreviewRuntime() ? 120 : 650;
      const remaining = Math.max(0, minimumLoadingMs - (window.performance.now() - startedAt));
      if (remaining > 0) {
        await new Promise((resolve) => window.setTimeout(resolve, remaining));
      }
      setLoading(false);
    }
  }, [appendLog, selectedPreset, setJsonTargetLanguage, setPage, setSelectedPreset]);

  React.useEffect(() => {
    void load();
  }, [load]);

  React.useEffect(() => {
    if (!loading) {
      setLoadingStep(LOADING_STEPS.length - 1);
      return;
    }
    setLoadingStep(0);
    const intervalId = window.setInterval(() => {
      setLoadingStep((step) => Math.min(step + 1, LOADING_STEPS.length - 1));
    }, 720);
    return () => window.clearInterval(intervalId);
  }, [loading]);

  React.useEffect(() => {
    if (isPreviewRuntime()) {
      return;
    }
    const pollIntervalMs = dashboard?.launch.running
      ? DASHBOARD_POLL_WHILE_GAME_RUNNING_MS
      : DASHBOARD_POLL_INTERVAL_MS;
    const intervalId = window.setInterval(async () => {
      if (document.visibilityState !== "visible") {
        return;
      }
      await refreshDashboard();
    }, pollIntervalMs);
    return () => window.clearInterval(intervalId);
  }, [dashboard?.launch.running, refreshDashboard]);

  React.useEffect(() => {
    if (isPreviewRuntime()) {
      return;
    }
    const refreshWhenVisible = () => {
      if (document.visibilityState === "visible") {
        void refreshDashboard();
      }
    };
    document.addEventListener("visibilitychange", refreshWhenVisible);
    window.addEventListener("focus", refreshWhenVisible);
    return () => {
      document.removeEventListener("visibilitychange", refreshWhenVisible);
      window.removeEventListener("focus", refreshWhenVisible);
    };
  }, [refreshDashboard]);

  return {
    dashboard,
    setDashboard,
    loading,
    loadingStep,
    loadingMessage,
    settingsDraft,
    setSettingsDraft,
    load,
    refreshDashboard,
  };
}
