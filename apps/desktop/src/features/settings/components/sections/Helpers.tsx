import { HelpersList, useHelpers } from "@/features/helpers";
import { Group } from "../parts";
import { OcrEnginesGroup } from "./OcrEngines";
import { OcrLanguagesGroup } from "./Search";

/** Settings › Helper programs. */
export function HelperSettings() {
  const flatpak = useHelpers().data?.platform === "flatpak";
  return (
    <>
      <Group
        title="Helper programs"
        scope="computer"
        description={
          flatpak
            ? "Open-source programs Libreri runs for DjVu books, reading scanned pages (OCR) and reading aloud. They come built into the Flatpak version."
            : "Open-source programs Libreri runs for DjVu books, reading scanned pages (OCR) and ACE comics. Libreri can install them for you with your computer's package manager."
        }
      >
        <HelpersList />
      </Group>
      <OcrEnginesGroup />
      <OcrLanguagesGroup />
    </>
  );
}
