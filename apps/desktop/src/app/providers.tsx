import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";

export function Providers({ children }: { children: ReactNode }) {
  const [client] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          // Data comes from the local Rust core, not a network: no retries,
          // no refetch on focus. Rust events invalidate queries instead.
          queries: { retry: false, refetchOnWindowFocus: false, staleTime: Infinity },
          mutations: { retry: false },
        },
      }),
  );
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
