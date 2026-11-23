// Version, commit and build date of the running app (Settings → Advanced →
// About and the version tag in the sidebar). Browser preview: "preview",
// no commit.
import { writable } from "svelte/store";
import { api, inTauri } from "./api";
import type { AppInfo } from "./types";

export const appInfo = writable<AppInfo | null>(null);

let started = false;

export function loadAppInfo(): void {
  if (started) return;
  started = true;
  if (!inTauri()) {
    appInfo.set({ version: "preview", commit: "unknown", build_date: "" });
    return;
  }
  api
    .appInfo()
    .then((i) => appInfo.set(i))
    .catch(() => {});
}

/** One line for bug reports. */
export function infoLine(i: AppInfo): string {
  return `PS2 Session Tracker ${i.version} (commit ${i.commit}${i.build_date ? `, built ${i.build_date}` : ""})`;
}
