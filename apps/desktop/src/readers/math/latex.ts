/**
 * Copying maths as LaTeX (Phase 8b). Exact where the book has the maths:
 * the TeX KaTeX keeps with Markdown formulas, TeX annotations in EPUB
 * MathML, or the MathML itself turned into LaTeX. From a PDF's text only
 * the symbols can be rebuilt, so that is a best attempt to correct by hand.
 */

const SYMBOLS: Record<string, string> = {
  α: "\\alpha",
  β: "\\beta",
  γ: "\\gamma",
  δ: "\\delta",
  ε: "\\epsilon",
  ϵ: "\\epsilon",
  ζ: "\\zeta",
  η: "\\eta",
  θ: "\\theta",
  ϑ: "\\vartheta",
  ι: "\\iota",
  κ: "\\kappa",
  λ: "\\lambda",
  μ: "\\mu",
  ν: "\\nu",
  ξ: "\\xi",
  π: "\\pi",
  ρ: "\\rho",
  σ: "\\sigma",
  ς: "\\varsigma",
  τ: "\\tau",
  υ: "\\upsilon",
  φ: "\\phi",
  ϕ: "\\phi",
  χ: "\\chi",
  ψ: "\\psi",
  ω: "\\omega",
  Γ: "\\Gamma",
  Δ: "\\Delta",
  Θ: "\\Theta",
  Λ: "\\Lambda",
  Ξ: "\\Xi",
  Π: "\\Pi",
  Σ: "\\Sigma",
  Φ: "\\Phi",
  Ψ: "\\Psi",
  Ω: "\\Omega",
  "±": "\\pm",
  "∓": "\\mp",
  "×": "\\times",
  "÷": "\\div",
  "·": "\\cdot",
  "⋅": "\\cdot",
  "−": "-",
  "≠": "\\neq",
  "≈": "\\approx",
  "≡": "\\equiv",
  "≤": "\\leq",
  "≥": "\\geq",
  "≪": "\\ll",
  "≫": "\\gg",
  "∝": "\\propto",
  "∼": "\\sim",
  "≅": "\\cong",
  "→": "\\to",
  "←": "\\leftarrow",
  "↔": "\\leftrightarrow",
  "⇒": "\\Rightarrow",
  "⇐": "\\Leftarrow",
  "⇔": "\\Leftrightarrow",
  "↦": "\\mapsto",
  "∞": "\\infty",
  "∂": "\\partial",
  "∇": "\\nabla",
  "∑": "\\sum",
  "∏": "\\prod",
  "∫": "\\int",
  "∬": "\\iint",
  "∮": "\\oint",
  "√": "\\sqrt",
  "∈": "\\in",
  "∉": "\\notin",
  "⊂": "\\subset",
  "⊆": "\\subseteq",
  "⊃": "\\supset",
  "⊇": "\\supseteq",
  "∪": "\\cup",
  "∩": "\\cap",
  "∅": "\\emptyset",
  "∀": "\\forall",
  "∃": "\\exists",
  "¬": "\\neg",
  "∧": "\\wedge",
  "∨": "\\vee",
  "⊕": "\\oplus",
  "⊗": "\\otimes",
  "∘": "\\circ",
  "′": "'",
  "″": "''",
  "…": "\\ldots",
  "⋯": "\\cdots",
  ℏ: "\\hbar",
  ℓ: "\\ell",
  ℝ: "\\mathbb{R}",
  ℕ: "\\mathbb{N}",
  ℤ: "\\mathbb{Z}",
  ℚ: "\\mathbb{Q}",
  ℂ: "\\mathbb{C}",
  "⟨": "\\langle",
  "⟩": "\\rangle",
  "°": "^{\\circ}",
  "⁡": "",
  "⁢": "",
  "⁣": "",
  "​": "",
};

