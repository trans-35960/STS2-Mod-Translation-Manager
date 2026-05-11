import React from "react";
import ReactDOM from "react-dom";

type TooltipState = {
  text: string;
  anchor: DOMRect;
};

type TooltipPosition = {
  left: number;
  top: number;
  placement: "top" | "bottom";
};

const VIEWPORT_PADDING = 8;
const ANCHOR_GAP = 8;

export function TooltipLayer() {
  const [tooltip, setTooltip] = React.useState<TooltipState | null>(null);
  const [position, setPosition] = React.useState<TooltipPosition | null>(null);
  const tooltipRef = React.useRef<HTMLDivElement | null>(null);

  React.useEffect(() => {
    function showForTarget(target: EventTarget | null) {
      const element = tooltipElement(target);
      const text = element?.getAttribute("data-tooltip")?.trim();
      if (!element || !text) {
        setTooltip(null);
        return;
      }
      setTooltip({ text, anchor: element.getBoundingClientRect() });
    }

    function hideForTarget(target: EventTarget | null, relatedTarget: EventTarget | null) {
      const element = tooltipElement(target);
      if (!element || (relatedTarget instanceof Node && element.contains(relatedTarget))) {
        return;
      }
      setTooltip(null);
    }

    function hide() {
      setTooltip(null);
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        hide();
      }
    }

    const onPointerOver = (event: PointerEvent) => showForTarget(event.target);
    const onFocusIn = (event: FocusEvent) => showForTarget(event.target);
    const onPointerOut = (event: PointerEvent) => hideForTarget(event.target, event.relatedTarget);
    const onFocusOut = (event: FocusEvent) => hideForTarget(event.target, event.relatedTarget);

    document.addEventListener("pointerover", onPointerOver, true);
    document.addEventListener("focusin", onFocusIn, true);
    document.addEventListener("pointerout", onPointerOut, true);
    document.addEventListener("focusout", onFocusOut, true);
    window.addEventListener("scroll", hide, true);
    window.addEventListener("resize", hide);
    window.addEventListener("keydown", onKeyDown);

    return () => {
      document.removeEventListener("pointerover", onPointerOver, true);
      document.removeEventListener("focusin", onFocusIn, true);
      document.removeEventListener("pointerout", onPointerOut, true);
      document.removeEventListener("focusout", onFocusOut, true);
      window.removeEventListener("scroll", hide, true);
      window.removeEventListener("resize", hide);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  React.useLayoutEffect(() => {
    if (!tooltip || !tooltipRef.current) {
      setPosition(null);
      return;
    }
    const rect = tooltipRef.current.getBoundingClientRect();
    const centered = tooltip.anchor.left + tooltip.anchor.width / 2 - rect.width / 2;
    const left = clamp(centered, VIEWPORT_PADDING, window.innerWidth - rect.width - VIEWPORT_PADDING);
    const topCandidate = tooltip.anchor.top - rect.height - ANCHOR_GAP;
    if (topCandidate >= VIEWPORT_PADDING) {
      setPosition({ left, top: topCandidate, placement: "top" });
      return;
    }
    setPosition({
      left,
      top: clamp(tooltip.anchor.bottom + ANCHOR_GAP, VIEWPORT_PADDING, window.innerHeight - rect.height - VIEWPORT_PADDING),
      placement: "bottom",
    });
  }, [tooltip]);

  if (!tooltip) {
    return null;
  }

  return ReactDOM.createPortal(
    <div
      ref={tooltipRef}
      className={`floating-tooltip ${position?.placement ?? "bottom"}`}
      role="tooltip"
      style={{
        left: position?.left ?? 0,
        top: position?.top ?? 0,
        opacity: position ? 1 : 0,
      }}
    >
      {tooltip.text}
    </div>,
    document.body,
  );
}

function tooltipElement(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof Element)) {
    return null;
  }
  return target.closest<HTMLElement>("[data-tooltip]");
}

function clamp(value: number, min: number, max: number): number {
  if (max < min) {
    return min;
  }
  return Math.min(Math.max(value, min), max);
}
