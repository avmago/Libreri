/**
 * The only door from the UI to Rust.
 *
 * `bindings.ts` is generated from the Rust commands (pnpm gen:ipc). Commands
 * that can fail return `{ status, data | error }`; `unwrap` turns that into a
 * normal promise that rejects with an `IpcError`, which suits TanStack Query.
 */
import { commands, events, type AppError, type AppErrorKind } from "./bindings";

export { commands, events };
export type * from "./bindings";

export class IpcError extends Error {
  readonly kind: AppErrorKind;

  constructor(error: AppError) {
    super(error.message);
    this.name = "IpcError";
    this.kind = error.kind;
  }
}

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

export async function unwrap<T>(result: Promise<Result<T>>): Promise<T> {
  const r = await result;
  if (r.status === "ok") return r.data;
  throw new IpcError(r.error);
}

export function isIpcError(e: unknown, kind?: AppErrorKind): e is IpcError {
  return e instanceof IpcError && (kind === undefined || e.kind === kind);
}

/** URL for a file inside the open library, served by the `book://` protocol. */
export function bookUrl(relativePath: string): string {
  const encoded = relativePath.split("/").map(encodeURIComponent).join("/");
  // Windows and Android expose custom protocols as http://<scheme>.localhost.
  const isWindows = navigator.userAgent.includes("Windows");
  return isWindows ? `http://book.localhost/${encoded}` : `book://localhost/${encoded}`;
}
