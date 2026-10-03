/**
 * Libreri's spell check and word completion on a text box.
 *
 * A text box cannot draw underlines itself, so a layer with the same text
 * (invisible) and the same layout sits over it and draws a wavy line under
 * each word that looks misspelt. Right-click (or Mod+;) on such a word
 * offers corrections, "Add to dictionary" and "Ignore". While typing, a
 * small list offers ways to finish the word; Tab takes the first.
 *
 * It works on any `<textarea>`, including ones made outside React (markup
 * text boxes), through `attachSpell`.
 */
import { useProfilePrefs } from "@/features/profiles";
import { addOwnWord, spellCheck, spellComplete, spellSuggest } from "./api";
import { matchCase, pieces, shiftMisses, wordBefore, type Miss } from "./text";
import "./spell.css";

export interface SpellOptions {
  /** The book being read or written about: its names and terms are fine. */
  bookId?: string | null;
}

/** Words ignored until the app closes. */
const ignored = new Set<string>();

const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

const STYLE_KEYS = [
  "fontFamily",
  "fontSize",
  "fontWeight",
  "fontStyle",
  "fontVariant",
  "lineHeight",
  "letterSpacing",
  "wordSpacing",
  "textTransform",
  "textIndent",
  "tabSize",
  "direction",
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
] as const;

function writing() {
  return useProfilePrefs.getState().prefs.writing;
}

/** Types `text` over the range, as if typed (undo works, React hears it). */
function replaceRange(area: HTMLTextAreaElement, start: number, end: number, text: string) {
  area.focus();
  area.setSelectionRange(start, end);
  if (!document.execCommand?.("insertText", false, text)) {
    area.setRangeText(text, start, end, "end");
    area.dispatchEvent(new Event("input", { bubbles: true }));
  }
}

/** A small list next to a point, for corrections and completions. */
class Popup {
  el = document.createElement("div");
  items: { label: string; hint?: string; run: () => void }[] = [];
  active = 0;

  constructor(
    className: string,
    private readonly onClose: () => void,
  ) {
    this.el.className = `lb-spell-popup ${className}`;
    this.el.setAttribute("role", "listbox");
    this.el.addEventListener("mousedown", (e) => e.preventDefault());
  }

  show(x: number, y: number, below: number) {
    if (!this.el.isConnected) document.body.append(this.el);
    this.render();
    const r = this.el.getBoundingClientRect();
    const left = Math.max(4, Math.min(x, window.innerWidth - r.width - 4));
    const top = y + below + r.height > window.innerHeight - 4 ? y - r.height - 2 : y + below;
    this.el.style.left = `${left}px`;
    this.el.style.top = `${Math.max(4, top)}px`;
  }

  render() {
    this.el.replaceChildren(
      ...this.items.map((it, i) => {
        const b = document.createElement("button");
        b.type = "button";
        b.className = `lb-spell-item${i === this.active ? " lb-spell-active" : ""}`;
        b.setAttribute("role", "option");
        b.setAttribute("aria-selected", String(i === this.active));
        b.textContent = it.label;
        if (it.hint) {
          const h = document.createElement("span");
          h.className = "lb-spell-hint";
          h.textContent = it.hint;
          b.append(h);
        }
        b.addEventListener("click", () => {
          it.run();
          this.close();
        });
        return b;
      }),
    );
  }

  move(d: number) {
    if (!this.items.length) return;
    this.active = (this.active + d + this.items.length) % this.items.length;
    this.render();
  }

  get open() {
    return this.el.isConnected;
  }

  close() {
    if (!this.el.isConnected) return;
    this.el.remove();
    this.onClose();
  }
}

/** Adds spell check and completion to a text box. Returns a function that
 * takes them away again. */
