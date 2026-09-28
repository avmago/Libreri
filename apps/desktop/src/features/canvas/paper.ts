import type { Paper } from "./api";

/** Line spacing of lined, squared and dotted paper, in canvas units. */
export const SPACING = 32;

/**
 * The CSS background that draws paper behind a transparent canvas, moved
 * and scaled with the canvas so the lines stay under the ink.
 */
export function paperStyle(
  paper: Paper,
  scrollX: number,
  scrollY: number,
  zoom: number,
  dark: boolean,
): React.CSSProperties {
  const bg = dark ? "#1c1c1f" : "#ffffff";
  const ink = dark ? "rgba(255,255,255,0.10)" : "rgba(37,99,235,0.16)";
  const dot = dark ? "rgba(255,255,255,0.22)" : "rgba(0,0,0,0.22)";
  const step = SPACING * zoom;
  const pos = `${scrollX * zoom}px ${scrollY * zoom}px`;
  switch (paper) {
    case "lined":
      return {
        backgroundColor: bg,
        backgroundImage: `linear-gradient(to bottom, transparent ${step - 1}px, ${ink} ${step - 1}px)`,
        backgroundSize: `100% ${step}px`,
        backgroundPosition: pos,
      };
    case "grid":
      return {
        backgroundColor: bg,
        backgroundImage: `linear-gradient(to bottom, transparent ${step - 1}px, ${ink} ${step - 1}px), linear-gradient(to right, transparent ${step - 1}px, ${ink} ${step - 1}px)`,
        backgroundSize: `${step}px ${step}px`,
        backgroundPosition: pos,
      };
    case "dotted": {
      const r = Math.max(0.8, 1.2 * zoom);
      return {
        backgroundColor: bg,
        backgroundImage: `radial-gradient(circle at ${step / 2}px ${step / 2}px, ${dot} ${r}px, transparent ${r + 0.5}px)`,
        backgroundSize: `${step}px ${step}px`,
        backgroundPosition: pos,
      };
    }
    default:
      return { backgroundColor: bg };
  }
}
