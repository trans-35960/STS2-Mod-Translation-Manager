import React from "react";
import { Pill } from "../../components/Common";
import type {
  ApplyResultState,
  LanguagePreview,
  PasteCandidate,
} from "../../types";
import { languageFolderCode } from "../mods/modUtils";

export function ApplyStatusToast({ result, onDismiss }: { result: ApplyResultState; onDismiss: () => void }) {
  const packed = Boolean(result.packed_pck_path || result.installed_mod_path);
  const failed = Boolean(result.error) || result.applied_entries === 0;
  return (
    <article className={`log-toast apply-toast ${packed && !failed ? "good" : "warn"}`}>
      <div>
        <strong>{result.message}</strong>
        {result.language_output_path && <p>언어 폴더: {result.language_output_path}</p>}
        {result.packed_pck_path && <p>PCK: {result.packed_pck_path}</p>}
        {result.installed_mod_path && <p>활성 모드: {result.installed_mod_path}</p>}
        {failed && <small>번역값이 없으면 적용된 PCK를 만들지 않습니다. translated_value를 채운 뒤 다시 실행해 주세요.</small>}
        {!failed && !packed && <small>PCK 원본을 찾지 못해 JSON만 저장했습니다. 경로/고급 설정의 출력 경로와 원본 PCK를 확인해 주세요.</small>}
        {!failed && packed && result.installed_mod_path && <small>패치된 PCK가 활성 모드 위치에 반영되었습니다. 새로고침하면 실제 파일트리에 대상 언어가 보여야 합니다.</small>}
        {!failed && packed && !result.installed_mod_path && <small>생성된 PCK는 출력 경로에 있습니다. 모드 폴더에 등록되지 않은 형식이라 직접 배치가 필요합니다.</small>}
      </div>
      <button type="button" onClick={onDismiss} aria-label="적용 결과 닫기">
        닫기
      </button>
    </article>
  );
}

export function CompareStack({ items }: { items: Array<{ id: string; code: string; value: string }> }) {
  if (items.length === 0) {
    return <span className="compare-empty">-</span>;
  }
  return (
    <div className="compare-stack">
      {items.map((item) => (
        <span title={item.value || "값 없음"} key={item.id}>
          <b>{item.code}</b>
          <em>{item.value || "-"}</em>
        </span>
      ))}
    </div>
  );
}

export function PasteCandidateCard({
  candidate,
  onApply,
  onDismiss,
}: {
  candidate: PasteCandidate;
  onApply: () => void;
  onDismiss: () => void;
}) {
  return (
    <div className="paste-candidate-card" onClick={(event) => event.stopPropagation()}>
      <div>
        <strong>붙여넣기 충돌</strong>
        <small>{candidate.source}</small>
      </div>
      <p>{candidate.value || "-"}</p>
      <div>
        <button type="button" onClick={onApply}>허가</button>
        <button type="button" onClick={onDismiss}>취소</button>
      </div>
    </div>
  );
}

export function ProjectSummary({
  title,
  version,
  author,
  targetLanguage,
  languages,
  description,
}: {
  title: string;
  version?: string;
  author?: string;
  targetLanguage?: string;
  languages: LanguagePreview[];
  description?: string;
}) {
  return (
    <section className="project-summary">
      <span>번역 중</span>
      <h2>{title}</h2>
      <div>
        <Pill>{version || "-"}</Pill>
        {author && <Pill>{author}</Pill>}
        {targetLanguage && <Pill>{targetLanguage}</Pill>}
        {languages.slice(0, 4).map((language) => (
          <Pill key={`${language.code}-${language.sample_path}`}>{languageFolderCode(language)}</Pill>
        ))}
      </div>
      {description && <p>{description}</p>}
    </section>
  );
}

export function ResizableHead({ label, onMouseDown }: { label: string; onMouseDown: (event: React.MouseEvent) => void }) {
  return (
    <span className="resizable-head">
      {label}
      <button type="button" aria-label={`${label} column resize`} onMouseDown={onMouseDown} />
    </span>
  );
}

export function AutoGrowTextarea(props: React.TextareaHTMLAttributes<HTMLTextAreaElement>) {
  const ref = React.useRef<HTMLTextAreaElement | null>(null);
  const [focused, setFocused] = React.useState(false);

  const resize = React.useCallback(() => {
    const textarea = ref.current;
    if (!textarea) {
      return;
    }
    textarea.style.height = "auto";
    textarea.style.height = `${textarea.scrollHeight}px`;
  }, []);

  React.useLayoutEffect(() => {
    if (focused) {
      resize();
    }
  }, [focused, props.value, resize]);

  React.useEffect(() => {
    const textarea = ref.current;
    if (!focused) {
      return;
    }
    if (!textarea || typeof ResizeObserver === "undefined") {
      return;
    }
    const observer = new ResizeObserver(resize);
    observer.observe(textarea);
    return () => observer.disconnect();
  }, [resize]);

  return (
    <textarea
      {...props}
      ref={ref}
      rows={focused ? 1 : 2}
      onBlur={(event) => {
        setFocused(false);
        props.onBlur?.(event);
      }}
      onFocus={(event) => {
        setFocused(true);
        props.onFocus?.(event);
        window.requestAnimationFrame(resize);
      }}
      onInput={(event) => {
        props.onInput?.(event);
        if (focused) {
          resize();
        }
      }}
    />
  );
}
