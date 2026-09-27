import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  unwrap,
  type BookMetadata,
  type OnlineChange,
  type Query,
  type Source,
} from "@/lib/ipc";

const libKey = ["lib"] as const;
const onlineKey = ["online-settings"] as const;

/** What a book would be looked up by (filled into the search form). */
export function useDetailsQuery(id: string | null) {
  return useQuery({
    queryKey: [...libKey, "details-query", id],
    queryFn: () => unwrap(commands.detailsQuery(id!)),
    enabled: id !== null,
    staleTime: Infinity,
  });
}

export function useFindDetails() {
  return useMutation({
    mutationFn: ({ id, query }: { id: string; query: Query | null }) =>
      unwrap(commands.findDetails(id, query)),
  });
}

export function useApplyDetails() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      metadata,
      coverUrl,
    }: {
      id: string;
      metadata: BookMetadata;
      coverUrl: string | null;
    }) => unwrap(commands.applyDetails(id, metadata, coverUrl)),
    onSuccess: () => qc.invalidateQueries({ queryKey: libKey }),
  });
}

/** A source's cover as a data: URL (fetched by Rust from allowed hosts only). */
export function useCoverPreview(url: string | null) {
  return useQuery({
    queryKey: ["cover-preview", url],
    queryFn: () => unwrap(commands.coverPreview(url!)),
    enabled: url !== null,
    staleTime: Infinity,
    retry: false,
  });
}

export function useFillMissingDetails() {
  return useMutation({
    mutationFn: (ids: string[]) => unwrap(commands.fillMissingDetails(ids)),
  });
}

export function useOnlineSettings() {
  return useQuery({ queryKey: onlineKey, queryFn: () => commands.getOnlineSettings() });
}

export function useSetOnlineSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (change: OnlineChange) => unwrap(commands.setOnlineSettings(change)),
    onSuccess: (s) => qc.setQueryData(onlineKey, s),
  });
}

export function useSetSourceKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ source, key }: { source: Source; key: string | null }) =>
      unwrap(commands.setSourceKey(source, key)),
    onSuccess: (s) => qc.setQueryData(onlineKey, s),
  });
}
