import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap } from "@/lib/ipc";

export const dictionariesKey = ["spell", "dictionaries"] as const;
export const ownWordsKey = ["spell", "own-words"] as const;

export function useDictionaries() {
  return useQuery({ queryKey: dictionariesKey, queryFn: () => commands.spellDictionaries() });
}

export function useDownloadDictionary() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => unwrap(commands.downloadDictionary(code)),
    onSettled: () => void qc.invalidateQueries({ queryKey: dictionariesKey }),
  });
}

export function useRemoveDictionary() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (code: string) => unwrap(commands.removeDictionary(code)),
    onSuccess: (d) => qc.setQueryData(dictionariesKey, d),
  });
}

export function cancelDictionaryDownload(code: string) {
  return commands.cancelDictionaryDownload(code);
}

export function useOwnWords() {
  return useQuery({ queryKey: ownWordsKey, queryFn: () => unwrap(commands.ownWords()) });
}

export function useRemoveOwnWord() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (word: string) => unwrap(commands.removeOwnWord(word)),
    onSuccess: (w) => qc.setQueryData(ownWordsKey, w),
  });
}

export const spellCheck = (text: string, languages: string[], book: string | null) =>
  unwrap(commands.spellCheck(text, languages, book));
export const spellSuggest = (word: string, languages: string[], book: string | null) =>
  unwrap(commands.spellSuggest(word, languages, book));
export const spellComplete = (prefix: string, languages: string[], book: string | null) =>
  unwrap(commands.spellComplete(prefix, languages, book));
export const addOwnWord = (word: string) => unwrap(commands.addOwnWord(word));
export const learnBook = (book: string) => commands.spellLearnBook(book);