const SUPER: Record<string, string> = {
  "⁰": "0",
  "¹": "1",
  "²": "2",
  "³": "3",
  "⁴": "4",
  "⁵": "5",
  "⁶": "6",
  "⁷": "7",
  "⁸": "8",
  "⁹": "9",
  "⁺": "+",
  "⁻": "-",
  "⁼": "=",
  "⁽": "(",
  "⁾": ")",
  ⁿ: "n",
  ⁱ: "i",
};
const SUB: Record<string, string> = {
  "₀": "0",
  "₁": "1",
  "₂": "2",
  "₃": "3",
  "₄": "4",
  "₅": "5",
  "₆": "6",
  "₇": "7",
  "₈": "8",
  "₉": "9",
  "₊": "+",
  "₋": "-",
  "₌": "=",
  "₍": "(",
  "₎": ")",
  ₐ: "a",
  ₑ: "e",
  ₒ: "o",
  ₓ: "x",
  ᵢ: "i",
  ⱼ: "j",
  ₖ: "k",
  ₙ: "n",
};

/** Mathematical italic and bold letters (U+1D400…) back to plain ones. */
function plainLetter(ch: string): string {
  const c = ch.codePointAt(0) ?? 0;
  if (c < 0x1d400 || c > 0x1d6a3) return ch;
  const i = (c - 0x1d400) % 52;
  return String.fromCharCode(i < 26 ? 65 + i : 97 + i - 26);
}

/** A symbol's LaTeX, with a space after a command so it does not run into
 * the next letter. */
function symbol(ch: string): string {
  // Mathematical italic and bold letters, Greek too (𝜋 → π).
  const plain = (ch.codePointAt(0) ?? 0) >= 0x1d400 ? ch.normalize("NFKC") : ch;
  const s = SYMBOLS[plain];
  if (s === undefined) return plainLetter(plain);
  return /^\\[a-zA-Z]+$/.test(s) ? `${s} ` : s;
}

