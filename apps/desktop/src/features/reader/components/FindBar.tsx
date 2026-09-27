import { useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { FindResult } from "@/readers";

/** "Find in book" (Mod+F): Enter for next, Shift+Enter for previous, Esc closes. */
export function FindBar({
  onFind,
  onClose,
}: {
  onFind: (query: string, backwards: boolean) => Promise<FindResult>;
  onClose: () => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<FindResult | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => input.current?.focus(), []);

  const run = async (backwards: boolean) => {
    if (!query.trim()) return;
    setBusy(true);
    try {
      setResult(await onFind(query, backwards));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      role="search"
      className="absolute top-3 right-4 z-20 flex items-center gap-1 rounded-lg border bg-popover p-1 shadow-lg"
    >
      <input
        ref={input}
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setResult(null);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            void run(e.shiftKey);
          } else if (e.key === "Escape") {
            e.preventDefault();
            onClose();
          }
        }}
        placeholder="Find in book"
        aria-label="Find in book"
        className="h-7 w-52 rounded-md bg-transparent px-2 text-[13px] outline-none"
      />
      <span
        className="min-w-14 text-right text-[11.5px] text-muted-foreground tabular-nums"
        aria-live="polite"
      >
        {busy
          ? "…"
          : result
            ? result.total
              ? `${result.current} of ${result.total}`
              : "No matches"
            : ""}
      </span>
      <Button
        variant="ghost"
        size="icon"
        className="size-7"
        aria-label="Previous match"
        onClick={() => void run(true)}
      >
        <ChevronUp />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="size-7"
        aria-label="Next match"
        onClick={() => void run(false)}
      >
        <ChevronDown />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="size-7"
        aria-label="Close find"
        onClick={onClose}
      >
        <X />
      </Button>
    </div>
  );
}
