import { toast } from "sonner";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { commands, unwrap } from "@/lib/ipc";

const PICTURES = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];

/** Asks for a picture file and reads it, made smaller when large. */
export async function pickPicture(): Promise<{ src: string; aspect: number } | null> {
  const path = await openDialog({
    multiple: false,
    filters: [{ name: "Pictures", extensions: PICTURES }],
  });
  if (typeof path !== "string") return null;
  try {
    const p = await unwrap(commands.readPicture(path));
    return { src: p.src, aspect: p.width / Math.max(1, p.height) };
  } catch (e) {
    toast.error("The picture could not be read", { description: String(e) });
    return null;
  }
}
