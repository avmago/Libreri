import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap, type Annotation } from "@/lib/ipc";

const key = (bookId: string) => ["lib", "reader", bookId] as const;

/** Where the saved reading position is cached. */
export const positionKey = (bookId: string) => [...key(bookId), "position"] as const;

export function usePosition(bookId: string) {
  return useQuery({
    queryKey: positionKey(bookId),
    queryFn: () => unwrap(commands.getPosition(bookId)),
    // Read once when the book opens; the reader owns the position after that.
    staleTime: Infinity,
    gcTime: 0,
  });
}

export function savePosition(bookId: string, locator: string, progress: number) {
  return unwrap(commands.savePosition(bookId, locator, progress));
}

export function useAnnotations(bookId: string) {
  return useQuery({
    queryKey: [...key(bookId), "annotations"],
    queryFn: () => unwrap(commands.listAnnotations(bookId)),
  });
}

export function useSaveAnnotation(bookId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: Annotation) => unwrap(commands.saveAnnotation(a)),
    onSuccess: (saved) =>
      qc.setQueryData<Annotation[]>([...key(bookId), "annotations"], (list = []) => {
        const rest = list.filter((a) => a.id !== saved.id);
        return [...rest, saved].sort((a, b) => (a.position ?? 0) - (b.position ?? 0));
      }),
  });
}

export function useDeleteAnnotation(bookId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.deleteAnnotation(id)),
    onSuccess: (_, id) =>
      qc.setQueryData<Annotation[]>([...key(bookId), "annotations"], (list = []) =>
        list.filter((a) => a.id !== id),
      ),
  });
}

export function useNotebook(bookId: string, enabled: boolean) {
  return useQuery({
    queryKey: [...key(bookId), "notebook"],
    queryFn: () => unwrap(commands.getNotebook(bookId)),
    enabled,
    staleTime: Infinity,
  });
}

export function useSaveNotebook(bookId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (content: string) => unwrap(commands.saveNotebook(bookId, content)),
    onSuccess: (nb) => qc.setQueryData([...key(bookId), "notebook"], nb),
  });
}