export function attachSpell(area: HTMLTextAreaElement, options: () => SpellOptions): () => void {
  const overlay = document.createElement("div");
  overlay.className = "lb-spell-overlay";
  overlay.setAttribute("aria-hidden", "true");
  area.insertAdjacentElement("afterend", overlay);
  const nativeSpell = area.spellcheck;

  let misses: Miss[] = [];
  let checked = "";
  let lastText = area.value;
  let seq = 0;
  let checkTimer: ReturnType<typeof setTimeout> | undefined;
  let completeTimer: ReturnType<typeof setTimeout> | undefined;

  const menu = new Popup("lb-spell-menu", () => {});
  const completions = new Popup("lb-spell-complete", () => {});

  const layout = () => {
    const cs = getComputedStyle(area);
    for (const k of STYLE_KEYS) overlay.style[k] = cs[k];
    const padX = parseFloat(cs.paddingLeft) + parseFloat(cs.paddingRight);
    const padY = parseFloat(cs.paddingTop) + parseFloat(cs.paddingBottom);
    overlay.style.left = `${area.offsetLeft + area.clientLeft}px`;
    overlay.style.top = `${area.offsetTop + area.clientTop}px`;
    overlay.style.width = `${Math.max(0, area.clientWidth - padX)}px`;
    overlay.style.height = `${Math.max(0, area.clientHeight - padY)}px`;
    overlay.scrollTop = area.scrollTop;
    overlay.scrollLeft = area.scrollLeft;
    const z = parseInt(cs.zIndex, 10);
    overlay.style.zIndex = Number.isNaN(z) ? "1" : String(z + 1);
  };

  const draw = () => {
    const on = writing().spellCheck;
    area.spellcheck = on ? false : nativeSpell;
    const text = area.value;
    const list = on ? misses.filter((m) => !ignored.has(m.word)) : [];
    const nodes: Node[] = pieces(text, list).map((p) => {
      if (p.miss === null) return document.createTextNode(p.text);
      const s = document.createElement("span");
      s.className = "lb-miss";
      s.dataset.i = String(misses.indexOf(list[p.miss]!));
      s.textContent = p.text;
      return s;
    });
    // A text box shows an empty last line after a final newline.
    nodes.push(document.createTextNode("​"));
    overlay.replaceChildren(...nodes);
    layout();
  };

  const check = async () => {
    const w = writing();
    if (!w.spellCheck) {
      misses = [];
      draw();
      return;
    }
    const text = area.value;
    if (text === checked) return;
    const n = ++seq;
    try {
      const found = await spellCheck(text, w.languages, options().bookId ?? null);
      if (n !== seq || area.value !== text) return;
      misses = found.map((m) => ({ start: m.start ?? 0, end: m.end ?? 0, word: m.word }));
      checked = text;
      draw();
    } catch {
      /* no library open, or the dictionary is missing: nothing to mark */
    }
  };
  const scheduleCheck = (ms = 350) => {
    clearTimeout(checkTimer);
    checkTimer = setTimeout(() => void check(), ms);
  };

  /** Where a range of the text is on screen (measured on a hidden copy). */
  const rectOf = (start: number, end: number): DOMRect => {
    const mirror = overlay.cloneNode(false) as HTMLDivElement;
    mirror.style.visibility = "hidden";
    const mark = document.createElement("span");
    mark.textContent = area.value.slice(start, end) || "​";
    mirror.append(area.value.slice(0, start), mark, area.value.slice(end) + "​");
    overlay.insertAdjacentElement("afterend", mirror);
    mirror.scrollTop = area.scrollTop;
    const r = mark.getBoundingClientRect();
    mirror.remove();
    return r;
  };

  const openMenu = async (i: number, x: number, y: number, below: number) => {
    const m = misses[i];
    if (!m) return;
    completions.close();
    const w = writing();
    menu.items = [{ label: "Looking for corrections…", run: () => {} }];
    menu.active = 0;
    menu.show(x, y, below);
    let found: string[] = [];
    try {
      found = await spellSuggest(m.word, w.languages, options().bookId ?? null);
    } catch {
      /* none */
    }
    if (!menu.open) return;
    const replace = (s: string) => {
      const cur = misses.indexOf(m);
      if (cur < 0 || area.value.slice(m.start, m.end) !== m.word) return;
      replaceRange(area, m.start, m.end, matchCase(m.word, s));
    };
    menu.items = [
      ...found.map((s) => ({ label: matchCase(m.word, s), run: () => replace(s) })),
      ...(found.length ? [] : [{ label: "No corrections found", run: () => {} }]),
      {
        label: `Add “${m.word}” to your dictionary`,
        run: () =>
          void addOwnWord(m.word).then(() => {
            misses = misses.filter((x) => x.word !== m.word);
            checked = "";
            draw();
          }),
      },
      {
        label: "Ignore",
        hint: "until Libreri closes",
        run: () => {
          ignored.add(m.word);
          draw();
        },
      },
    ];
    menu.active = 0;
    menu.show(x, y, below);
  };

  const missAt = (x: number, y: number): number | null => {
    for (const s of overlay.querySelectorAll<HTMLElement>(".lb-miss")) {
      for (const r of s.getClientRects())
        if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom) return Number(s.dataset.i);
    }
    return null;
  };

  const complete = async () => {
    const w = writing();
    if (!w.complete || document.activeElement !== area) return;
    const caret = area.selectionStart;
    if (caret !== area.selectionEnd) return;
    const at = wordBefore(area.value, caret);
    if (!at || at.prefix.length < 3) return completions.close();
    let found: string[];
    try {
      found = await spellComplete(at.prefix, w.languages, options().bookId ?? null);
    } catch {
      return;
    }
    if (area.selectionStart !== caret || area.value.slice(at.start, caret) !== at.prefix) return;
    if (!found.length) return completions.close();
    completions.items = found.map((s, i) => ({
      label: s,
      hint: i === 0 ? "Tab" : undefined,
      run: () => replaceRange(area, at.start, caret, s),
    }));
    completions.active = 0;
    const r = rectOf(caret, caret);
    completions.show(r.left, r.top, r.height + 2);
  };

  const onInput = (e: Event) => {
    const text = area.value;
    misses = shiftMisses(misses, lastText, text);
    lastText = text;
    draw();
    scheduleCheck();
    clearTimeout(completeTimer);
    const typing = (e as InputEvent).inputType?.startsWith("insertText");
    if (typing) completeTimer = setTimeout(() => void complete(), 140);
    else completions.close();
  };

  const onKey = (e: KeyboardEvent) => {
    const popup = menu.open ? menu : completions.open ? completions : null;
    if (popup) {
      const take = e.key === "Enter" && popup === menu;
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        e.stopPropagation();
        popup.move(e.key === "ArrowDown" ? 1 : -1);
        return;
      }
      if ((e.key === "Tab" && !e.shiftKey && popup === completions) || take) {
        e.preventDefault();
        e.stopPropagation();
        popup.items[popup.active]?.run();
        popup.close();
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        popup.close();
        return;
      }
    }
    // Mod+; opens corrections for the misspelt word at or before the caret.
    if (e.key === ";" && (isMac ? e.metaKey : e.ctrlKey) && !e.altKey) {
      const caret = area.selectionStart;
      const i = [...misses.keys()]
        .filter((k) => !ignored.has(misses[k]!.word) && misses[k]!.start <= caret)
        .pop();
      if (i === undefined) return;
      e.preventDefault();
      e.stopPropagation();
      const m = misses[i]!;
      const r = rectOf(m.start, m.end);
      void openMenu(i, r.left, r.top, r.height + 2);
    }
  };

  const onContext = (e: MouseEvent) => {
    const i = missAt(e.clientX, e.clientY);
    if (i === null) return;
    e.preventDefault();
    void openMenu(i, e.clientX, e.clientY, 4);
  };

  const onScroll = () => {
    overlay.scrollTop = area.scrollTop;
    overlay.scrollLeft = area.scrollLeft;
    completions.close();
  };
  const onBlur = () => {
    completions.close();
  };
  const onOutside = (e: MouseEvent) => {
    if (menu.open && !menu.el.contains(e.target as Node)) menu.close();
  };
  const onClick = () => completions.close();

  area.addEventListener("input", onInput);
  area.addEventListener("keydown", onKey);
  area.addEventListener("contextmenu", onContext);
  area.addEventListener("scroll", onScroll);
  area.addEventListener("blur", onBlur);
  area.addEventListener("click", onClick);
  document.addEventListener("mousedown", onOutside, true);
  const resize = new ResizeObserver(() => layout());
  resize.observe(area);
  // Settings may change while the box is open.
  const unsubscribe = useProfilePrefs.subscribe((s, prev) => {
    if (s.prefs.writing !== prev.prefs.writing) {
      checked = "";
      draw();
      void check();
    }
  });
  // Text set by React (not typed) is checked too.
  const poll = setInterval(() => {
    if (area.value !== lastText) {
      misses = shiftMisses(misses, lastText, area.value);
      lastText = area.value;
      draw();
      scheduleCheck(0);
    }
  }, 700);

  draw();
  scheduleCheck(0);

  return () => {
    clearTimeout(checkTimer);
    clearTimeout(completeTimer);
    clearInterval(poll);
    unsubscribe();
    resize.disconnect();
    area.removeEventListener("input", onInput);
    area.removeEventListener("keydown", onKey);
    area.removeEventListener("contextmenu", onContext);
    area.removeEventListener("scroll", onScroll);
    area.removeEventListener("blur", onBlur);
    area.removeEventListener("click", onClick);
    document.removeEventListener("mousedown", onOutside, true);
    menu.close();
    completions.close();
    overlay.remove();
    area.spellcheck = nativeSpell;
  };
}
