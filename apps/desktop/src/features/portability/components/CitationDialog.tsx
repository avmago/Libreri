import { useState } from "react";
import { Check, Copy } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import type { CitationStyle } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useCitation } from "../api";
import { copyCitation } from "../clipboard";
import { CITATION_STYLES } from "../model";
import { usePortability } from "../store";

const STYLE_KEY = "libreri.citationStyle";

function savedStyle(): CitationStyle {
  try {
    const s = localStorage.getItem(STYLE_KEY);
    if (CITATION_STYLES.some((c) => c.id === s)) return s as CitationStyle;
  } catch {
    // Storage can be unavailable; APA is a fine default.
  }
  return "apa";
}

/** "Copy citation": the reference in several styles, ready to paste. */
export function CitationDialog() {
  const ids = usePortability((s) => s.citing);
  const close = usePortability((s) => s.close);
  return (
    <Dialog
      open={ids !== null}
      onOpenChange={(o) => !o && close()}
      title={ids && ids.length > 1 ? `Cite ${ids.length} books` : "Cite this book"}
      className="w-[620px]"
    >
      {ids && <Body ids={ids} onDone={close} />}
    </Dialog>
  );
}

function Body({ ids, onDone }: { ids: string[]; onDone: () => void }) {
  const [style, setStyle] = useState<CitationStyle>(savedStyle);
  const [copied, setCopied] = useState(false);
  const { data, error, isLoading } = useCitation(ids, style);

  const choose = (s: CitationStyle) => {
    setStyle(s);
    setCopied(false);
    try {
      localStorage.setItem(STYLE_KEY, s);
    } catch {
      // Not remembered; nothing else changes.
    }
  };

  const copy = async () => {
    if (!data) return;
    await copyCitation(data);
    setCopied(true);
    toast.success(ids.length > 1 ? "References copied" : "Reference copied");
  };

  return (
    <div className="flex flex-col gap-3">
      <div role="tablist" aria-label="Citation style" className="flex gap-1">
        {CITATION_STYLES.map((s) => (
          <button
            key={s.id}
            type="button"
            role="tab"
            aria-selected={s.id === style}
            onClick={() => choose(s.id)}
            className={cn(
              "rounded-md px-2.5 py-1 text-[13px]",
              s.id === style ? "bg-primary text-primary-foreground" : "hover:bg-muted",
            )}
          >
            {s.label}
          </button>
        ))}
      </div>
      <div
        className={cn(
          "max-h-80 min-h-24 overflow-auto rounded-md border bg-background px-3 py-2.5 leading-relaxed select-text",
          style === "bibtex" && "font-mono text-[12px] whitespace-pre",
        )}
        aria-live="polite"
      >
        {isLoading && <span className="text-muted-foreground">Formatting…</span>}
        {error && <span className="text-destructive">{error.message}</span>}
        {data &&
          (style === "bibtex" ? (
            data.text
          ) : (
            // Built in Rust from escaped text; only <p> and <i> tags.
            <div
              className="flex flex-col gap-2 [&_p]:pl-6 [&_p]:-indent-6"
              dangerouslySetInnerHTML={{ __html: data.html }}
            />
          ))}
      </div>
      <p className="text-[12px] text-muted-foreground">
        Check the details before you rely on a reference: it is only as good as the book&apos;s
        details. Titles are used as typed.
      </p>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onDone}>
          Close
        </Button>
        <Button onClick={() => void copy()} disabled={!data}>
          {copied ? <Check /> : <Copy />} Copy
        </Button>
      </div>
    </div>
  );
}
