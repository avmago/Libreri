import { useState } from "react";
import { Brain } from "lucide-react";
import { MathText } from "@/components/MathText";
import type { Annotation } from "@/lib/ipc";
import { hasMath } from "@/lib/math";
import { cn } from "@/lib/utils";
import { KIND_NAME, kindsOf, pickableWords, possibleKinds, type CardKind } from "./model";
import { useReview } from "./store";

const SHORT: Record<CardKind, string> = { passage: "Passage", qa: "Q&A", cloze: "Cloze" };

/**
 * "Review card" in a highlight's menu: which cards it makes (passage,
 * question and answer from its comment, cloze with chosen words), or none.
 */
export function CardOptionsSection({ annotation }: { annotation: Annotation }) {
  const data = useReview((s) => s.data);
  const update = useReview((s) => s.update);
  const [picking, setPicking] = useState(false);
  const [editing, setEditing] = useState(false);
  const exact = annotation.quote?.exact?.trim();
  const own = data.cards[annotation.id]?.text?.trim();
  const quote = own || exact;
  const [draft, setDraft] = useState(quote ?? "");
  if (annotation.kind !== "highlight" || !quote) return null;
  const bookOff = data.booksOff.includes(annotation.bookId);
  const o = data.cards[annotation.id] ?? {};
  const on = kindsOf(annotation.note, o, data.settings.autoNew);
  const chosen = new Set(o.cloze ?? []);

  const setKinds = (kinds: CardKind[], extra: { cloze?: string[] } = {}) =>
    update((d) => ({
      ...d,
      cards: {
        ...d.cards,
        [annotation.id]: { ...d.cards[annotation.id], ...extra, kinds, off: false },
      },
    }));
  const toggle = (k: CardKind) => {
    if (k === "cloze" && !o.cloze?.length) return setPicking(true);
    setKinds(on.includes(k) ? on.filter((x) => x !== k) : [...on, k]);
  };
  const pickWord = (w: string) => {
    const next = new Set(chosen);
    if (next.has(w)) next.delete(w);
    else next.add(w);
    const words = [...next];
    const kinds = on.filter((k) => k !== "cloze");
    setKinds(words.length ? [...kinds, "cloze"] : kinds, { cloze: words });
  };

  const saveText = (text: string | undefined) => {
    update((d) => {
      const cur = d.cards[annotation.id] ?? {};
      // Hidden words that are no longer in the text go.
      const words = text ? pickableWords(text) : pickableWords(exact ?? "");
      return {
        ...d,
        cards: {
          ...d.cards,
          [annotation.id]: { ...cur, text, cloze: cur.cloze?.filter((w) => words.includes(w)) },
        },
      };
    });
    setEditing(false);
  };

  if (bookOff)
    return (
      <p className="mt-1 flex items-center gap-1.5 border-t px-1 pt-1.5 text-[11.5px] text-muted-foreground">
        <Brain className="size-3.5" aria-hidden /> This book is left out of review.
      </p>
    );

  const can = possibleKinds(annotation.note, { cloze: ["x"] });
  return (
    <div
      className="mt-1 flex w-64 flex-col gap-1.5 border-t px-0.5 pt-1.5"
      aria-label="Review card"
    >
      <div className="flex items-center gap-1.5 text-[11.5px] font-medium text-muted-foreground">
        <Brain className="size-3.5" aria-hidden />
        Review card
        <span className="flex-1" />
        <button
          type="button"
          className="font-normal hover:text-foreground"
          onClick={() =>
            update((d) => ({
              ...d,
              cards: { ...d.cards, [annotation.id]: { ...d.cards[annotation.id], off: !o.off } },
            }))
          }
        >
          {o.off ? "Review it" : "Leave out"}
        </button>
      </div>
      {!o.off && (
        <div className="flex gap-1" role="group" aria-label="Cards from this highlight">
          {(["passage", "qa", "cloze"] as const).map((k) => {
            const usable = can.includes(k);
            return (
              <button
                key={k}
                type="button"
                aria-pressed={on.includes(k)}
                disabled={!usable}
                title={
                  !usable
                    ? "Write a comment to ask a question; the passage is the answer"
                    : KIND_NAME[k]
                }
                onClick={() => toggle(k)}
                className={cn(
                  "flex-1 rounded-md border px-1.5 py-1 text-[12px] font-medium disabled:opacity-40",
                  on.includes(k)
                    ? "border-primary bg-primary/10 text-foreground"
                    : "text-muted-foreground hover:bg-muted",
                )}
              >
                {SHORT[k]}
              </button>
            );
          })}
        </div>
      )}
      {!o.off && !editing && (
        <button
          type="button"
          className="self-start text-[11.5px] text-muted-foreground hover:text-foreground"
          title="Change the card's text, for example to write a formula as $x^2$"
          onClick={() => {
            setDraft(quote);
            setEditing(true);
          }}
        >
          {own ? "Card text changed · Edit" : "Edit the card's text"}
        </button>
      )}
      {!o.off && editing && (
        <div className="flex flex-col gap-1">
          <textarea
            aria-label="Card text"
            rows={3}
            autoFocus
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            className="min-h-16 rounded-md border bg-background px-2 py-1 text-[12.5px]"
          />
          {hasMath(draft) && (
            <div
              aria-label="Preview"
              className="max-h-32 overflow-auto rounded-md border px-2 py-1 font-serif text-[13px]"
            >
              <MathText text={draft} />
            </div>
          )}
          <div className="flex items-center gap-2 text-[11.5px]">
            {own && (
              <button
                type="button"
                className="text-muted-foreground hover:text-foreground"
                onClick={() => saveText(undefined)}
              >
                Use the highlight
              </button>
            )}
            <span className="flex-1" />
            <button
              type="button"
              className="text-muted-foreground hover:text-foreground"
              onClick={() => setEditing(false)}
            >
              Cancel
            </button>
            <button
              type="button"
              className="font-medium text-primary"
              onClick={() =>
                saveText(draft.trim() && draft.trim() !== exact ? draft.trim() : undefined)
              }
            >
              Save
            </button>
          </div>
        </div>
      )}
      {!o.off && (picking || (on.includes("cloze") && chosen.size > 0)) && (
        <div className="flex flex-col gap-1">
          <p className="text-[11.5px] text-muted-foreground">
            {picking ? "Click the words to hide:" : "Hidden words:"}
          </p>
          {picking ? (
            <div className="flex max-h-32 flex-wrap gap-x-1 gap-y-0.5 overflow-auto font-serif text-[13px] leading-snug">
              {pickableWords(quote).map((w, i) => (
                <button
                  key={`${w}-${i}`}
                  type="button"
                  aria-pressed={chosen.has(w)}
                  onClick={() => pickWord(w)}
                  className={cn(
                    "rounded px-0.5",
                    chosen.has(w) ? "bg-primary text-primary-foreground" : "hover:bg-muted",
                  )}
                >
                  {w.startsWith("$") ? <MathText text={w} /> : w}
                </button>
              ))}
            </div>
          ) : (
            <p className="text-[12px]">
              {[...chosen].map((w, i) => (
                <span key={w}>
                  {i > 0 && ", "}
                  <MathText text={w} />
                </span>
              ))}
            </p>
          )}
          <button
            type="button"
            className="self-end text-[11.5px] text-muted-foreground hover:text-foreground"
            onClick={() => setPicking(!picking)}
          >
            {picking ? "Done" : "Change words"}
          </button>
        </div>
      )}
    </div>
  );
}
