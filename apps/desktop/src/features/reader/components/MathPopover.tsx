import { useMemo, useState } from "react";
import katex from "katex";
import { Copy, NotebookPen } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import { Floating } from "./Popovers";

/**
 * A formula as LaTeX (Phase 8b): shown with a live preview so it can be
 * checked and corrected, then copied or added to the notebook.
 */
export function MathPopover({
  latex: initial,
  rect,
  from,
  onNotebook,
  onClose,
}: {
  latex: string;
  rect: DOMRect;
  /** The book's own maths (exact), rebuilt from a page's text, or read
   * from a picture by the maths model (both best attempts). */
  from: "book" | "text" | "picture";
  onNotebook: (block: string) => void;
  onClose: () => void;
}) {
  const [latex, setLatex] = useState(initial);
  const preview = useMemo(() => {
    try {
      return katex.renderToString(latex, {
        displayMode: true,
        throwOnError: true,
        strict: "ignore",
      });
    } catch (e) {
      return `<span class="text-destructive text-[12px]">${String((e as Error).message ?? e)
        .replace(/</g, "&lt;")
        .slice(0, 160)}</span>`;
    }
  }, [latex]);
  const copy = (text: string, what: string) =>
    void navigator.clipboard.writeText(text).then(
      () => toast.success(`${what} copied`),
      () => toast.error("Could not copy"),
    );

  return (
    <Floating rect={rect} onClose={onClose}>
      <div className="flex w-[26rem] max-w-[90vw] flex-col gap-2 p-1">
        <div
          className="max-h-40 overflow-auto rounded-md border bg-background px-2 py-1"
          // KaTeX output of the person's own LaTeX (trust: false, no HTML).
          dangerouslySetInnerHTML={{ __html: preview }}
        />
        <Textarea
          aria-label="LaTeX"
          rows={3}
          spellCheck={false}
          value={latex}
          onChange={(e) => setLatex(e.target.value)}
          className="min-h-16 font-mono text-[12.5px]"
        />
        {from === "text" && (
          <p className="text-[12px] text-muted-foreground">
            Rebuilt from the page's text: fractions, roots and layout cannot be seen there, so check
            it against the preview.
          </p>
        )}
        {from === "picture" && (
          <p className="text-[12px] text-muted-foreground">
            Read from the picture by the maths model: check it against the page and the preview.
          </p>
        )}
        <div className="flex flex-wrap justify-end gap-1.5">
          <Button variant="ghost" size="sm" onClick={() => onNotebook(`$$\n${latex}\n$$`)}>
            <NotebookPen /> Add to notebook
          </Button>
          <Button variant="outline" size="sm" onClick={() => copy(`$${latex}$`, "LaTeX with $…$")}>
            Copy as $…$
          </Button>
          <Button size="sm" onClick={() => copy(latex, "LaTeX")}>
            <Copy /> Copy LaTeX
          </Button>
        </div>
      </div>
    </Floating>
  );
}
