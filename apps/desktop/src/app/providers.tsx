import { QueryCache, QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";
import { isIpcError } from "@/lib/ipc";

export function Providers({ children }: { children: ReactNode }) {
  const [client] = useState(() => {
    const client: QueryClient = new QueryClient({
      // Anything that finds nobody signed in (auto-lock, another window)
      // sends the app back to the profile picker.
      queryCache: new QueryCache({
        onError: (e) => {
          if (isIpcError(e, "signedOut")) client.setQueryData(["lib", "session"], null);
        },
      }),
      defaultOptions: {
        // Data comes from the local Rust core, not a network: no retries,
        // no refetch on focus. Rust events invalidate queries instead.
        queries: { retry: false, refetchOnWindowFocus: false, staleTime: Infinity },
        mutations: { retry: false },
      },
    });
    return client;
  });
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
