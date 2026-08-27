import { isValidElement, useState, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

export interface MarkdownContentProps {
  content: string;
  onOpenLink?: (url: string) => void | Promise<void>;
}

function externalMarkdownUrl(value: string): string | null {
  try {
    const url = new URL(value);
    if (url.protocol !== "https:" || url.username || url.password) return null;
    return url.toString();
  } catch {
    return null;
  }
}

function markdownUrlTransform(value: string): string {
  if (value.startsWith("#")) return value;
  return externalMarkdownUrl(value) ?? "";
}

function CodeBlock({ language, children }: { language?: string; children: ReactNode }) {
  const [copied, setCopied] = useState(false);
  const code = String(children ?? "").replace(/\n$/, "");

  async function copyCode() {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1_500);
    } catch {
      setCopied(false);
    }
  }

  return <div className="markdown-code-block">
    <div className="markdown-code-header">
      <span>{language || "Code"}</span>
      <button type="button" aria-label="Copy code" onClick={() => void copyCode()}>{copied ? "Copied" : "Copy"}</button>
    </div>
    <pre data-scroll-chain><code className={language ? `language-${language}` : undefined}>{code}</code></pre>
  </div>;
}

export function MarkdownContent({ content, onOpenLink }: MarkdownContentProps) {
  const components: Components = {
    a({ href, children, ...props }) {
      const external = href ? externalMarkdownUrl(href) : null;
      if (!external && !href?.startsWith("#")) return <span>{children}</span>;
      return <a
        {...props}
        href={external ?? href}
        rel={external ? "noreferrer noopener" : undefined}
        onClick={(event) => {
          if (external) {
            event.preventDefault();
            if (onOpenLink) void onOpenLink(external);
          }
        }}
      >{children}</a>;
    },
    img({ src, alt, ...props }) {
      return <img {...props} src={src} alt={alt ?? ""} loading="lazy" referrerPolicy="no-referrer" />;
    },
    pre({ children }) {
      if (isValidElement<{ className?: string; children?: ReactNode }>(children)) {
        const match = /(?:^|\s)language-([^\s]+)/.exec(children.props.className ?? "");
        return <CodeBlock language={match?.[1]}>{children.props.children}</CodeBlock>;
      }
      return <CodeBlock>{children}</CodeBlock>;
    }
  };

  return <div className="markdown-content">
    <ReactMarkdown remarkPlugins={[remarkGfm]} skipHtml urlTransform={markdownUrlTransform} components={components}>{content}</ReactMarkdown>
  </div>;
}
