import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { events, type SessionDto } from "@/lib/ipc";
import { sessionKey } from "../api";

/**
 * Follows sign-ins and locks made in other windows: when someone else is
 * signed in now, this window drops what it showed and asks again.
 */
export function useSessionEvents() {
  const qc = useQueryClient();
  useEffect(() => {
    const off = events.sessionChanged.listen((e) => {
      const current = qc.getQueryData<SessionDto | null>(sessionKey);
      if ((current?.profile.id ?? null) === (e.payload.profileId ?? null)) return;
      qc.removeQueries({ queryKey: ["lib"], predicate: (q) => q.queryKey[1] !== "session" });
      void qc.invalidateQueries({ queryKey: sessionKey });
    });
    return () => void off.then((f) => f());
  }, [qc]);
}
