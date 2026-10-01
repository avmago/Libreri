import { commands } from "@/lib/ipc";

/** Opens the system print dialog for what the print styles show. */
export async function printWindow(): Promise<void> {
  try {
    const r = await commands.printWindow();
    if (r.status === "ok") return;
  } catch {
    /* outside the app (tests): the browser's own */
  }
  window.print();
}
