import { describe, expect, it } from "vitest";
import { DomSpeech } from "./dom";

describe("read aloud over HTML", () => {
  it("reads blocks sentence by sentence, with maths as words", async () => {
    const root = document.createElement("article");
    root.innerHTML = `
      <h1>Light</h1>
      <p>The lamp burns oil. Its flame is <em>bright</em>.</p>
      <p>The energy is <span class="katex"><span class="katex-mathml"><math><semantics><mrow><mi>m</mi><msup><mi>c</mi><mn>2</mn></msup></mrow><annotation encoding="application/x-tex">mc^2</annotation></semantics></math></span><span class="katex-html">mc2</span></span> here.</p>`;
    document.body.append(root);
    const shown: string[] = [];
    const s = new DomSpeech({ root }, (r) => shown.push(r.toString()));
    const said: string[] = [];
    for (let p = await s.next(); p; p = await s.next()) {
      said.push(p.text);
      s.show(p, true);
    }
    expect(said).toEqual([
      "Light",
      "The lamp burns oil.",
      "Its flame is bright.",
      "The energy is m c squared here.",
    ]);
    expect(shown[1]).toBe("The lamp burns oil.");
    root.remove();
  });

  it("starts from a given element", async () => {
    const root = document.createElement("div");
    root.innerHTML = "<p>One.</p><p id='two'>Two.</p><p>Three.</p>";
    document.body.append(root);
    const s = new DomSpeech({ root, from: root.querySelector("#two")! }, () => {});
    expect((await s.next())?.text).toBe("Two.");
    expect((await s.next())?.text).toBe("Three.");
    expect(await s.next()).toBeNull();
    root.remove();
  });
});
