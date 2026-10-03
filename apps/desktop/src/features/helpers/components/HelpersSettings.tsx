import { CheckCircle2, CircleAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useHelpers } from "../api";
import { useHelperDialog } from "../store";

/** Settings › Helpers: DjVuLibre, Tesseract and unar, installed or not. */
export function HelpersList() {
  const { data, refetch, isFetching } = useHelpers();
  const open = useHelperDialog((s) => s.open);
  if (!data) return <p className="px-4 py-3 text-muted-foreground">Checking…</p>;
  return (
    <>
      {data.helpers.map((h) => (
        <div key={h.helper} className="flex items-center gap-6 px-4 py-3">
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="flex items-center gap-2 font-medium">
              {h.installed ? (
                <CheckCircle2 className="size-4 text-green-600" aria-label="Installed" />
              ) : (
                <CircleAlert className="size-4 text-amber-600" aria-label="Not installed" />
              )}
              {h.name}
              {h.version && (
                <span className="text-[12px] font-normal text-muted-foreground">{h.version}</span>
              )}
            </span>
            <span className="text-[12.5px] text-muted-foreground">{h.purpose}</span>
          </div>
          <Button variant="outline" size="sm" onClick={() => open(h.helper)}>
            {h.installed || data.platform === "flatpak" ? "Details" : "Install…"}
          </Button>
        </div>
      ))}
      <div className="flex justify-end px-4 py-2">
        <Button variant="ghost" size="sm" disabled={isFetching} onClick={() => void refetch()}>
          Check again
        </Button>
      </div>
    </>
  );
}
