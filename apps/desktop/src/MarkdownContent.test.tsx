import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { MarkdownContent } from "./MarkdownContent";

test("renders CommonMark and GitHub-flavored chat content", () => {
  render(<MarkdownContent content={[
    "## Result",
    "",
    "Use **bold**, `inline()`, and ~~old~~ text.",
    "",
    "- first",
    "- second",
    "",
    "| Name | State |",
    "| --- | --- |",
    "| Chat | Ready |"
  ].join("\n")} />);

  expect(screen.getByRole("heading", { name: "Result" })).toBeInTheDocument();
  expect(screen.getByText("bold").tagName).toBe("STRONG");
  expect(screen.getByText("inline()").tagName).toBe("CODE");
  expect(screen.getByText("old").tagName).toBe("DEL");
  expect(screen.getAllByRole("listitem")).toHaveLength(2);
  expect(screen.getByRole("table")).toHaveTextContent("Chat");
});

test("renders fenced code as a labeled copyable code box", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  const { container } = render(<MarkdownContent content={"```ts\nconst ready = true;\n```"} />);

  const box = container.querySelector(".markdown-code-block");
  expect(box).not.toBeNull();
  expect(within(box as HTMLElement).getByText("ts")).toBeInTheDocument();
  expect(within(box as HTMLElement).getByText("const ready = true;")).toBeInTheDocument();
  fireEvent.click(within(box as HTMLElement).getByRole("button", { name: "Copy code" }));
  expect(writeText).toHaveBeenCalledWith("const ready = true;");
});

test("keeps raw HTML inert and opens only credential-free HTTPS links through the host", () => {
  const openLink = vi.fn();
  render(<MarkdownContent content={"<script>alert(1)</script>\n\n[Docs](https://example.com/guide)"} onOpenLink={openLink} />);

  expect(document.querySelector("script")).toBeNull();
  fireEvent.click(screen.getByRole("link", { name: "Docs" }));
  expect(openLink).toHaveBeenCalledWith("https://example.com/guide");
});
