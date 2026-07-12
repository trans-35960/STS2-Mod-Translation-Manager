import React from "react";
import {
  Languages,
  Maximize2,
  Minimize2,
  Minus,
  RefreshCw,
  Settings,
  SquareStack,
  X,
  type LucideIcon,
} from "lucide-react";
import appIconUrl from "../assets/app-icon.png";
import { getAppWindow } from "../api/tauri";
import { labels } from "../i18n";
import type { Dashboard, DashboardStatFilter, Locale, Page, TranslationPreparationProgress } from "../types";
import { isPreviewRuntime } from "../utils/runtime";
export function LoadingScreen({ step }: { step: string }) {
  return (
    <div className="loading-screen" role="status" aria-live="polite">
      <div className="loading-card">
        <div className="loading-icon-wrap" aria-hidden="true">
          <span className="loading-icon-glow" />
          <img src={appIconUrl} alt="" className="loading-app-icon" />
        </div>
        <div className="loading-copy">
          <strong>STS2 Mod Manager</strong>
          <span>{step}</span>
        </div>
        <div className="loading-progress" aria-hidden="true">
          <span />
        </div>
      </div>
    </div>
  );
}

export function BusyProgressModal({ busy, liveProgress }: { busy: string | null; liveProgress?: TranslationPreparationProgress | null }) {
  const [elapsedSeconds, setElapsedSeconds] = React.useState(0);
  React.useEffect(() => {
    if (!busy) {
      setElapsedSeconds(0);
      return;
    }
    const startedAt = window.performance.now();
    setElapsedSeconds(0);
    const intervalId = window.setInterval(() => {
      setElapsedSeconds(Math.floor((window.performance.now() - startedAt) / 1000));
    }, 1000);
    return () => window.clearInterval(intervalId);
  }, [busy]);

  const progress = busyProgress(busy, liveProgress);
  if (!progress) {
    return null;
  }
  const isLiveTranslationProgress = Boolean(liveProgress && busy?.startsWith("prepare_translation_node"));
  return (
    <div className="busy-progress-backdrop" role="presentation">
      <section className="busy-progress-modal" role="status" aria-live="polite" aria-label={progress.title}>
        <header>
          <strong>{progress.title}</strong>
          <span>{progress.detail}</span>
        </header>
        <div className="busy-progress-bar" aria-hidden="true">
          <span />
        </div>
        {isLiveTranslationProgress && liveProgress && (
          <div className="busy-progress-meta">
            <span>{progressPhaseLabel(liveProgress.phase)}</span>
            <span>경과 {formatElapsed(elapsedSeconds)}</span>
            {liveProgress.source_bytes > 0 && <span>원본 {formatProgressBytes(liveProgress.source_bytes)}</span>}
            {liveProgress.cache_hit !== null && (
              <span>{liveProgress.cache_hit ? "캐시 재사용" : "캐시 생성 중"}</span>
            )}
          </div>
        )}
        <ol className="busy-progress-steps">
          {progress.steps.map((step, index) => (
            <li className={index < progress.activeStep ? "completed" : index === progress.activeStep ? "active current" : ""} key={step}>
              <span>{index + 1}</span>
              <p>{step}</p>
            </li>
          ))}
        </ol>
      </section>
    </div>
  );
}

function busyProgress(busy: string | null, liveProgress?: TranslationPreparationProgress | null): { title: string; detail: string; steps: string[]; activeStep: number } | null {
  if (!busy) {
    return null;
  }
  if (busy === "clear_translation_extract_cache") {
    return {
      title: "추출 캐시 삭제 중",
      detail: "이전 PCK/아카이브 분석 결과를 지우고 있습니다.",
      steps: ["대상 모드 확인", "캐시 폴더 삭제", "모드 목록 새로고침"],
      activeStep: 1,
    };
  }
  if (busy === "clear_current_runs") {
    return {
      title: "진행 중 런 정리 중",
      detail: "current_run.save를 백업 후 제거하고 Steam Cloud 캐시 기록을 정리합니다.",
      steps: ["로컬 current_run 확인", "Steam remote 파일 확인", "remotecache.vdf 정리", "문제 해결 목록 새로고침"],
      activeStep: 2,
    };
  }
  if (busy === "repair_mod_installations") {
    return {
      title: "모드 설치 정리 중",
      detail: "압축 파일과 잘못 배치된 모드 항목을 정리하고 목록을 다시 읽습니다.",
      steps: ["모드 경로 확인", "복구/정리 적용", "모드 목록 새로고침"],
      activeStep: 1,
    };
  }
  if (busy === "extract_translation") {
    return {
      title: "파일 추출 중",
      detail: "화면 조작을 잠시 막고 선택한 모드 파일을 새 작업 폴더로 복사합니다.",
      steps: ["대상 모드 확인", "PCK/아카이브 분석", "선택 파일 복사", "모드 목록 갱신"],
      activeStep: 2,
    };
  }
  if (busy.startsWith("prepare_translation_node")) {
    return {
      title: "번역 작업 준비 중",
      detail: liveProgress?.detail ?? "선택한 항목을 번역 도구에서 바로 열 수 있도록 작업 파일과 시트를 준비합니다.",
      steps: ["대상 모드 확인", "PCK 분석 및 추출 캐시 준비", "원본/대상 언어 파일 복사", "번역 시트 생성", "번역 도구 열기"],
      activeStep: liveProgress?.step ?? 0,
    };
  }
  return null;
}

