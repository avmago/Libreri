import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap } from "@/lib/ipc";

/** Under the library's key, so switching libraries forgets it. */
export const searchKey = ["lib", "search"] as const;
export const ocrLanguagesKey = ["ocr-languages"] as const;

export function useTextSearch(query: string) {
  const q = query.trim();
  return useQuery({
    queryKey: [...searchKey, "text", q],
    queryFn: () => unwrap(commands.searchText(q, 100)),
    enabled: q.length >= 2,
    staleTime: 30_000,
    placeholderData: keepPreviousData,
  });
}

export function useBookHits(id: string, query: string, enabled: boolean) {
  return useQuery({
    queryKey: [...searchKey, "book", id, query.trim()],
    queryFn: () => unwrap(commands.searchInBook(id, query.trim(), 500)),
    enabled,
    staleTime: 30_000,
  });
}

export function useTextStatus(id: string | null) {
  return useQuery({
    queryKey: [...searchKey, "status", id],
    queryFn: () => unwrap(commands.textStatus(id!)),
    enabled: !!id,
  });
}

export function useIndexStatus() {
  return useQuery({
    queryKey: [...searchKey, "index"],
    queryFn: () => unwrap(commands.searchIndexStatus()),
  });
}

export function useBooksWithoutText() {
  return useQuery({
    queryKey: [...searchKey, "without-text"],
    queryFn: () => unwrap(commands.booksWithoutText()),
    staleTime: 60_000,
  });
}

export function useRebuildIndex() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.rebuildSearchIndex()),
    onSettled: () => void qc.invalidateQueries({ queryKey: searchKey }),
  });
}

export function useMakeSearchable() {
  return useMutation({
    mutationFn: (a: { ids: string[]; languages: string[]; redo: boolean }) =>
      unwrap(commands.makeSearchable(a.ids, a.languages, a.redo)),
  });
}

export function useForgetOcr() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.forgetOcr(id)),
    onSettled: () => void qc.invalidateQueries({ queryKey: searchKey }),
  });
}

export function useOcrLanguages() {
  return useQuery({
    queryKey: ocrLanguagesKey,
    queryFn: () => unwrap(commands.ocrLanguages()),
  });
}

export function useSetOcrLanguages() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (languages: string[]) => unwrap(commands.setOcrLanguages(languages)),
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: ocrLanguagesKey });
      void qc.invalidateQueries({ queryKey: searchKey });
    },
  });
}

export function useDownloadOcrLanguage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => unwrap(commands.downloadOcrLanguage(code)),
    onSettled: () => void qc.invalidateQueries({ queryKey: ocrLanguagesKey }),
  });
}

export function useRemoveOcrLanguage() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => unwrap(commands.removeOcrLanguage(code)),
    onSettled: () => void qc.invalidateQueries({ queryKey: ocrLanguagesKey }),
  });
}

export const ocrEnginesKey = ["ocr-engines"] as const;

/** Tesseract and the downloadable OCR models (ADR 0029). */
export function useOcrEngines() {
  return useQuery({
    queryKey: ocrEnginesKey,
    queryFn: () => commands.ocrEngines(),
  });
}

function useOcrEngineMutation<A>(fn: (a: A) => Promise<unknown>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSettled: () => void qc.invalidateQueries({ queryKey: ocrEnginesKey }),
  });
}

export function useSetOcrEngine() {
  return useOcrEngineMutation((id: string) => unwrap(commands.setOcrEngine(id)));
}

export function useDownloadOcrModel() {
  return useOcrEngineMutation((id: string) => unwrap(commands.downloadOcrModel(id)));
}

export function useRemoveOcrModel() {
  return useOcrEngineMutation((id: string) => unwrap(commands.removeOcrModel(id)));
}
