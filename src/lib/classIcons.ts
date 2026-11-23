// Official class icons (data URLs from the backend, which loads them from
// Census and caches them). Empty until they are cached or without Tauri (OBS
// source, browser preview): ClassIcon then draws its own CC0 symbols.
import { writable } from "svelte/store";
import { api, inTauri, on } from "./api";
import type { ClassId } from "./types";

export const classIcons = writable<Partial<Record<ClassId, string>>>({});

let started = false;

/** Loads the icons once per window and again when the backend has new ones. */
export function loadClassIcons(): void {
  if (started || !inTauri()) return;
  started = true;
  const load = () =>
    api
      .classIcons()
      .then((m) => classIcons.set(m))
      .catch(() => {});
  load();
  on("class-icons", load).catch(() => {});
}
