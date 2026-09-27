/**
 * Page themes: the colours of the page itself, not the app around it
 * (board 4i). "Follow app theme" picks Night when the app is dark.
 */
export type PageThemeId =
  "original" | "sepia" | "paper" | "night" | "dim" | "oled" | "sepiaDark" | "nord" | "solarized";

export interface PageTheme {
  id: PageThemeId;
  name: string;
  /** Page background. */
  bg: string;
  /** Text. */
  fg: string;
  /** Around the pages (PDF gutter, margins). */
  surround: string;
  link: string;
  dark: boolean;
}

export const PAGE_THEMES: PageTheme[] = [
  {
    id: "original",
    name: "Original",
    bg: "#ffffff",
    fg: "#1a1a1a",
    surround: "#e9e9ec",
    link: "#1d4ed8",
    dark: false,
  },
  {
    id: "paper",
    name: "Paper",
    bg: "#faf8f3",
    fg: "#2b2a27",
    surround: "#e8e4da",
    link: "#1d4ed8",
    dark: false,
  },
  {
    id: "sepia",
    name: "Sepia",
    bg: "#f4ecd8",
    fg: "#5b4636",
    surround: "#e3d7bb",
    link: "#8a4b16",
    dark: false,
  },
  {
    id: "solarized",
    name: "Solarized",
    bg: "#fdf6e3",
    fg: "#586e75",
    surround: "#eee8d5",
    link: "#268bd2",
    dark: false,
  },
  {
    id: "dim",
    name: "Dim",
    bg: "#2b2d31",
    fg: "#d4d4d8",
    surround: "#1f2023",
    link: "#93c5fd",
    dark: true,
  },
  {
    id: "night",
    name: "Night",
    bg: "#1c1c1f",
    fg: "#e4e4e7",
    surround: "#111113",
    link: "#93c5fd",
    dark: true,
  },
  {
    id: "sepiaDark",
    name: "Sepia Dark",
    bg: "#2a2420",
    fg: "#e0cfb5",
    surround: "#1c1814",
    link: "#f0b37e",
    dark: true,
  },
  {
    id: "nord",
    name: "Nord",
    bg: "#2e3440",
    fg: "#d8dee9",
    surround: "#242933",
    link: "#88c0d0",
    dark: true,
  },
  {
    id: "oled",
    name: "OLED Black",
    bg: "#000000",
    fg: "#d4d4d4",
    surround: "#000000",
    link: "#93c5fd",
    dark: true,
  },
];

export function pageTheme(id: PageThemeId): PageTheme {
  return PAGE_THEMES.find((t) => t.id === id) ?? PAGE_THEMES[0]!;
}
