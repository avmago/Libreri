// Before `pnpm tauri dev`: makes sure the installed packages match
// pnpm-lock.yaml, and installs them if not. After a pull that adds a
// package (Excalidraw, say), the app would otherwise start and
// fail only when that part is opened.
//
// pnpm keeps a copy of the lockfile it installed from in
// node_modules/.pnpm/lock.yaml; if that is missing or different, run
// `pnpm install --frozen-lockfile`.
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => {
  try {
    return readFileSync(join(root, p), "utf8");
  } catch {
    return null;
  }
};

const wanted = read("pnpm-lock.yaml");
const installed = read("node_modules/.pnpm/lock.yaml");
if (wanted === null || installed === wanted) process.exit(0);

console.log(
  installed === null
    ? "\n[libreri] Packages are not installed yet; running pnpm install…\n"
    : "\n[libreri] pnpm-lock.yaml changed since the last install; running pnpm install…\n",
);
const run = spawnSync("pnpm", ["install", "--frozen-lockfile"], {
  cwd: root,
  stdio: "inherit",
  shell: process.platform === "win32",
});
if (run.status !== 0) {
  console.error(
    "\n[libreri] pnpm install failed. Run it yourself in the Libreri folder and start again.\n",
  );
  process.exit(run.status ?? 1);
}
