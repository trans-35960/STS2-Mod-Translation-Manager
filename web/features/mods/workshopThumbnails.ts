import React from "react";

/**
 * Steam Workshop thumbnails (`data:` URLs) by item id. They arrive in the
 * background after the dashboard loads, so they live outside the dashboard
 * state and survive its reloads.
 */
let thumbnails: Record<string, string> = {};
const listeners = new Set<() => void>();

export function addWorkshopThumbnails(next: Record<string, string>) {
  const changed = Object.entries(next).filter(([itemId, url]) => thumbnails[itemId] !== url);
  if (changed.length === 0) {
    return;
  }
  thumbnails = { ...thumbnails, ...Object.fromEntries(changed) };
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useWorkshopThumbnail(itemId: string | null | undefined): string | null {
  return React.useSyncExternalStore(subscribe, () => (itemId ? thumbnails[itemId] ?? null : null));
}
