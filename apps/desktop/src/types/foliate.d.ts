// foliate-js ships plain JavaScript modules without type declarations.
// The shapes Libreri relies on are described in readers/ebook/EbookRenderer.ts.
declare module "foliate-js/view.js";
declare module "foliate-js/overlayer.js";
declare module "foliate-js/epubcfi.js" {
  type Filter = (node: Node) => number;
  interface RangeLike {
    startContainer: Node;
    startOffset: number;
    endContainer: Node;
    endOffset: number;
    collapsed: boolean;
  }
  interface Parsed {
    parent?: unknown[];
    shift(): unknown;
  }
  export function parse(cfi: string): Parsed & { parent?: { shift(): unknown } };
  export function fromRange(range: RangeLike, filter?: Filter): string;
  export function toRange(doc: Document, parts: unknown, filter?: Filter): Range;
  export function joinIndir(...parts: string[]): string;
}
