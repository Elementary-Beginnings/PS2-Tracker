<script lang="ts">
  // "Game window" selection, shared by crosshair and overlay. Saves a stable
  // rule (WM_CLASS / title / process name), never a window id.
  import { onMount } from "svelte";
  import { t } from "./i18n";
  import { api, errorText, inTauri } from "./api";
  import type { Config, FocusInfo, GameStatus, WindowEntry } from "./types";

  let {
    config,
    save,
    notify,
  }: { config: Config; save: (c?: Config) => Promise<void>; notify: (t: string, e?: boolean) => void } = $props();

  const c = $derived(config.crosshair);
  let windows = $state<WindowEntry[]>([]);
  let status = $state<GameStatus | null>(null);
  let focus = $state<FocusInfo | null>(null);
  const id = "gw-select";

  const isAuto = $derived(!c.game_window.class && !c.game_window.title && !c.game_window.process);
  const selected = $derived(
    isAuto
      ? "auto"
      : String(
          windows.findIndex(
            (w) => c.game_window.class && w.class.toLowerCase().includes(c.game_window.class.toLowerCase()),
          ),
        ),
  );

  async function refresh() {
    if (!inTauri()) return;
    try {
      windows = await api.gameWindows();
    } catch (e) {
      notify(errorText(e), true);
    }
  }
  async function poll() {
    if (!inTauri()) return;
    status = await api.gameStatus().catch(() => null);
  }
  onMount(() => {
    refresh();
    poll();
    const timer = setInterval(poll, 2000);
    return () => clearInterval(timer);
  });

  async function choose(v: string) {
    if (v === "auto") {
      c.game_window = { class: "", title: "", process: "" };
    } else {
      const w = windows[Number(v)];
      if (!w) return;
      // WM_CLASS is stable across restarts; the title only if there is no class
      const cls = w.class.split(" ").pop() ?? "";
      c.game_window = { class: cls, title: cls ? "" : w.title, process: w.process };
    }
    await save();
    await poll();
  }

  async function test() {
    await poll();
    notify(status?.title ? t.gameWindow.testFound(status.title) : t.gameWindow.testNone, !status?.title);
  }
  async function probe() {
    try {
      focus = await api.focusProbe();
    } catch (e) {
      notify(errorText(e), true);
    }
  }
</script>

<div class="grid gw">
  <label for={id} class="muted small">{t.gameWindow.label}</label>
  <div class="row">
    <select {id} value={selected} onchange={(e) => choose(e.currentTarget.value)}>
      <option value="auto">{t.gameWindow.auto}</option>
      {#if !isAuto && selected === "-1"}
        <option value="-1">{t.gameWindow.saved(c.game_window.class || c.game_window.title || c.game_window.process)}</option>
      {/if}
      {#each windows as w, i (i)}
        <option value={String(i)}>{w.matches ? "★ " : ""}{w.title || "–"} · {w.class || w.process}</option>
      {/each}
    </select>
    <button onclick={refresh} disabled={!inTauri()}>↻ {t.gameWindow.refresh}</button>
    <button onclick={test} disabled={!inTauri()}>{t.gameWindow.test}</button>
  </div>
  <div class="small" role="status">
    {t.gameWindow.statusLabel}:
    <strong class="st {status?.state ?? 'unknown'}">{t.gameWindow.state[status?.state ?? "unknown"]}</strong>
    {#if status?.title}<span class="muted"> · {status.title}</span>{/if}
  </div>
  <p class="muted small">{t.gameWindow.hint}</p>
  <details class="section small">
    <summary>{t.voice.advanced}</summary>
    <label class="field">{t.crosshair.gameMatch}
      <input bind:value={c.game_match} onchange={() => save()} />
    </label>
    <div class="row">
      <button class="mini" onclick={probe} disabled={!inTauri()}>{t.crosshair.probe}</button>
      {#if focus}
        <span>{t.crosshair.focusState[focus.state]}</span>
        <span class="muted">· {focus.class || "–"} · {focus.title || "–"} · {t.crosshair.backendLabel(focus.backend)}</span>
      {/if}
    </div>
  </details>
</div>

<style>
  .gw select {
    min-width: 0;
    flex: 1;
  }
  .st.active {
    color: var(--good);
  }
  .st.background {
    color: var(--warn);
  }
</style>
