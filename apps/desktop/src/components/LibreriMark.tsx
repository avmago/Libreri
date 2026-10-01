/** Libreri's mark: three books standing and one leaning on the shelf
 * (drawn in the current text colour, sized like the icons around it). */
export function LibreriMark({ className, tight }: { className?: string; tight?: boolean }) {
  return (
    <svg
      // Tight: no space around the drawing, to line up with text.
      viewBox={tight ? "2 14 195 169" : "0 0 200 200"}
      fill="none"
      stroke="currentColor"
      strokeWidth={18}
      strokeLinecap="round"
      className={className}
      aria-hidden
    >
      <path d="M11 23V174M52 66V174M94 43V174M118 91L188 155M117 174H188" />
    </svg>
  );
}
