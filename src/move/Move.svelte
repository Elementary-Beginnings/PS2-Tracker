<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, inTauri, on } from "../lib/api";
  import type { MovedObject, MoveLayerState } from "../lib/types";
  import { sameArea, snap, toCss, type Rect } from "../lib/movelayer";
  import { demoMoveLayer } from "../lib/demo";
  import { t } from "../lib/i18n";
  import Overlay from "../overlay/Overlay.svelte";
  import Popup from "../popup/Popup.svelte";

  // Move layer: a dimmed window over the whole game monitor. Both objects are
  // drawn at their real position and size; dragging happens here with pointer
  // events (no window-manager move). The backend keeps the session; this page
  // reports drag ends and the bar buttons. Keys (Tab, arrows, Enter, Esc) are
  // global hotkeys of the backend, so they work even without focus here.
  type Key = "stats" | "popup";
  const params = new URLSearchParams(location.search);
  const demo = !inTauri() && params.has("demo");
  const dpr = window.devicePixelRatio || 1;
  let s = $state<MoveLayerState | null>(demo ? demoMoveLayer(params) : null);
  let grid = $state(false);
  let error = $state("");
  let drag = $state<{ key: Key; x: number; y: number; gx: number[]; gy: number[] } | null>(null);
  /** Update from the backend while dragging: applied after the drag. */
  let deferred: MoveLayerState | null = null;

  /** Pointer movement keeps the move mode alive (idle timeout), at most every 5 s. */
  let lastActivity = 0;
  function activity() {
    const now = Date.now();
    if (demo || now - lastActivity < 5000) return;
    lastActivity = now;
    api.moveLayerActivity().catch(() => {});
  }

  onMount(() => {
    if (!inTauri()) return;
    api.moveLayerState().then((v) => (s = v)).catch((e) => (error = errorText(e)));
    const un = on<MoveLayerState>("move-layer-update", (v) => {
      if (drag) deferred = v;
      else s = v;
    });
    return () => {
      un.then((f) => f());
    };
  });

  const area = $derived<Rect | null>(s ? { x: s.monitor.x, y: s.monitor.y, w: s.monitor.width, h: s.monitor.height } : null);
  const objOf = (k: Key): MovedObject | null => (s ? s[k] : null);
  const here = (k: Key) => {
    const o = objOf(k);
    return !!(o && s && sameArea(o.area, s.monitor));
  };
  const rectOf = (k: Key): Rect | null => {
    const o = objOf(k);
    if (!o) return null;
    const p: [number, number] = drag && drag.key === k ? [drag.x, drag.y] : o.pos;
    return { x: p[0], y: p[1], w: o.size[0], h: o.size[1] };
  };
  const label = (k: Key) => (k === "stats" ? t.overlay.frameLabel : t.overlay.popupLabel);
  const otherMonitors = $derived(s ? s.monitors.filter((m) => !sameArea(objOf(s!.selected as Key)?.area ?? [0, 0, 0, 0], m)) : []);

  function select(k: Key) {
    if (!s || s.selected === k) return;
    s.selected = k;
    if (!demo) api.moveLayerSelect(k).catch(() => {});
  }

  function down(e: PointerEvent, k: Key) {
    if (e.button !== 0 || !s || !area) return;
    const o = objOf(k);
    if (!o) return;
    select(k);
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    e.preventDefault();
    const sx = e.clientX;
    const sy = e.clientY;
    const [ox, oy] = o.pos;
    drag = { key: k, x: ox, y: oy, gx: [], gy: [] };
    const other = (["stats", "popup"] as Key[]).filter((x) => x !== k && here(x)).map((x) => rectOf(x)!);
    const move = (ev: PointerEvent) => {
      if (!area) return;
      const want = { x: Math.round(ox + (ev.clientX - sx) * dpr), y: Math.round(oy + (ev.clientY - sy) * dpr) };
      const r = snap(want, { w: o.size[0], h: o.size[1] }, area, other, { threshold: Math.round(12 * dpr), grid: grid ? 10 : 0, enabled: !ev.altKey });
      drag = { key: k, x: r.x, y: r.y, gx: r.guidesX, gy: r.guidesY };
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      const d = drag;
      drag = null;
      if (d && s) {
        s[k].pos = [d.x, d.y];
        if (!demo) api.moveLayerSet(k, d.x, d.y).catch((err) => (error = errorText(err)));
      }
      if (deferred) {
        s = { ...deferred, [k]: { ...deferred[k], pos: d ? [d.x, d.y] : deferred[k].pos } };
        deferred = null;
      }
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  }

  const end = (save: boolean) => !demo && api.moveLayerEnd(save).catch((e) => (error = errorText(e)));
  const reset = () => s && !demo && api.moveLayerReset(s.selected).catch((e) => (error = errorText(e)));
  function toMonitor(name: string) {
    if (!s || !name || demo) return;
    api.moveLayerMonitor(s.selected, name).catch((e) => (error = errorText(e)));
  }

  /** Position shown while dragging: relative to the monitor, physical px. */
  const coords = $derived(drag && s ? `${drag.x - s.monitor.x}, ${drag.y - s.monitor.y}` : "");
  const grabbed = $derived(s?.pointer_free === false);
</script>

<div class="layer" class:demo role="application" aria-label={t.move.title} onpointermove={activity}>
  {#if s && area}
    {#each ["stats", "popup"] as const as k (k)}
      {#if here(k)}
        {@const r = rectOf(k)!}
        <div
          class="obj"
          class:sel={s.selected === k}
          class:dragging={drag?.key === k}
          style:left={`${toCss(r.x, area.x, dpr)}px`}
          style:top={`${toCss(r.y, area.y, dpr)}px`}
          style:width={`${r.w / dpr}px`}
          style:height={`${r.h / dpr}px`}
          role="button"
          tabindex="-1"
          aria-label={label(k)}
          onpointerdown={(e) => down(e, k)}
        >
          <div class="content" aria-hidden="true">
            {#if k === "stats"}
              <Overlay preview />
            {:else}
              <Popup preview />
            {/if}
          </div>
          <span class="tag">{label(k)}</span>
          {#if drag?.key === k}<span class="coords">{coords}</span>{/if}
        </div>
      {/if}
    {/each}
    {#if drag}
      {#each drag.gx as gx (gx)}<div class="guide v" style:left={`${toCss(gx, area.x, dpr)}px`}></div>{/each}
      {#each drag.gy as gy (gy)}<div class="guide h" style:top={`${toCss(gy, area.y, dpr)}px`}></div>{/each}
    {/if}

    <div class="bar" role="toolbar" aria-label={t.move.title}>
      <strong>{t.move.title}</strong>
      <span class="muted">{t.move.selected(label(s.selected as Key))}</span>
      <button class="primary" onclick={() => end(true)}>{t.move.save}</button>
      <button onclick={() => end(false)}>{t.move.cancel}</button>
      <button onclick={reset} title={t.move.resetHint}>{t.move.reset}</button>
      <label class="check"><input type="checkbox" bind:checked={grid} /> {t.move.grid}</label>
      {#if otherMonitors.length}
        <select aria-label={t.move.toMonitor} onchange={(e) => { toMonitor(e.currentTarget.value); e.currentTarget.value = ""; }}>
          <option value="">{t.move.toMonitor}</option>
          {#each otherMonitors as m (m.name)}<option value={m.name}>{m.name} ({m.width}×{m.height})</option>{/each}
        </select>
      {/if}
      <span class="keys">{t.move.keys(grabbed, s.hotkey_finish || "Ctrl+Escape")}</span>
      {#if s.failed_hotkeys.includes(s.hotkey_edit)}<span class="warn" role="alert">{t.move.taken(s.hotkey_edit, s.hotkey_finish)}</span>{/if}
      {#if s.failed_hotkeys.includes(s.hotkey_finish)}<span class="warn" role="alert">{t.move.finishTaken(s.hotkey_finish)}</span>{/if}
      {#if s.idle_timeout_secs > 0}<span class="away">{t.move.idle(s.idle_timeout_secs)}</span>{/if}
      {#each ["stats", "popup"] as const as k (k)}
        {#if !here(k)}<span class="away">{t.move.elsewhere(label(k), objOf(k)?.monitor ?? "")}</span>{/if}
      {/each}
      {#if error}<span class="err" role="alert">{error}</span>{/if}
    </div>

    {#if grabbed}
      <div class="grab" role="alert">
        <strong>{t.move.grabTitle}</strong>
        <span>{t.move.grabText}</span>
      </div>
    {/if}
  {:else}
    <div class="grab"><span>{error || t.move.loading}</span></div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
    overflow: hidden;
    user-select: none;
    width: 100%;
    height: 100%;
  }
  .layer {
    position: fixed;
    inset: 0;
    background: rgb(5 8 12 / 55%);
    font-family: "Inter", "Noto Sans", "Segoe UI", system-ui, sans-serif;
    color: #e7e9ee;
    cursor: default;
  }
  .demo {
    background:
      linear-gradient(rgb(5 8 12 / 55%), rgb(5 8 12 / 55%)),
      linear-gradient(160deg, #1d2a33 0%, #2e3b2a 50%, #0e1216 100%);
  }
  .obj {
    position: absolute;
    box-sizing: border-box;
    outline: 2px dashed rgb(255 255 255 / 75%);
    outline-offset: -1px;
    cursor: grab;
    touch-action: none;
  }
  .obj.sel {
    outline: 3px solid #4fc3c9;
  }
  .obj.dragging {
    cursor: grabbing;
  }
  .content {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }
  .tag,
  .coords {
    position: absolute;
    left: 0;
    font-size: 12px;
    font-weight: 700;
    padding: 2px 6px;
    white-space: nowrap;
  }
  .tag {
    top: -22px;
    background: #4fc3c9;
    color: #0b1014;
    border-radius: 4px 4px 0 0;
  }
  .coords {
    bottom: -22px;
    background: rgb(0 0 0 / 80%);
    color: #fff;
    border-radius: 0 0 4px 4px;
    font-variant-numeric: tabular-nums;
  }
  .guide {
    position: absolute;
    background: #ffd166;
    pointer-events: none;
  }
  .guide.v {
    top: 0;
    bottom: 0;
    width: 1px;
  }
  .guide.h {
    left: 0;
    right: 0;
    height: 1px;
  }
  .bar {
    position: absolute;
    left: 50%;
    bottom: 32px;
    transform: translateX(-50%);
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 8px 12px;
    max-width: min(92vw, 1100px);
    padding: 10px 14px;
    background: rgb(15 18 24 / 92%);
    border: 1px solid rgb(255 255 255 / 15%);
    border-radius: 10px;
    font-size: 14px;
    box-shadow: 0 6px 24px rgb(0 0 0 / 45%);
  }
  .bar button,
  .bar select {
    font: inherit;
    color: inherit;
    background: rgb(255 255 255 / 8%);
    border: 1px solid rgb(255 255 255 / 20%);
    border-radius: 6px;
    padding: 5px 12px;
    cursor: pointer;
  }
  .bar select option {
    background: #0f1218;
  }
  .bar button.primary {
    background: #4fc3c9;
    border-color: #4fc3c9;
    color: #0b1014;
    font-weight: 700;
  }
  .bar button:focus-visible,
  .bar select:focus-visible,
  .bar input:focus-visible {
    outline: 2px solid #ffd166;
    outline-offset: 2px;
  }
  .muted,
  .keys,
  .away {
    color: #b3bac6;
  }
  .keys {
    flex-basis: 100%;
    text-align: center;
    font-size: 13px;
  }
  .err,
  .warn {
    color: #ffa557;
  }
  .warn {
    flex-basis: 100%;
    text-align: center;
    font-weight: 600;
  }
  .check {
    display: inline-flex;
    gap: 6px;
    align-items: center;
  }
  .grab {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: grid;
    gap: 10px;
    max-width: 70vw;
    padding: 28px 36px;
    text-align: center;
    background: rgb(15 18 24 / 94%);
    border: 2px solid #ffd166;
    border-radius: 14px;
    font-size: 22px;
    box-shadow: 0 8px 32px rgb(0 0 0 / 55%);
  }
  .grab strong {
    font-size: 30px;
    color: #ffd166;
  }
</style>