function tidy(s: string): string {
  return s
    .replace(/[ \t]+/g, " ")
    .replace(/ ([}^_)\]]|$)/g, "$1")
    .replace(/([{([]) /g, "$1")
    .trim();
}

/** Text of maths (from a PDF) as LaTeX: symbols, and superscript and
 * subscript characters. Structure (fractions, roots) cannot be seen. */
export function textToLatex(text: string): string {
  let out = "";
  const chars = [...text.replace(/\s+/g, " ")];
  for (let i = 0; i < chars.length; i++) {
    const ch = chars[i]!;
    for (const [map, mark] of [
      [SUPER, "^"],
      [SUB, "_"],
    ] as const) {
      if (map[ch] !== undefined) {
        let run = "";
        while (i < chars.length && map[chars[i]!] !== undefined) run += map[chars[i++]!];
        i--;
        out += run.length > 1 ? `${mark}{${run}}` : `${mark}${run}`;
        break;
      }
    }
    if (SUPER[ch] !== undefined || SUB[ch] !== undefined) continue;
    out += symbol(ch);
  }
  return tidy(functionNames(out));
}

/** "sin", "log", "lim"… written as plain letters, as commands. */
function functionNames(s: string): string {
  return s.replace(
    // A name on its own, or run into a one-letter variable ("sinx").
    /(?<![\\a-zA-Z])(arcsin|arccos|arctan|sinh|cosh|tanh|sin|cos|tan|cot|sec|csc|log|ln|exp|lim|max|min|sup|inf|det|gcd|arg|deg|dim)(?![a-zA-Z](?=[a-zA-Z]))(?=[a-zA-Z]?(?![a-zA-Z]))/g,
    "\\$1 ",
  );
}

const FUNCTIONS = new Set([
  "sin",
  "cos",
  "tan",
  "cot",
  "sec",
  "csc",
  "log",
  "ln",
  "exp",
  "lim",
  "max",
  "min",
  "sup",
  "inf",
  "det",
  "dim",
  "arg",
  "deg",
  "gcd",
  "sinh",
  "cosh",
  "tanh",
]);

function group(s: string): string {
  const t = s.trim();
  return [...t].length === 1 || /^\\[a-zA-Z]+ ?$/.test(t) ? t : `{${t}}`;
}

const ACCENTS: Record<string, string> = {
  "^": "\\hat",
  ˆ: "\\hat",
  "~": "\\tilde",
  "˜": "\\tilde",
  "¯": "\\bar",
  "‾": "\\overline",
  "→": "\\vec",
  "⃗": "\\vec",
  "˙": "\\dot",
  "¨": "\\ddot",
  _: "\\underline",
};

function children(el: Element): Element[] {
  return [...el.children];
}

/** MathML as LaTeX. */
export function mathmlToLatex(el: Element): string {
  const tex = el.querySelector("annotation[encoding='application/x-tex']")?.textContent;
  if (tex?.trim()) return tex.trim();
  return tidy(node(el));
}

function node(el: Element): string {
  const tag = el.localName;
  const kids = children(el);
  const all = () => kids.map(node).join("");
  switch (tag) {
    case "math":
    case "mrow":
    case "mstyle":
    case "mpadded":
    case "mphantom":
    case "semantics":
    case "menclose":
      return tag === "semantics" && kids[0] ? node(kids[0]) : all();
    case "annotation":
    case "annotation-xml":
      return "";
    case "mi": {
      const t = (el.textContent ?? "").trim();
      if (FUNCTIONS.has(t)) return `\\${t} `;
      if ([...t].length > 1) return `\\mathrm{${t}}`;
      return symbol(t);
    }
    case "mn":
      return el.textContent?.trim() ?? "";
    case "mo": {
      const t = (el.textContent ?? "").trim();
      if (t === "{" || t === "}") return `\\${t}`;
      return [...t].map(symbol).join("");
    }
    case "mtext": {
      const t = el.textContent ?? "";
      return t.trim() ? `\\text{${t}}` : " ";
    }
    case "mspace":
      return "\\,";
    case "mfrac":
      return `\\frac{${node(kids[0]!)}}{${node(kids[1]!)}}`;
    case "msqrt":
      return `\\sqrt{${all()}}`;
    case "mroot":
      return `\\sqrt[${node(kids[1]!)}]{${node(kids[0]!)}}`;
    case "msup":
      return `${node(kids[0]!)}^${group(node(kids[1]!))}`;
    case "msub":
      return `${node(kids[0]!)}_${group(node(kids[1]!))}`;
    case "msubsup":
      return `${node(kids[0]!)}_${group(node(kids[1]!))}^${group(node(kids[2]!))}`;
    case "munder":
    case "mover":
    case "munderover": {
      const base = node(kids[0]!);
      const acc = tag === "mover" ? (kids[1]?.textContent ?? "").trim() : "";
      if (tag === "mover" && ACCENTS[acc]) return `${ACCENTS[acc]}{${base}}`;
      const under = tag !== "mover" && kids[1] ? node(kids[1]) : null;
      const over =
        tag === "mover" && kids[1]
          ? node(kids[1])
          : tag === "munderover" && kids[2]
            ? node(kids[2])
            : null;
      if (/^\\(sum|prod|int|oint|lim|max|min|bigcup|bigcap)\b/.test(base.trim())) {
        const u = under !== null ? `_${group(under)}` : "";
        const o = over !== null ? `^${group(over)}` : "";
        return `${base.trim()}${u}${o} `;
      }
      let r = base;
      if (under !== null) r = `\\underset{${under}}{${r}}`;
      if (over !== null) r = `\\overset{${over}}{${r}}`;
      return r;
    }
    case "mfenced": {
      const open = el.getAttribute("open") ?? "(";
      const close = el.getAttribute("close") ?? ")";
      const sep = el.getAttribute("separators") ?? ",";
      return `\\left${open === "{" ? "\\{" : open}${kids.map(node).join(sep)}\\right${close === "}" ? "\\}" : close}`;
    }
    case "mtable": {
      const rows = kids
        .filter((r) => r.localName === "mtr" || r.localName === "mlabeledtr")
        .map((r) =>
          children(r)
            .filter((c) => c.localName === "mtd")
            .map((c) => node(c).trim())
            .join(" & "),
        );
      return `\\begin{matrix}${rows.join(" \\\\ ")}\\end{matrix}`;
    }
    case "mtd":
    case "mtr":
      return all();
    default:
      return kids.length ? all() : [...(el.textContent ?? "")].map(symbol).join("");
  }
}
