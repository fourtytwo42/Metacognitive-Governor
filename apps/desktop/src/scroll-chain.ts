const SCROLL_CHAIN_ATTRIBUTE = "data-scroll-chain";

export function wheelDeltaToPixels(deltaY: number, deltaMode: number, pageSize: number): number {
  if (deltaMode === 1) return deltaY * 16;
  if (deltaMode === 2) return deltaY * Math.max(1, pageSize);
  return deltaY;
}

export function scrollChainByDelta(start: HTMLElement, deltaY: number): number {
  let remaining = deltaY;
  let current: HTMLElement | null = start;

  while (current && Math.abs(remaining) > 0.01) {
    if (current.hasAttribute(SCROLL_CHAIN_ATTRIBUTE)) {
      const overflowY = getComputedStyle(current).overflowY;
      if (overflowY === "hidden" || overflowY === "clip") {
        current = current.parentElement;
        continue;
      }
      const maximum = Math.max(0, current.scrollHeight - current.clientHeight);
      const before = Math.min(maximum, Math.max(0, current.scrollTop));
      const after = Math.min(maximum, Math.max(0, before + remaining));
      current.scrollTop = after;
      remaining -= after - before;
    }
    current = current.parentElement;
  }

  return remaining;
}
