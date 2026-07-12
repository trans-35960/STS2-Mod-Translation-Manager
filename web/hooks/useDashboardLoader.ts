import React from "react";
import { invokeCommand } from "../api/tauri";
import { LOADING_STEPS } from "../constants";
import { previewDashboard } from "../previewData";
import type { Dashboard, Page, UiSettings } from "../types";
import { formatCommandError } from "../utils/logging";
import { isPreviewRuntime } from "../utils/runtime";
import { blockingSetupIssues } from "../utils/setup";

const DASHBOARD_POLL_INTERVAL_MS = 30000;
const LAUNCH_STATUS_POLL_INTERVAL_MS = 3000;

export function useDashboardLoader({
  appendLog,
  busy,
  setPage,
  setSelectedPreset,
  setJsonTargetLanguage,
}: {
  appendLog: (message: string) => void;
  busy: string | null;
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
    const startedAt = window.performance.now();
    try {
      const data = await invokeCommand("load_dashboard");
      const commandMs = Math.round(window.performance.now() - startedAt);
      setDashboard((current) =>
        current ? { ...data, cache_usage: current.cache_usage } : data,
      );
      setSettingsDraft((current) => current ?? data.settings);
      if (commandMs >= 500) {
        appendLog(`[perf] 대시보드 새로고침 ${commandMs}ms · 모드 ${data.mods.length}개 · 번역작업 ${data.translations.length}개`);
      }
    } catch (error) {
      appendLog(formatCommandError("load_dashboard", undefined, error));
    } finally {
      pollRunningRef.current = false;
    }
  }, [appendLog, busy, loading]);

  const refreshLaunchStatus = React.useCallback(async () => {
    if (isPreviewRuntime() || busy || loading) {
      return;
    }
    try {
      const launch = await invokeCommand("load_launch_status");
      setDashboard((current) => {
        if (
          !current ||
          (current.launch.ready === launch.ready &&
            current.launch.game_exe === launch.game_exe &&
            current.launch.steam_exe === launch.steam_exe &&
            current.launch.target_label === launch.target_label &&
            current.launch.running === launch.running)
        ) {
          return current;
        }
        return { ...current, launch };
      });
    } catch (error) {
      appendLog(formatCommandError("load_launch_status", undefined, error));
    }
  }, [appendLog, busy, loading]);

  const load = React.useCallback(async () => {
    const startedAt = window.performance.now();
    setLoading(true);
    setLoadingMessage("초기 설정을 확인하고 있습니다");
    try {
      if (isPreviewRuntime()) {
        setDashboard(previewDashboard);
        setSettingsDraft(previewDashboard.settings);
        setJsonTargetLanguage((current) => current || previewDashboard.settings.target_language || "kor");
        setSelectedPreset((current) => current || previewDashboard.presets[0]?.name || "");
        setLoadingMessage("브라우저 미리보기 데이터를 준비했습니다");
        appendLog("Browser preview mode: Tauri commands are replaced with sample data.");
        return;
      }
      setLoadingMessage("시스템 경로를 확인하고 있습니다");
      const data = await invokeCommand("load_dashboard");
      const commandMs = Math.round(window.performance.now() - startedAt);
      setLoadingMessage("세이브 백업 경로와 실행 준비 상태를 확인하고 있습니다");
      setDashboard(data);
      setSettingsDraft(data.settings);
      appendLog(`[perf] 초기 대시보드 로드 ${commandMs}ms · 모드 ${data.mods.length}개 · 프리셋 ${data.presets.length}개`);
      setJsonTargetLanguage((current) => current || data.settings.target_language || "kor");
      setSelectedPreset((current) => current || data.presets[0]?.name || "");
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
        setSelectedPreset((current) => current || previewDashboard.presets[0]?.name || "");
        setLoadingMessage("브라우저 미리보기 데이터를 준비했습니다");
        appendLog("Browser preview mode: Tauri commands are replaced with sample data.");
      } else {
        appendLog(formatCommandError("load_dashboard", undefined, error));
      }
    } finally {
      setLoading(false);
    }
  }, [appendLog, setJsonTargetLanguage, setPage, setSelectedPreset]);

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
    const intervalId = window.setInterval(async () => {
      if (document.visibilityState !== "visible") {
        return;
      }
      await refreshDashboard();
    }, DASHBOARD_POLL_INTERVAL_MS);
    return () => window.clearInterval(intervalId);
  }, [refreshDashboard]);

  React.useEffect(() => {
    if (isPreviewRuntime()) {
      return;
    }
    const intervalId = window.setInterval(() => {
      if (document.visibilityState === "visible") {
        void refreshLaunchStatus();
      }
    }, LAUNCH_STATUS_POLL_INTERVAL_MS);
    return () => window.clearInterval(intervalId);
  }, [refreshLaunchStatus]);

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
