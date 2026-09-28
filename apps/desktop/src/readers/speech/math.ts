/**
 * Maths read as words: MathML (EPUB books, and the MathML KaTeX adds to
 * Markdown books) and plain TeX as a fallback.
 */

const WORDS: Record<string, string> = {
  "+": "plus",
  "−": "minus",
  "-": "minus",
  "±": "plus or minus",
  "×": "times",
  "·": "times",
  "⋅": "times",
  "÷": "divided by",
  "=": "equals",
  "≠": "is not equal to",
  "≈": "is approximately",
  "<": "is less than",
  ">": "is greater than",
  "≤": "is less than or equal to",
  "≥": "is greater than or equal to",
  "→": "tends to",
  "∞": "infinity",
  "∑": "the sum of",
  "∏": "the product of",
  "∫": "the integral of",
  "∂": "partial",
  "∇": "nabla",
  "∈": "in",
  "∀": "for all",
  "∃": "there exists",
  "√": "the square root of",
  "′": "prime",
  "(": "",
  ")": "",
  "[": "",
  "]": "",
  "{": "",
  "}": "",
  ",": ",",
};

const GREEK: Record<string, string> = {
  α: "alpha",
  β: "beta",
  γ: "gamma",
  δ: "delta",
  ε: "epsilon",
  ϵ: "epsilon",
  ζ: "zeta",
  η: "eta",
  θ: "theta",
  ι: "iota",
  κ: "kappa",
  λ: "lambda",
  μ: "mu",
  ν: "nu",
  ξ: "xi",
  π: "pi",
  ρ: "rho",
  σ: "sigma",
  τ: "tau",
  υ: "upsilon",
  φ: "phi",
  ϕ: "phi",
  χ: "chi",
  ψ: "psi",
  ω: "omega",
  Γ: "capital gamma",
  Δ: "capital delta",
  Θ: "capital theta",
  Λ: "capital lambda",
  Π: "capital pi",
  Σ: "capital sigma",
  Φ: "capital phi",
  Ψ: "capital psi",
  Ω: "capital omega",
};

function token(t: string): string {
  const s = t.trim();
  if (!s) return "";
  if (s in WORDS) return WORDS[s]!;
  if (s in GREEK) return GREEK[s]!;
  return s;
}

function power(exp: string): string {
  if (exp === "2") return "squared";
  if (exp === "3") return "cubed";
  if (exp === "prime" || exp === "′") return "prime";
  return `to the power ${exp}`;
}

const join = (parts: string[]) => parts.filter(Boolean).join(" ").replace(/\s+/g, " ").trim();

/** A MathML element as words. */
export function mathmlToSpeech(el: Element): string {
  const alt = el.getAttribute("alttext");
  const say = (n: Element): string => {
    const kids = Array.from(n.children);
    switch (n.localName) {
      case "mi":
      case "mn":
      case "mo":
      case "mtext":
        return token(n.textContent ?? "");
      case "msup":
        return join([say(kids[0]!), power(say(kids[1]!))]);
      case "msub":
        return join([say(kids[0]!), "sub", say(kids[1]!)]);
      case "msubsup":
        return join([say(kids[0]!), "sub", say(kids[1]!), power(say(kids[2]!))]);
      case "mfrac":
        return join([say(kids[0]!), "over", say(kids[1]!)]);
      case "msqrt":
        return join(["the square root of", ...kids.map(say)]);
      case "mroot":
        return join(["the", say(kids[1]!), "root of", say(kids[0]!)]);
      case "munderover":
        return join([say(kids[0]!), "from", say(kids[1]!), "to", say(kids[2]!), "of"]);
      case "munder":
        return join([say(kids[0]!), say(kids[1]!)]);
      case "mover":
        return join([say(kids[0]!), say(kids[1]!)]);
      case "annotation":
      case "annotation-xml":
        return "";
      case "semantics":
        return kids[0] ? say(kids[0]) : "";
      default:
        return join(kids.map(say));
    }
  };
  const spoken = say(el);
  return spoken || alt || "";
}

/** Plain TeX as words (for formulas without MathML). */
export function texToSpeech(tex: string): string {
  let s = tex;
  const rules: [RegExp, string][] = [
    [/\\frac\{([^{}]*)\}\{([^{}]*)\}/g, " $1 over $2 "],
    [/\\sqrt\{([^{}]*)\}/g, " the square root of $1 "],
    [/\^\{?2\}?/g, " squared "],
    [/\^\{?3\}?/g, " cubed "],
    [/\^\{([^{}]*)\}/g, " to the power $1 "],
    [/\^(\w)/g, " to the power $1 "],
    [/_\{([^{}]*)\}/g, " sub $1 "],
    [/_(\w)/g, " sub $1 "],
    [/\\(?:left|right|,|;|!|quad|qquad)/g, " "],
    [/\\cdot|\\times/g, " times "],
    [/\\pm/g, " plus or minus "],
    [/\\leq?/g, " is less than or equal to "],
    [/\\geq?/g, " is greater than or equal to "],
    [/\\neq/g, " is not equal to "],
    [/\\approx/g, " is approximately "],
    [/\\infty/g, " infinity "],
    [/\\sum/g, " the sum of "],
    [/\\int/g, " the integral of "],
    [/\\to|\\rightarrow/g, " tends to "],
  ];
  for (const [re, to] of rules) s = s.replace(re, to);
  s = s.replace(/\\([a-zA-Z]+)/g, (_, name: string) => {
    const g = Object.entries(GREEK).find(([, w]) => w === name.toLowerCase());
    return g
      ? ` ${name[0] === name[0]!.toUpperCase() ? "capital " : ""}${name.toLowerCase()} `
      : ` ${name} `;
  });
  s = s.replace(/[{}]/g, " ");
  s = s.replace(/[=+\-<>]/g, (c) => ` ${WORDS[c] ?? c} `);
  return s.replace(/\s+/g, " ").trim();
}
