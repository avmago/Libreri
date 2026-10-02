import { describe, expect, it } from "vitest";
import { chooseVoice } from "./choose";
import type { Voice } from "./engine";

const V = (id: string, lang: string, group: Voice["group"] = "system"): Voice => ({
  id,
  name: id,
  lang,
  group,
});
const voices = [
  V("kokoro:af_bella", "en-US", "kokoro"),
  V("kokoro:af_heart", "en-US", "kokoro"),
  V("piper:de_DE-thorsten-high", "de-DE", "piper"),
  V("Samantha", "en-US"),
  V("Anna", "de-DE"),
  V("Amelie", "fr-FR"),
];

describe("chooseVoice", () => {
  it("takes the voice chosen for the language", () => {
    expect(chooseVoice(voices, { voice: "Samantha", voiceFor: { de: "Anna" } }, "de")).toBe("Anna");
  });
  it("keeps the last voice when it speaks the book's language", () => {
    expect(chooseVoice(voices, { voice: "Samantha" }, "en-US")).toBe("Samantha");
  });
  it("suggests the best natural voice otherwise", () => {
    expect(chooseVoice(voices, { voice: null }, "en")).toBe("kokoro:af_heart");
    expect(chooseVoice(voices, { voice: "Samantha" }, "de-DE")).toBe("piper:de_DE-thorsten-high");
  });
  it("leaves it to the system when no natural voice speaks it", () => {
    expect(chooseVoice(voices, { voice: "Samantha" }, "fr")).toBeNull();
  });
  it("ignores voices that are gone", () => {
    expect(chooseVoice(voices, { voice: "kokoro:gone", voiceFor: { en: "x" } }, "en-US")).toBe(
      "kokoro:af_heart",
    );
  });
});