export function AppMenuBar(props: {
  labels: typeof labels.ko;
  page: Page;
  loading: boolean;
  busy: string | null;
  setPage: (page: Page) => void;
  onRefresh: () => void;
}) {
  const { labels: t } = props;
  const refreshLoading = props.loading || props.busy === "scan_updates";
  return (
    <header className="app-menu-bar">
      <div className="app-menu-left">
        <div className="menu-brand" title="Slay the Spire 2 Mod Manager">
          <img src={appIconUrl} alt="" className="menu-brand-icon" />
          <strong>STS2 Mod Manager</strong>
        </div>
        <nav className="menu-page-tabs" aria-label="Primary">
          <MenuTab icon={SquareStack} label={t.mods} active={props.page === "mods"} onClick={() => props.setPage("mods")} />
          <MenuTab icon={Languages} label={t.translationTools} active={props.page === "translationTools"} onClick={() => props.setPage("translationTools")} />
          <MenuTab icon={Settings} label={t.settings} active={props.page === "settings"} onClick={() => props.setPage("settings")} />
        </nav>
        <button
          className={`menu-icon-button refresh-button${refreshLoading ? " loading" : ""}`}
          type="button"
          onClick={props.onRefresh}
          disabled={props.loading || Boolean(props.busy)}
          aria-label={refreshLoading ? "새로고침 중" : t.refresh}
          data-tooltip={refreshLoading ? "새로고침 중" : t.refresh}
          aria-busy={refreshLoading}
        >
          <RefreshCw size={15} />
        </button>
      </div>
      {refreshLoading && <span className="menu-loading-strip" aria-hidden="true" />}
      <div
        className="titlebar-spacer"
        data-tauri-drag-region
        onDoubleClick={() => void toggleTitlebarMaximize()}
      />
      <WindowControls />
    </header>
  );
}

function progressPhaseLabel(phase: TranslationPreparationProgress["phase"]): string {
  return ({
    source_check: "리소스 확인",
    cache_hit: "추출 캐시 확인 완료",
    extracting: "PCK 압축 해제",
    selecting_files: "번역 파일 탐색",
    copying_files: "작업 파일 복사",
    source_ready: "원본 준비 완료",
    creating_sheet: "번역 시트 생성",
    opening_tools: "편집기 표시",
  })[phase];
}

function formatElapsed(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}초`;
  }
  return `${Math.floor(seconds / 60)}분 ${seconds % 60}초`;
}

function formatProgressBytes(bytes: number): string {
  if (bytes < 1024 * 1024) {
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(bytes >= 100 * 1024 * 1024 ? 0 : 1)} MB`;
}

async function toggleTitlebarMaximize() {
  if (isPreviewRuntime()) {
    return;
  }
  await getAppWindow().toggleMaximize();
}

function MenuTab(props: { icon: LucideIcon; label: string; active: boolean; onClick: () => void }) {
  const Icon = props.icon;
  return (
    <button
      className={props.active ? "menu-tab active" : "menu-tab"}
      type="button"
      onClick={props.onClick}
      aria-label={props.label}
      data-tooltip={props.label}
    >
      <Icon size={15} />
      <span>{props.label}</span>
    </button>
  );
}

