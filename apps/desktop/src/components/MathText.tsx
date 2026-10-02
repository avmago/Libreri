import "katex/dist/katex.min.css";
import { useMemo } from "react";
import { hasMath, renderMathText } from "@/lib/math";
import { cn } from "@/lib/utils";

/**
 * Plain text with its maths drawn (`$x^2$`, `$$\int f$$`). Text without
 * maths is shown as it is, so nothing changes for ordinary comments.
 */
export function MathText({
  text,
  className,
  as: Tag = "span",
}: {
  text: string;
  className?: string;
  as?: "span" | "p" | "div";
}) {
  const html = useMemo(() => (hasMath(text) ? renderMathText(text) : null), [text]);
  if (html === null) return <Tag className={cn("whitespace-pre-line", className)}>{text}</Tag>;
  return (
    <Tag
      className={cn("math-text", className)}
      // KaTeX output (trust off) and escaped text: no HTML from the comment.
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
