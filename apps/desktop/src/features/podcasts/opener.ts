/** How Podcasts opens Settings › Online details (set by the app shell,
 * which owns navigation). */
let opener: (() => void) | null = null;

export function setPodcastSettingsOpener(fn: () => void) {
  opener = fn;
}

export function openPodcastSettings() {
  opener?.();
}
