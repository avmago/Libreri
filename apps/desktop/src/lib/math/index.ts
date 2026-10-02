/**
 * Maths inside plain text (comments, review cards): `$…$` and `\(…\)` in a
 * line, `$$…$$` and `\[…\]` on their own. Everything else stays plain text
 * (escaped, line breaks kept); no Markdown, no HTML.
 */
import katex from "katex";

export type MathPart = { kind: "text" | "inline" | "display"; value: string };

const escape = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** Cuts text into plain parts and formulas. "$5 and $6" stays money. */
export function splitMath(text: string): MathPart[] {
  const out: MathPart[] = [];
  let plain = "";
  let i = 0;
  const flush = () => {
    if (plain) out.push({ kind: "text", value: plain });
    plain = "";
  };
  while (i < text.length) {
    const rest = text.slice(i);
    let m: RegExpExecArray | null = null;
    let kind: MathPart["kind"] | null = null;
    if (rest.startsWith("$$")) {
      m = /^\$\$([\s\S]+?)\$\$/.exec(rest);
      kind = "display";
    } else if (rest.startsWith("\\[")) {
      m = /^\\\[([\s\S]+?)\\\]/.exec(rest);
      kind = "display";
    } else if (rest.startsWith("\\(")) {
      m = /^\\\(([\s\S]+?)\\\)/.exec(rest);
      kind = "inline";
    } else if (rest[0] === "$" && text[i - 1] !== "\\") {
      // Opening $ followed by a non-space; closing $ after a non-space and
      // not before a digit; on one line.
      m = /^\$(?!\s)((?:\\\$|[^$\n])+?)(?<!\s)\$(?!\d)/.exec(rest);
      kind = "inline";
    }
    if (m && kind && m[1]!.trim()) {
      flush();
      out.push({ kind, value: m[1]!.trim() });
      i += m[0].length;
    } else {
      plain += text[i];
      i++;
    }
  }
  flush();
  return out;
}

export function hasMath(text: string | null | undefined): boolean {
  return !!text && splitMath(text).some((p) => p.kind !== "text");
}

/** The text as HTML with its formulas drawn by KaTeX. */
export function renderMathText(text: string): string {
  return splitMath(text)
    .map((p) => {
      if (p.kind === "text") return escape(p.value).replace(/\n/g, "<br>");
      try {
        return katex.renderToString(p.value, {
          displayMode: p.kind === "display",
          throwOnError: false,
          output: "htmlAndMathml",
          strict: "ignore",
          trust: false,
        });
      } catch {
        return `<code>${escape(p.value)}</code>`;
      }
    })
    .join("");
}

/** For Anki (MathJax): `\(…\)` and `\[…\]`, the rest escaped as HTML. */
export function mathForAnki(text: string): string {
  return splitMath(text)
    .map((p) =>
      p.kind === "text"
        ? escape(p.value).replace(/\n/g, "<br>")
        : p.kind === "inline"
          ? `\\(${escape(p.value)}\\)`
          : `\\[${escape(p.value)}\\]`,
    )
    .join("");
}

/** For speech, search snippets and plain copies: the formula's source. */
export function mathAsPlain(text: string): string {
  return splitMath(text)
    .map((p) => p.value)
    .join("");
}
