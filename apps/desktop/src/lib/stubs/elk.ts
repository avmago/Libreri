/**
 * Stands in for elkjs, which Mermaid uses only for diagrams that ask for
 * `layout: elk`. elkjs is under the Eclipse Public License 2.0, which is
 * not compatible with Libreri's GPL, so it is not included: such diagrams
 * show an error, and every other diagram (the default layout) draws as usual.
 */
export default class ELK {
  layout(): Promise<never> {
    return Promise.reject(
      new Error(
        "The ELK layout is not included in Libreri; remove “layout: elk” from the diagram.",
      ),
    );
  }
}
