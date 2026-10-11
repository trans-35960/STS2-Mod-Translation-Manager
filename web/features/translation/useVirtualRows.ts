import React from "react";

const DEFAULT_ROW_HEIGHT = 76;
const OVERSCAN_PX = 900;

type VirtualRange = {
  start: number;
  end: number;
  padTop: number;
  padBottom: number;
};

/**
 * Windowed rendering for a list of variable-height rows inside a scrolling
 * element. Only rows near the viewport are mounted; the rest are replaced by
 * top/bottom spacers sized from measured (or estimated) row heights.
 *
 * Rows report their size through `measureRow`, so heights stay correct when a
 * textarea grows, warnings appear, or columns are resized.
 */
export function useVirtualRows({
  scrollRef,
  listRef,
  keys,
}: {
  /** The element that scrolls (`overflow: auto`). */
  scrollRef: React.RefObject<HTMLElement | null>;
  /** The element that directly contains the rows and spacers. */
  listRef: React.RefObject<HTMLElement | null>;
  keys: string[];
}) {
  const heightsRef = React.useRef(new Map<string, number>());
  const elementKeysRef = React.useRef(new WeakMap<Element, string>());
  const [measureVersion, setMeasureVersion] = React.useState(0);
  const [viewport, setViewport] = React.useState({ top: 0, height: 900 });

  const offsets = React.useMemo(() => {
    const heights = heightsRef.current;
    const result = new Float64Array(keys.length + 1);
    for (let index = 0; index < keys.length; index += 1) {
      result[index + 1] = result[index] + (heights.get(keys[index]) ?? DEFAULT_ROW_HEIGHT);
    }
    return result;
    // measureVersion invalidates the offsets after rows report new heights.
  }, [keys, measureVersion]);

  const readViewport = React.useCallback(() => {
    const scroller = scrollRef.current;
    const list = listRef.current;
    if (!scroller || !list) {
      return;
    }
    const listTop = list.offsetTop;
    const top = Math.max(0, scroller.scrollTop - listTop);
    const height = scroller.clientHeight;
    setViewport((current) =>
      Math.abs(current.top - top) < 1 && current.height === height ? current : { top, height },
    );
  }, [listRef, scrollRef]);

  React.useEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) {
      return;
    }
    readViewport();
    // Chromium already delivers scroll events once per frame, and React batches
    // the state update, so no extra throttling is needed (a rAF-based throttle
    // would also stall while the window is hidden).
    scroller.addEventListener("scroll", readViewport, { passive: true });
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(readViewport);
    observer?.observe(scroller);
    return () => {
      scroller.removeEventListener("scroll", readViewport);
      observer?.disconnect();
    };
  }, [readViewport, scrollRef]);

  const rowObserver = React.useMemo(() => {
    if (typeof ResizeObserver === "undefined") {
      return null;
    }
    return new ResizeObserver((entries) => {
      let changed = false;
      for (const entry of entries) {
        const key = elementKeysRef.current.get(entry.target);
        if (!key) {
          continue;
        }
        const height = entry.borderBoxSize?.[0]?.blockSize ?? (entry.target as HTMLElement).offsetHeight;
        if (height > 0 && Math.abs((heightsRef.current.get(key) ?? -1) - height) > 0.5) {
          heightsRef.current.set(key, height);
          changed = true;
        }
      }
      // ResizeObserver batches all size changes of a frame into one callback.
      if (changed) {
        setMeasureVersion((version) => version + 1);
      }
    });
  }, []);

  React.useEffect(() => () => rowObserver?.disconnect(), [rowObserver]);

  const measureRow = React.useCallback(
    (key: string, element: HTMLElement | null) => {
      if (!element || !rowObserver) {
        return;
      }
      elementKeysRef.current.set(element, key);
      rowObserver.observe(element);
    },
    [rowObserver],
  );

  const unmeasureRow = React.useCallback(
    (element: HTMLElement | null) => {
      if (element && rowObserver) {
        rowObserver.unobserve(element);
      }
    },
    [rowObserver],
  );

  const range = React.useMemo((): VirtualRange => {
    const count = keys.length;
    if (count === 0) {
      return { start: 0, end: 0, padTop: 0, padBottom: 0 };
    }
    const from = Math.max(0, viewport.top - OVERSCAN_PX);
    const to = viewport.top + viewport.height + OVERSCAN_PX;
    const start = Math.min(count - 1, Math.max(0, upperBound(offsets, from) - 1));
    const end = Math.max(start + 1, Math.min(count, upperBound(offsets, to)));
    return {
      start,
      end,
      padTop: offsets[start],
      padBottom: offsets[count] - offsets[end],
    };
  }, [keys.length, offsets, viewport]);

  /** Scrolls so the row at `index` sits near the middle of the viewport. */
  const scrollToIndex = React.useCallback(
    (index: number) => {
      const scroller = scrollRef.current;
      const list = listRef.current;
      if (!scroller || !list || index < 0 || index >= keys.length) {
        return;
      }
      const rowTop = offsets[index];
      const rowHeight = offsets[index + 1] - rowTop;
      const target = list.offsetTop + rowTop - Math.max(0, (scroller.clientHeight - rowHeight) / 2);
      scroller.scrollTop = Math.max(0, target);
      readViewport();
    },
    [keys.length, listRef, offsets, readViewport, scrollRef],
  );

  return { range, measureRow, unmeasureRow, scrollToIndex };
}

/** First index whose offset is greater than `value`. */
function upperBound(offsets: Float64Array, value: number): number {
  let low = 0;
  let high = offsets.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (offsets[middle] <= value) {
      low = middle + 1;
    } else {
      high = middle;
    }
  }
  return low;
}
