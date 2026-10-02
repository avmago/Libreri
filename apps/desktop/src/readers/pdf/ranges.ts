/**
 * Opens a PDF piece by piece (B4): the first megabyte comes first, and
 * PDF.js asks for the rest of the file only as pages need it, with HTTP
 * range requests to `book://`. A 500 MB scan opens without being read
 * whole. If the server answers without ranges, the whole file is used.
 */
type PdfJs = typeof import("pdfjs-dist/legacy/build/pdf.mjs");

const FIRST = 1 << 20;
/** The protocol answers at most 8 MB per request; asked for in pieces. */
const PIECE = 8 << 20;

async function get(url: string, start: number, end: number): Promise<Uint8Array> {
  const out = new Uint8Array(end - start);
  let at = start;
  while (at < end) {
    const to = Math.min(end, at + PIECE);
    const r = await fetch(url, { headers: { Range: `bytes=${at}-${to - 1}` } });
    if (r.status !== 206 && r.status !== 200) throw new Error(`the book could not be read (${r.status})`);
    const bytes = new Uint8Array(await r.arrayBuffer());
    if (!bytes.length) throw new Error("the book could not be read");
    if (r.status === 200) {
      // No ranges after all: the whole file came.
      out.set(bytes.subarray(start, end));
      return out;
    }
    out.set(bytes.subarray(0, Math.min(bytes.length, end - at)), at - start);
    at += bytes.length;
  }
  return out;
}

export async function pdfSource(
  pdfjs: PdfJs,
  url: string,
): Promise<{ range: InstanceType<PdfJs["PDFDataRangeTransport"]> } | { data: Uint8Array }> {
  const r = await fetch(url, { headers: { Range: `bytes=0-${FIRST - 1}` } });
  if (!r.ok) throw new Error(`the book could not be read (${r.status})`);
  const first = new Uint8Array(await r.arrayBuffer());
  const total = Number(/\/(\d+)\s*$/.exec(r.headers.get("Content-Range") ?? "")?.[1]);
  if (r.status !== 206 || !total || first.length >= total) return { data: first };
  const transport = new pdfjs.PDFDataRangeTransport(total, first);
  transport.requestDataRange = (begin: number, end: number) => {
    get(url, begin, end).then(
      (chunk) => transport.onDataRange(begin, chunk),
      (e: unknown) => console.warn("Libreri: part of the PDF could not be read", e),
    );
  };
  return { range: transport };
}
