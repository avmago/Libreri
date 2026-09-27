import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { commands, unwrap, type Annotation } from "@/lib/ipc";

const key = ["lib", "notes"] as const;

/** Notes shown in open readers and in the hub both refresh after a change. */
function refresh(qc: QueryClient, bookId?: string) {
  void qc.invalidateQueries({ queryKey: key });
  if (bookId) void qc.invalidateQueries({ queryKey: ["lib", "reader", bookId, "annotations"] });
}

export function useAllNotes() {
  return useQuery({ queryKey: [...key, "all"], queryFn: () => unwrap(commands.listAllNotes()) });
}

export function useNotebookList() {
  return useQuery({
    queryKey: [...key, "notebooks"],
    queryFn: () => unwrap(commands.listNotebooks()),
  });
}

export function useNoteFile(relPath: string | null) {
  return useQuery({
    queryKey: [...key, "file", relPath],
    queryFn: () => unwrap(commands.readNote(relPath!)),
    enabled: relPath !== null,
    staleTime: Infinity,
  });
}

export function useWriteNote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ relPath, content }: { relPath: string; content: string }) =>
      unwrap(commands.writeNote(relPath, content)),
    onSuccess: (_, { relPath, content }) => {
      qc.setQueryData([...key, "file", relPath], content);
      void qc.invalidateQueries({ queryKey: [...key, "notebooks"] });
      // A book's notebook may be open in a reader too.
      void qc.invalidateQueries({
        predicate: (q) => q.queryKey[1] === "reader" && q.queryKey[3] === "notebook",
      });
    },
  });
}

export function useCreateNote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (title: string) => unwrap(commands.createNote(title)),
    onSuccess: () => void qc.invalidateQueries({ queryKey: [...key, "notebooks"] }),
  });
}

export function useUpdateNote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: Annotation) => unwrap(commands.saveAnnotation(a)),
    onSuccess: (a) => refresh(qc, a.bookId),
  });
}

export function useRemoveNote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: Annotation) => unwrap(commands.deleteAnnotation(a.id)),
    onSuccess: (_, a) => refresh(qc, a.bookId),
  });
}
