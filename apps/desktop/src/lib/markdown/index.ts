/**
 * Markdown rendering shared by Markdown books and notebooks: CommonMark,
 * maths with KaTeX (`$…$`, `$$…$$`, `\(…\)`, `\[…\]`), code highlighting and
 * Mermaid diagrams (rendered afterwards with `renderMermaid`).
 *
 * Raw HTML in Markdown is not rendered, so a book or note can never run
 * code in the app.
 */
import MarkdownIt, { type StateBlock, type StateInline } from "markdown-it";
import katex from "katex";
import hljs from "highlight.js/lib/common";

function tex(source: string, displayMode: boolean): string {
  try {
    return katex.renderToString(source, {
      displayMode,
      throwOnError: false,
      output: "htmlAndMathml",
      strict: "ignore",
    });
  } catch {
    return `<code>${source}</code>`;
  }
}

/** `$x$` and `\(x\)` inside a line. */
function mathInline(state: StateInline, silent: boolean): boolean {
  const src = state.src;
  const start = state.pos;
  const paren = src.startsWith("\\(", start);
  const dollar = !paren && src[start] === "$" && src[start + 1] !== "$";
  if (!paren && !dollar) return false;
  // "$5 and $6" is money, not maths: the opening $ must be followed by a
  // non-space and the closing $ preceded by one and not followed by a digit.
  if (dollar && /\s/.test(src[start + 1] ?? " ")) return false;
  const [open, close] = paren ? ["\\(", "\\)"] : ["$", "$"];
  let end = src.indexOf(close, start + open.length);
  while (end !== -1 && src[end - 1] === "\\") end = src.indexOf(close, end + 1);
  if (end === -1) return false;
  if (close === "$" && (/\s/.test(src[end - 1] ?? "") || /\d/.test(src[end + 1] ?? ""))) {
    return false;
  }
  const content = src.slice(start + open.length, end);
  if (!content.trim()) return false;
  if (!silent) {
    const token = state.push("math_inline", "", 0);
    token.content = content;
  }
  state.pos = end + close.length;
  return true;
}

/** `$$ … $$` and `\[ … \]` on their own lines. */
function mathBlock(
  state: StateBlock,
  startLine: number,
  endLine: number,
  silent: boolean,
): boolean {
  const lineText = (n: number) =>
    state.src.slice(state.bMarks[n]! + state.tShift[n]!, state.eMarks[n]);
  const first = lineText(startLine).trim();
  const delims: [string, string][] = [
    ["$$", "$$"],
    ["\\[", "\\]"],
  ];
  const pair = delims.find(([o]) => first.startsWith(o));
  if (!pair) return false;
  const [open, close] = pair;
  if (silent) return true;

  let content = first.slice(open.length);
  let line = startLine;
  let found = false;
  if (content.trimEnd().endsWith(close) && content.trim().length >= close.length) {
    content = content.trimEnd().slice(0, -close.length);
    found = true;
  }
  while (!found && ++line < endLine) {
    const text = lineText(line);
    if (text.trimEnd().endsWith(close)) {
      content += `\n${text.trimEnd().slice(0, -close.length)}`;
      found = true;
    } else {
      content += `\n${text}`;
    }
  }
  if (!found) return false;
  state.line = line + 1;
  const token = state.push("math_block", "div", 0);
  token.block = true;
  token.content = content.trim();
  token.map = [startLine, state.line];
  return true;
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function createRenderer() {
  const md = new MarkdownIt({
    html: false,
    linkify: true,
    typographer: true,
    highlight(code, lang) {
      if (lang === "mermaid") {
        return `<pre class="mermaid">${escapeHtml(code)}</pre>`;
      }
      if (lang && hljs.getLanguage(lang)) {
        try {
          return `<pre class="hljs"><code>${hljs.highlight(code, { language: lang }).value}</code></pre>`;
        } catch {
          /* fall through */
        }
      }
      return "";
    },
  });
  md.inline.ruler.after("escape", "math_inline", mathInline);
  md.block.ruler.after("blockquote", "math_block", mathBlock, {
    alt: ["paragraph", "reference", "blockquote", "list"],
  });
  md.renderer.rules.math_inline = (tokens, i) => tex(tokens[i]!.content, false);
  md.renderer.rules.math_block = (tokens, i) =>
    `<div class="math-block">${tex(tokens[i]!.content, true)}</div>\n`;
  // Headings get ids so the table of contents can jump to them.
  md.core.ruler.push("heading_ids", (state) => {
    const used = new Map<string, number>();
    state.tokens.forEach((t, i) => {
      if (t.type !== "heading_open") return;
      const text = state.tokens[i + 1]?.content ?? "";
      const base =
        text
          .toLowerCase()
          .replace(/[^\p{L}\p{N}]+/gu, "-")
          .replace(/^-|-$/g, "") || "section";
      const n = used.get(base) ?? 0;
      used.set(base, n + 1);
      t.attrSet("id", n ? `${base}-${n}` : base);
    });
  });
  return md;
}

let renderer: ReturnType<typeof createRenderer> | null = null;

/** Removes a leading `---` front-matter block. */
export function stripFrontMatter(text: string): string {
  const m = /^\uFEFF?---\r?\n[\s\S]*?\r?\n(---|\.\.\.)\s*(\r?\n|$)/.exec(text);
  return m ? text.slice(m[0].length) : text;
}

export function renderMarkdown(text: string): string {
  renderer ??= createRenderer();
  return renderer.render(stripFrontMatter(text));
}

/** Draws Mermaid diagrams inside `root`, loading Mermaid only when needed. */
export async function renderMermaid(root: HTMLElement, dark: boolean): Promise<void> {
  const blocks = root.querySelectorAll<HTMLElement>("pre.mermaid:not([data-processed])");
  if (!blocks.length) return;
  const { default: mermaid } = await import("mermaid");
  mermaid.initialize({
    startOnLoad: false,
    theme: dark ? "dark" : "neutral",
    securityLevel: "strict",
  });
  try {
    await mermaid.run({ nodes: Array.from(blocks) });
  } catch (e) {
    console.warn("Libreri: a Mermaid diagram could not be drawn", e);
  }
}
