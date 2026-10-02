/** What a renderer gives read aloud: the book's text, a sentence at a time. */
export interface SpeechPiece {
  text: string;
  /** Language, when the book says. */
  lang?: string;
}

export interface SpeechSource {
  /** The next sentence, or null at the end of the book. */
  next(): Promise<SpeechPiece | null>;
  /** The sentence `next` will give, when it is at hand without turning
   * to another chapter (to get it ready to be said). */
  peek?(): SpeechPiece | null;
  /** Marks the sentence being read; with `follow`, turns the page to it. */
  show(piece: SpeechPiece, follow: boolean): void;
  /** Removes the mark. */
  clear(): void;
}
