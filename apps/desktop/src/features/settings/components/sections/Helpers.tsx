import { HelpersList } from "@/features/helpers";
import { Group } from "../parts";
import { OcrLanguagesGroup } from "./Search";

/** Settings › Helper programs. */
export function HelperSettings() {
  return (
    <>
      <Group
        title="Helper programs"
        scope="computer"
        description="Open-source programs Libreri runs for DjVu books, reading scanned pages (OCR) and ACE comics. Libreri can install them for you with your computer's package manager."
      >
        <HelpersList />
      </Group>
      <OcrLanguagesGroup />
    </>
  );
}
