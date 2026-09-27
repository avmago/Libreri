/**
 * Where PDF.js finds its helper files (copied into public/pdfjs by the Vite
 * plugin). Without `wasmUrl`, pages made of JPEG 2000 or JBIG2 images, which
 * is how most scanned PDFs are stored, render blank.
 */
export const PDF_ASSETS = {
  standardFontDataUrl: "/pdfjs/standard_fonts/",
  cMapUrl: "/pdfjs/cmaps/",
  cMapPacked: true,
  iccUrl: "/pdfjs/iccs/",
  wasmUrl: "/pdfjs/wasm/",
} as const;
