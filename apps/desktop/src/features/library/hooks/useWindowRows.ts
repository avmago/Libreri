/**
 * Windowing for big libraries (B11): only the rows near the screen are
 * drawn, with empty space standing in for the rest, so 10,000 books scroll
 * as smoothly as 100. Small lists (under `FROM` items) are drawn whole, as
 * before.
 */
import { useEffect, useState } from "react";

const FROM = 400;
/** Rows drawn above and below the screen. */
const OVERSCAN = 6;
const REVEAL = "libreri:reveal-book";

/** Asks the windowed view to scroll a book (by index) into view. */
export function revealBook(index: number) {
  window.dispatchEvent(new CustomEvent(REVEAL, { detail: index }));
}

function scrollParent(el: HTMLElement | null): HTMLElement | null {
  for (let p = el?.parentElement; p; p = p.parentElement) {
    const o = getComputedStyle(p).overflowY;
    if (o === "auto" || o === "scroll") return p;
  }
  return null;
}

/**
 * `count` items laid out `columns` to a row, each row `rowHeight` pixels
 * (gap included); `offset` items (folders) come before the books. `el` is
 * the element the rows start in. Returns the item range to draw and the
 * space to leave above and below.
 */
export function useWindowRows(
  el: HTMLElement | null,
  count: number,
  columns: number,
  rowHeight: number,
  offset = 0,
) {
  const on = count >= FROM && rowHeight > 0;
  const cols = Math.max(1, columns);
  const rows = Math.ceil(count / cols);
  const [range, setRange] = useState<[number, number]>([0, 30]);

  useEffect(() => {
    const sc = scrollParent(el);
    if (!on || !el || !sc) return;
    const measure = () => {
      const top = el.getBoundingClientRect().top - sc.getBoundingClientRect().top;
      const first = Math.max(0, Math.floor(-top / rowHeight) - OVERSCAN);
      const last = Math.min(rows, Math.ceil((sc.clientHeight - top) / rowHeight) + OVERSCAN);
      setRange((r) => (r[0] === first && r[1] === last ? r : [first, last]));
    };
    let frame = requestAnimationFrame(measure);
    const later = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measure);
    };
    sc.addEventListener("scroll", later, { passive: true });
    const ro = new ResizeObserver(later);
    ro.observe(sc);
    // Arrow keys move to books that may not be drawn: scroll there first.
    const reveal = (e: Event) => {
      const i = (e as CustomEvent<number>).detail;
      if (typeof i !== "number") return;
      const row = Math.floor((i + offset) / cols);
      const y =
        el.getBoundingClientRect().top -
        sc.getBoundingClientRect().top +
        sc.scrollTop +
        row * rowHeight;
      if (y < sc.scrollTop) sc.scrollTop = y - 8;
      else if (y + rowHeight > sc.scrollTop + sc.clientHeight)
        sc.scrollTop = y + rowHeight - sc.clientHeight + 8;
    };
    window.addEventListener(REVEAL, reveal);
    return () => {
      cancelAnimationFrame(frame);
      sc.removeEventListener("scroll", later);
      ro.disconnect();
      window.removeEventListener(REVEAL, reveal);
    };
  }, [el, on, rowHeight, rows, cols, offset]);

  if (!on) return { on, start: 0, end: count, before: 0, after: 0 };
  const [r0, r1] = range;
  const start = Math.min(count, r0 * cols);
  const end = Math.min(count, Math.max(start, r1 * cols));
  return {
    on,
    start,
    end,
    before: Math.min(r0, rows) * rowHeight,
    after: Math.max(0, (rows - Math.ceil(end / cols)) * rowHeight),
  };
}

/**
 * Columns of an auto-fill CSS grid as drawn now, and (with `item`) the
 * height of one item plus the row gap, kept up to date.
 */
export function useGridSize(
  el: HTMLElement | null,
  minWidth: number,
  gap: number,
  item?: { selector: string; rowGap: number; fallback: number },
) {
  const [size, setSize] = useState({ cols: 1, rowHeight: item?.fallback ?? 0 });
  const selector = item?.selector;
  const rowGap = item?.rowGap ?? 0;
  useEffect(() => {
    if (!el) return;
    const update = () => {
      const css = getComputedStyle(el);
      const inner = el.clientWidth - parseFloat(css.paddingLeft) - parseFloat(css.paddingRight);
      const cols = Math.max(1, Math.floor((inner + gap) / (minWidth + gap)));
      const first = selector ? el.querySelector<HTMLElement>(selector) : null;
      const h = first ? first.getBoundingClientRect().height + rowGap : 0;
      setSize((s) => {
        const rowHeight = h > 10 ? h : s.rowHeight;
        return s.cols === cols && Math.abs(s.rowHeight - rowHeight) < 0.5 ? s : { cols, rowHeight };
      });
    };
    const ro = new ResizeObserver(update);
    ro.observe(el);
    const mo = new MutationObserver(update);
    if (selector) mo.observe(el, { childList: true });
    return () => {
      ro.disconnect();
      mo.disconnect();
    };
  }, [el, minWidth, gap, selector, rowGap]);
  return size;
}
