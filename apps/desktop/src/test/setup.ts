import "@testing-library/jest-dom/vitest";

Object.defineProperty(window, "scrollTo", {
  configurable: true,
  value: () => undefined
});
Object.defineProperty(Element.prototype, "scrollIntoView", {
  configurable: true,
  writable: true,
  value: () => undefined
});
Object.defineProperty(Element.prototype, "setPointerCapture", {
  configurable: true,
  writable: true,
  value: () => undefined
});
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(cleanup);
