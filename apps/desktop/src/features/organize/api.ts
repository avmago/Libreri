import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap } from "@/lib/ipc";

export function useSimilarTags() {
  return useQuery({
    queryKey: ["lib", "organize", "similar"],
    queryFn: () => unwrap(commands.similarTags()),
  });
}

/** Every change here touches many books: refresh everything in the library. */
function useOrganizeMutation<A>(fn: (a: A) => Promise<number>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSettled: () => qc.invalidateQueries({ queryKey: ["lib"] }),
  });
}

export const useRenameTag = () =>
  useOrganizeMutation(({ from, to }: { from: string; to: string }) =>
    unwrap(commands.renameTag(from, to)),
  );
export const useMergeTags = () =>
  useOrganizeMutation(({ sources, into }: { sources: string[]; into: string }) =>
    unwrap(commands.mergeTags(sources, into)),
  );
export const useDeleteTag = () =>
  useOrganizeMutation((tag: string) => unwrap(commands.deleteTag(tag)));
export const useRenameCategory = () =>
  useOrganizeMutation(({ from, to }: { from: string; to: string }) =>
    unwrap(commands.renameCategory(from, to)),
  );
export const useDeleteCategory = () =>
  useOrganizeMutation((path: string) => unwrap(commands.deleteCategory(path)));
