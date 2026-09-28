import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap } from "@/lib/ipc";

export const canvasesKey = ["canvases"] as const;
export type Paper = "plain" | "lined" | "grid" | "dotted";
export const PAPERS: { id: Paper; label: string }[] = [
  { id: "plain", label: "Plain" },
  { id: "lined", label: "Lined" },
  { id: "grid", label: "Squared" },
  { id: "dotted", label: "Dotted" },
];

/** The profile's canvases (a book's, when `book` is given). */
export function useCanvases(book: string | null) {
  return useQuery({
    queryKey: [...canvasesKey, book],
    queryFn: () => unwrap(commands.canvases(book)),
  });
}

export function useCreateCanvas() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: { title: string; book: string | null; paper: Paper }) =>
      unwrap(commands.createCanvas(a.title, a.book, a.paper)),
    onSettled: () => void qc.invalidateQueries({ queryKey: canvasesKey }),
  });
}

export function useDeleteCanvas() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => unwrap(commands.deleteCanvas(path)),
    onSettled: () => void qc.invalidateQueries({ queryKey: canvasesKey }),
  });
}

export function useRenameCanvas() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: { path: string; title: string }) =>
      unwrap(commands.renameCanvas(a.path, a.title)),
    onSettled: () => void qc.invalidateQueries({ queryKey: canvasesKey }),
  });
}

export const readCanvas = (path: string) => unwrap(commands.readCanvas(path));
export const writeCanvas = (path: string, content: string) =>
  unwrap(commands.writeCanvas(path, content));
export const setCanvasPaper = (path: string, paper: Paper) =>
  unwrap(commands.setCanvasPaper(path, paper));
export const inkToText = (png: string, lang: string | null) =>
  unwrap(commands.inkToText(png, lang));

export function useInkSettings() {
  return useQuery({ queryKey: ["ink"], queryFn: () => commands.inkSettings() });
}

export function useSetInkEngine() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (engine: "system" | "tesseract") => unwrap(commands.setInkEngine(engine)),
    onSuccess: (d) => qc.setQueryData(["ink"], d),
  });
}