function WindowControls() {
  const [maximized, setMaximized] = React.useState(false);
  const syncMaximized = React.useCallback(() => {
    if (isPreviewRuntime()) {
      return;
    }
    getAppWindow()
      .isMaximized()
      .then(setMaximized)
      .catch(() => undefined);
  }, []);

  React.useEffect(() => {
    if (isPreviewRuntime()) {
      return;
    }
    const appWindow = getAppWindow();
    syncMaximized();
    let unlistenResize: (() => void) | null = null;
    let unlistenMove: (() => void) | null = null;
    void appWindow.onResized(syncMaximized).then((unlisten) => {
      unlistenResize = unlisten;
    });
    void appWindow.onMoved(syncMaximized).then((unlisten) => {
      unlistenMove = unlisten;
    });
    return () => {
      unlistenResize?.();
      unlistenMove?.();
    };
  }, [syncMaximized]);

  const maximizeLabel = maximized ? "이전 크기로 복원" : "최대화";
  const toggleMaximize = async () => {
    await runWindowAction("toggleMaximize");
    syncMaximized();
  };
  return (
    <div className="window-controls">
      <button type="button" className="window-control" onClick={() => void runWindowAction("minimize")} aria-label="최소화" data-tooltip="최소화">
        <Minus size={14} />
      </button>
      <button type="button" className="window-control" onClick={() => void toggleMaximize()} aria-label={maximizeLabel} data-tooltip={maximizeLabel}>
        {maximized ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
      </button>
      <button type="button" className="window-control close" onClick={() => void runWindowAction("close")} aria-label="닫기" data-tooltip="닫기">
        <X size={15} />
      </button>
    </div>
  );
}

async function runWindowAction(action: "minimize" | "toggleMaximize" | "close") {
  if (isPreviewRuntime()) {
    return;
  }
  const appWindow = getAppWindow();
  if (action === "minimize") {
    await appWindow.minimize();
    return;
  }
  if (action === "toggleMaximize") {
    await appWindow.toggleMaximize();
    return;
  }
  await appWindow.close();
}

export function Segmented(props: { value: Locale; onChange: (value: Locale) => void; disabled?: boolean }) {
  return (
    <div className="segmented">
      <button className={props.value === "ko" ? "selected" : ""} onClick={() => props.onChange("ko")} disabled={props.disabled}>한국어</button>
      <button className={props.value === "en" ? "selected" : ""} onClick={() => props.onChange("en")} disabled={props.disabled}>English</button>
    </div>
  );
}

export function StatsGrid({
  dashboard,
  labels: t,
  selected,
  onSelect,
}: {
  dashboard: Dashboard;
  labels: typeof labels.ko;
  selected: DashboardStatFilter;
  onSelect: (filter: DashboardStatFilter) => void;
}) {
  const stats = dashboard.stats;
  function toggle(filter: DashboardStatFilter) {
    onSelect(selected === filter ? "all" : filter);
  }
  return (
    <section className="stats-grid">
      <Stat label={t.active} value={stats.active_mods} tone={stats.vanilla_safe ? "good" : "warn"} active={selected === "active"} onClick={() => toggle("active")} />
      <Stat label={t.inactive} value={stats.inactive_mods} tone={stats.inactive_mods > 0 ? "good" : undefined} active={selected === "inactive"} onClick={() => toggle("inactive")} />
      <Stat label={t.external} value={stats.external_mods} active={selected === "external"} onClick={() => toggle("external")} />
      <Stat
        label={t.changes}
        value={stats.detected_changes}
        tone={stats.detected_changes > 0 ? "warn" : "good"}
        active={selected === "changed"}
        onClick={() => toggle("changed")}
      />
    </section>
  );
}

export function Stat({
  label,
  value,
  tone,
  active = false,
  onClick,
  detail,
}: {
  label: string;
  value: string | number;
  detail?: string;
  tone?: "good" | "warn";
  active?: boolean;
  onClick?: () => void;
}) {
  const className = `stat ${tone ?? ""}${active ? " active" : ""}${onClick ? " clickable" : ""}`;
  const content = (
    <>
      <span>{label}</span>
      <strong>{value}</strong>
      {detail && <small>{detail}</small>}
    </>
  );
  if (onClick) {
    return (
      <button className={className} type="button" onClick={onClick} title={`${label} 항목 보기${detail ? ` · ${detail}` : ""}`} aria-pressed={active}>
        {content}
      </button>
    );
  }
  return (
    <div className={className}>
      {content}
    </div>
  );
}
