import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { App } from "./App";

test("advanced mode reveals runtime details", () => {
  render(<App />);

  expect(screen.queryByRole("heading", { name: "Runtime status" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Advanced mode" }));
  expect(screen.getByRole("heading", { name: "Runtime status" })).toBeInTheDocument();
});
