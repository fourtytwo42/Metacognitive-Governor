import { expect, test } from "vitest";
import { scrollChainByDelta, wheelDeltaToPixels } from "./scroll-chain";

function scrollViewport(parent: HTMLElement | null, scrollTop: number, clientHeight: number, scrollHeight: number) {
  const viewport = document.createElement("div");
  viewport.setAttribute("data-scroll-chain", "");
  let position = scrollTop;
  Object.defineProperties(viewport, {
    clientHeight: { configurable: true, get: () => clientHeight },
    scrollHeight: { configurable: true, get: () => scrollHeight },
    scrollTop: {
      configurable: true,
      get: () => position,
      set: (value: number) => { position = Math.min(scrollHeight - clientHeight, Math.max(0, value)); }
    }
  });
  parent?.appendChild(viewport);
  return viewport;
}

test("nested wheel movement continues through every scrollable ancestor", () => {
  const outer = scrollViewport(null, 40, 100, 300);
  const middle = scrollViewport(outer, 80, 100, 200);
  const inner = scrollViewport(middle, 90, 100, 200);

  expect(scrollChainByDelta(inner, 170)).toBe(0);
  expect(inner.scrollTop).toBe(100);
  expect(middle.scrollTop).toBe(100);
  expect(outer.scrollTop).toBe(180);

  expect(scrollChainByDelta(inner, 80)).toBe(60);
  expect(inner.scrollTop).toBe(100);
  expect(middle.scrollTop).toBe(100);
  expect(outer.scrollTop).toBe(200);

  expect(scrollChainByDelta(inner, -250)).toBe(0);
  expect(inner.scrollTop).toBe(0);
  expect(middle.scrollTop).toBe(0);
  expect(outer.scrollTop).toBe(150);
});

test("an inner viewport consumes ordinary movement without moving its ancestors", () => {
  const outer = scrollViewport(null, 30, 100, 300);
  const middle = scrollViewport(outer, 40, 100, 300);
  const inner = scrollViewport(middle, 20, 100, 300);

  expect(scrollChainByDelta(inner, 50)).toBe(0);
  expect(inner.scrollTop).toBe(70);
  expect(middle.scrollTop).toBe(40);
  expect(outer.scrollTop).toBe(30);
});

test("locked ancestors are skipped without acquiring a phantom scroll offset", () => {
  const outer = scrollViewport(null, 20, 100, 300);
  const locked = scrollViewport(outer, 0, 100, 500);
  const inner = scrollViewport(locked, 90, 100, 200);
  locked.style.overflowY = "hidden";

  expect(scrollChainByDelta(inner, 70)).toBe(0);
  expect(inner.scrollTop).toBe(100);
  expect(locked.scrollTop).toBe(0);
  expect(outer.scrollTop).toBe(80);
});

test("wheel units normalize without changing the scroll direction", () => {
  expect(wheelDeltaToPixels(3, 0, 500)).toBe(3);
  expect(wheelDeltaToPixels(3, 1, 500)).toBe(48);
  expect(wheelDeltaToPixels(-2, 2, 500)).toBe(-1_000);
});
