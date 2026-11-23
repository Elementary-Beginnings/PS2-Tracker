<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, inTauri, on } from "../lib/api";
  import type { FeedRow, OverlaySettings, PackBackground, Snapshot, Toast } from "../lib/types";
  import { delta, duration, faction, feedIcon, feedLabel, feedTone, feedWho, num, pct } from "../lib/format";
  import { demoSnapshot } from "../lib/demo";
  import { t } from "../lib/i18n";
  import ClassIcon from "../lib/ClassIcon.svelte";

  /** preview: drawn inside the move layer (never reports a size, never hidden). */
  let { preview = false }: { preview?: boolean } = $props();

  const obs = !inTauri();
  const params = new URLSearchParams(location.search);
  /** Browser preview with demo data (screenshots): `?demo=1&preset=full&style=card&bg=light&faction=2&toast=1` */
  const demo = obs && params.has("demo");
  const defaults: OverlaySettings = {
    enabled: true, monitor: "", anchor: "top-right", x: 24, y: 24, width: 360, height: 420, size: "auto",
    style: "card", popup_anchor: "top-center", popup_x: 0, popup_y: 160, popup_width: 420, popup_height: 100,
    popup_scale: 1, popup_duration_ms: 4000, popup_text: true, popup_style: "plain", popup_opacity: 1,
    background_opacity: 0.7, scale: 1, click_through: true, show_stats: true, show_killfeed: true,
    show_weapons: true, show_toasts: true, show_comparison: true, show_my_class: false, hotkey_edit: "", hotkey_toggle: "",
    pack_background: true, compact: false, auto_hide: false, show_kd_revived: false,
    show_squad: false, squad_metric: "kd", squad_min_kills: 5, popup_monitor: "",
    hotkey_finish_move: "Ctrl+Escape", move_idle_timeout_secs: 300,
  };
  const PRESETS: Record<string, Partial<OverlaySettings>> = {
    minimal: { show_stats: true, show_comparison: false, show_killfeed: false, show_weapons: false, compact: true },
    streamer: { show_stats: true, show_comparison: false, show_killfeed: true, show_weapons: false, compact: false },
    full: { show_stats: true, show_comparison: true, show_killfeed: true, show_weapons: true, compact: false },
  };

  let bg = $state<PackBackground | null>(null);
  let o = $state<OverlaySettings>(defaults);
  let snap = $state<Snapshot | null>(null);
  /** Move mode: the real window shows nothing (the move layer draws it). */
  let edit = $state(false);
  let factor = $state(1);
  let toastNow = $state<(Toast & { id: number }) | null>(null);
  let now = $state(Math.floor(Date.now() / 1000));
  let root = $state<HTMLDivElement>();
  let tid = 0;
  /** Settings and size step loaded: only then the measured size is final. */
  let ready = $state(false);

  if (demo) {
    o = {
      ...defaults,
      ...PRESETS[params.get("preset") ?? "full"],
      style: params.get("style") ?? "card",
      background_opacity: Number(params.get("opacity") ?? defaults.background_opacity),
      show_squad: params.has("squad"),
      show_my_class: params.has("myclass"),
    };
    factor = Number(params.get("factor") ?? "1");
    snap = demoSnapshot(params);
    if (params.has("toast")) toastNow = { id: 1, title: "Double kill", text: "2 kills in 4 s", kind: "voice" };
  }

  /** One notification at a time in a slot of fixed size: new kills or
   *  notifications never change the window size. */
  function toast(x: Toast) {
    if (!o.show_toasts) return;
    const id = ++tid;
    toastNow = { ...x, id };
    setTimeout(() => {
      if (toastNow?.id === id) toastNow = null;
    }, 4500);
  }

  async function refreshState() {
    const s = await api.overlayState();
    edit = preview ? false : s.edit;
    factor = s.size_factor;
  }

  onMount(() => {
    const clock = setInterval(() => (now = Math.floor(Date.now() / 1000)), 1000);
    if (demo) return () => clearInterval(clock);
    if (obs) {
      fetch("/api/snapshot").then((r) => r.json()).then((s) => (snap = s)).catch(() => {});
      fetch("/api/background").then((r) => r.json()).then((b) => (bg = b)).catch(() => {});
      const es = new EventSource("/api/events");
      es.addEventListener("snapshot", (e) => (snap = JSON.parse((e as MessageEvent).data)));
      es.addEventListener("toast", (e) => toast(JSON.parse((e as MessageEvent).data)));
      return () => {
        clearInterval(clock);
        es.close();
      };
    }
    const un = [
      on<Snapshot | null>("snapshot", (s) => (snap = s)),
      on<boolean>("overlay-mode", (e) => (edit = preview ? false : e)),
      on<Toast>("toast", toast),
      on("config-changed", async () => {
        o = (await api.getConfig()).overlay;
        bg = await api.toastBackground();
        await refreshState();
      }),
    ];
    (async () => {
      o = (await api.getConfig()).overlay;
      bg = await api.toastBackground();
      snap = await api.getSnapshot();
      await refreshState();
      ready = true;
    })();
    return () => {
      clearInterval(clock);
      un.forEach((p) => p.then((f) => f()));
    };
  });

  // Report the content size (physical pixels) whenever something that changes
  // the layout changes; the backend resizes the window only if it differs.
  $effect(() => {
    // squad_metric/squad_min_kills are left out on purpose: fixed rows, same size
    void [o.show_stats, o.show_comparison, o.show_killfeed, o.show_weapons, o.compact, o.scale, o.style, factor, o.show_kd_revived, o.show_squad];
    if (obs || preview || !root || !ready) return;
    tick().then(() => {
      if (!root) return;
      const r = root.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      api.overlayContentSize("stats", Math.ceil(r.width * dpr), Math.ceil(r.height * dpr)).catch(() => {});
    });
  });

  const f = $derived(faction(snap?.faction_id));
  const dur = $derived(snap ? (snap.ended_at ?? now) - snap.started_at : 0);
  const feedRows = $derived(o.compact ? 3 : 5);
  const feed = $derived(Array.from({ length: feedRows }, (_, i) => snap?.killfeed[i] ?? null));
  const weapons = $derived(Array.from({ length: 4 }, (_, i) => snap?.weapons[i] ?? null));
  /** Always three rows: the block never changes the window size. */
  const squad = $derived(Array.from({ length: 3 }, (_, i) => snap?.squad_top?.[i] ?? null));
  const squadValue = (v: number) => (o.squad_metric === "kills" ? String(Math.round(v)) : num(v, 2));
  /** A row below the minimum kills shows its kills against the minimum. */
  const squadText = (r: { value: number; kills: number; qualified?: boolean }) =>
    r.qualified === false ? `${r.kills}/${o.squad_min_kills}` : squadValue(r.value);
  const squadNotRanked = $derived(squad.some((r) => r && r.qualified === false));
  const dash = "–";
  /** Feed rows about another player: they get the class symbol. */
  const personKinds = new Set(["kill", "death", "teamkill", "teamkilled"]);
  /** Another player is involved (not the environment, not a suicide). */
  const hasOther = (e: FeedRow) => personKinds.has(e.kind) && e.other_label !== "environment" && e.other_label !== "self";
  type Tile = { v: string; l: string };
  const tiles = $derived.by((): Tile[] => {
    const s = snap;
    const all: Tile[] = [
      { v: s ? num(s.rates.kd, 2) : dash, l: t.session.tiles.kd },
      ...(o.show_kd_revived ? [{ v: s ? num(s.rates.kd_revived ?? s.rates.kd, 2) : dash, l: t.overlay.kdRevivedTile }] : []),
      { v: s ? num(s.rates.kpm, 2) : dash, l: t.session.tiles.kpm },
      { v: s ? pct(s.rates.hsr, 0) : dash, l: t.session.tiles.hsr },
      { v: s ? String(s.streak) : dash, l: t.overlay.oStreak(s?.best_streak ?? 0) },
      { v: s ? String(s.stats.kills) : dash, l: t.session.tiles.kills },
      { v: s ? String(s.stats.deaths) : dash, l: t.session.tiles.deaths },
      { v: s ? String(s.stats.revives) : dash, l: t.session.tiles.revives },
      { v: s ? `${num(s.rates.xp_per_hour / 1000, 1)}k` : dash, l: t.session.tiles.xph },
    ];
    return o.compact ? all.slice(0, o.show_kd_revived ? 5 : 4) : all;
  });
  const cmp = $derived(
    snap?.comparison
      ? [
          { l: t.session.tiles.kd, d: snap.comparison.kd.delta_pct },
          { l: t.session.tiles.kpm, d: snap.comparison.kpm.delta_pct },
          { l: t.session.tiles.hsr, d: snap.comparison.hsr.delta_pct },
        ]
      : [],
  );
</script>

<div
  bind:this={root}
  class="ov style-{o.style}"
  class:ghost={edit && !preview}
  role="presentation"
  class:compact={o.compact}
  class:demo={demo && !preview}
  data-bg={demo && !preview ? (params.get("bg") ?? "dark") : null}
  style:--bgA={o.background_opacity}
  style:--fc={f.color}
  style:font-size={`${16 * factor * o.scale}px`}
>
  <div class="card" role="status" aria-label={t.overlay.title}>
    <div class="head">
      <span class="fac">{f.short}</span>
      <strong class="name">{snap?.character_name ?? t.overlay.waiting}</strong>
      <span class="dur">{snap ? duration(dur) : dash}</span>
    </div>

    {#if o.show_stats}
      <div class="tiles" class:four={o.compact}>
        {#each tiles as x, i (i)}
          <div class="tile"><b>{x.v}</b><span>{x.l}</span></div>
        {/each}
      </div>
    {/if}

    {#if o.show_comparison}
      <div class="cmp">
        {#each cmp.length ? cmp : [{ l: t.session.tiles.kd, d: null }, { l: t.session.tiles.kpm, d: null }, { l: t.session.tiles.hsr, d: null }] as c (c.l)}
          <span>{c.l} <em class:up={(c.d ?? 0) > 0} class:down={(c.d ?? 0) < 0}>{c.d == null ? dash : `${c.d > 0 ? "▲" : c.d < 0 ? "▼" : ""} ${delta(c.d)}`}</em></span>
        {/each}
        <span class="vs">{t.overlay.vsLifetime}</span>
      </div>
    {/if}

    {#if o.show_killfeed}
      <div class="feed">
        {#each feed as e, i (i)}
          <div class="fe {e ? feedTone(e.kind) : 'empty'}">
            {#if e}
              <span class="ic" aria-hidden="true">{feedIcon[e.kind] ?? "•"}</span>
              <span class="k">{feedLabel(e.kind)}</span>
              <span class="who" class:dim={!e.other_name && !e.vehicle_name}
                >{#if o.show_my_class && hasOther(e)}<ClassIcon cls={e.my_class} title={t.classes.mine} /><span class="sep">›</span>{/if}{#if hasOther(e)}<ClassIcon
                    cls={e.other_class}
                    title={t.classes.enemy}
                  />
                {/if}{feedWho(e)}</span
              >
              <span class="w">{e.weapon_name}</span>
              {#if e.headshot}
                <span class="hs" class:taken={feedTone(e.kind) !== "kill"}>{feedTone(e.kind) === "kill" ? t.overlay.hs : t.overlay.hsTaken}</span>
              {:else}<span></span>{/if}
            {:else}
              <span class="ic placeholder">·</span><span></span><span></span><span></span><span></span>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if o.show_squad}
      <div class="squad">
        <div class="sq-head"><span>{t.overlay.squadTitle}</span><span>{t.overlay.squadMetrics[o.squad_metric] ?? ""}{squadNotRanked ? ` · ${t.overlay.squadMin(o.squad_min_kills)}` : ""}</span></div>
        {#each squad as r, i (i)}
          {@const rf = faction(r?.faction_id)}
          <div class="sq" class:own={r?.own} class:empty={!r}>
            <span class="rank">{i + 1}</span>
            {#if r}
              <span class="sqf" style:background={rf.color}>{rf.short}</span><span class="who"><ClassIcon cls={r.class} title={t.classes.class} /> {r.name}</span><b class:dimmed={r.qualified === false}>{squadText(r)}</b>
            {:else}
              <span class="sqf none"></span><span class="who">{dash}</span><b></b>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if o.show_weapons && !o.compact}
      <div class="weapons">
        {#each weapons as w, i (i)}
          <div class="wr">
            <!-- empty rows keep their height (fixed window size) but stay invisible -->
            <span class="who" class:placeholder={!w}>{w?.name ?? dash}</span><span>{w?.kills ?? ""}</span><span class="w">{w ? pct(w.hsr, 0) : ""}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  {#if obs && o.show_toasts}
    <!-- OBS source only; in the app the pop-up is its own window -->
    <div class="slot">
      {#if toastNow}
        {#key toastNow.id}
          {#if o.pack_background && bg}
            <div class="toast packbg" style:background-image={`url(${bg.data_url})`}>
              <strong>{toastNow.title}</strong>{#if toastNow.text}<span>{toastNow.text}</span>{/if}
            </div>
          {:else}
            <div class="toast"><strong>{toastNow.title}</strong>{#if toastNow.text}<span>{toastNow.text}</span>{/if}</div>
          {/if}
        {/key}
      {/if}
    </div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    margin: 0;
    overflow: hidden;
    user-select: none;
  }
  /* Same tokens as the app (dark), see src/lib/app.css */
  .ov {
    --panel: 15 18 24;
    --text: #e7e9ee;
    --muted: #b3bac6;
    --good: #5fd99a;
    --bad: #ffa557;
    --warn: #e9b949;
    /* kill feed, same values as --kill/--death/--tk/--hs/--hs-taken in app.css (dark) */
    --kill: #5fd99a;
    --death: #ffa557;
    --tk: #e9b949;
    --hs: #7fd4ff;
    --hs-taken: #ff8a65;
    --up: #62b4ff;
    --down: #ffa557;
    position: absolute;
    top: 0;
    left: 0;
    display: inline-grid;
    gap: 0.4em;
    width: max-content;
    padding: 0.5em;
    color: var(--text);
    font-family: "Inter", "Noto Sans", "Segoe UI", system-ui, sans-serif;
    font-variant-numeric: tabular-nums;
    line-height: 1.25;
    /* thin contour: readable over bright and dark scenes */
    text-shadow:
      0 0 2px rgb(0 0 0 / 90%),
      0 1px 2px rgb(0 0 0 / 80%);
  }
  .card {
    display: grid;
    /* one column that never grows past the card (no clipping on the right) */
    grid-template-columns: minmax(0, 1fr);
    gap: 0.5em;
    width: 22em;
    padding: 0.55em 0.75em 0.65em;
    /* slightly darker at the bottom, where the small text sits */
    background: linear-gradient(rgb(var(--panel) / var(--bgA)), rgb(var(--panel) / min(1, calc(var(--bgA) + 0.08))));
    border: 1px solid color-mix(in srgb, var(--fc) 45%, rgb(255 255 255 / 12%));
    border-radius: 12px;
    box-shadow:
      0 4px 14px rgb(0 0 0 / 35%),
      inset 0 1px 0 rgb(255 255 255 / 6%);
    /* where supported (OBS/Chromium), blur bright scenery behind the card */
    backdrop-filter: blur(6px);
    position: relative;
    overflow: hidden;
  }
  .card::before {
    content: "";
    position: absolute;
    inset: 0 0 auto 0;
    height: 3px;
    background: var(--fc);
  }
  .compact .card {
    width: 17.5em;
  }
  .head {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 0.5em;
    align-items: center;
  }
  .fac {
    background: var(--fc);
    color: #fff;
    border-radius: 6px;
    padding: 0.05em 0.4em;
    font-size: 0.72em;
    font-weight: 800;
    letter-spacing: 0.04em;
    text-shadow: none;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 650;
  }
  .dur {
    font-size: 1.15em;
    font-weight: 700;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 0.35em 0.6em;
  }
  .tile {
    display: grid;
  }
  .tile b {
    font-size: 1.45em;
    font-weight: 750;
    line-height: 1.1;
  }
  .tile span,
  .vs {
    font-size: 0.7em;
    min-height: 1.2em;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    white-space: nowrap;
  }
  .cmp {
    display: flex;
    gap: 0.7em;
    align-items: baseline;
    font-size: 0.82em;
    border-top: 1px solid rgb(255 255 255 / 10%);
    padding-top: 0.4em;
  }
  .cmp span {
    white-space: nowrap;
  }
  .cmp em {
    font-style: normal;
    font-weight: 700;
  }
  .cmp .up {
    color: var(--up);
  }
  .cmp .down {
    color: var(--down);
  }
  .vs {
    margin-left: auto;
    font-size: 0.85em;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .placeholder {
    visibility: hidden;
  }
  .feed,
  .weapons,
  .squad {
    display: grid;
    gap: 0.12em;
    font-size: 0.84em;
    border-top: 1px solid rgb(255 255 255 / 10%);
    padding-top: 0.4em;
  }
  .fe {
    display: grid;
    grid-template-columns: 1.1em 4.7em minmax(0, 1fr) minmax(0, 0.9fr) 3.4em;
    gap: 0.4em;
    align-items: baseline;
    min-height: 1.3em;
  }
  .fe .ic,
  .fe .k {
    font-weight: 700;
  }
  .fe.kill .ic,
  .fe.kill .k {
    color: var(--kill);
  }
  .fe.death .ic,
  .fe.death .k {
    color: var(--death);
  }
  .fe.tk .ic,
  .fe.tk .k {
    color: var(--tk);
  }
  .who.dim {
    color: var(--muted);
    font-style: italic;
  }
  .who :global(.ci) {
    margin-right: 0.25em;
  }
  .who .sep {
    margin: 0 0.15em;
    opacity: 0.6;
  }
  .fe.empty {
    color: rgb(255 255 255 / 20%);
  }
  .hs {
    justify-self: end;
    font-size: 0.78em;
    font-weight: 800;
    color: var(--hs);
    border: 1px solid currentColor;
    border-radius: 4px;
    padding: 0 0.3em;
    white-space: nowrap;
  }
  .hs.taken {
    color: var(--hs-taken);
  }
  .who,
  .w {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .w {
    color: var(--muted);
  }
  .wr {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 2.6em 3em;
    gap: 0.4em;
    text-align: right;
    min-height: 1.3em;
  }
  .wr .who {
    text-align: left;
  }
  /* squad top 3: fixed columns and rows, the window never changes for it */
  .sq-head {
    display: flex;
    justify-content: space-between;
    font-size: 0.82em;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .sq {
    display: grid;
    grid-template-columns: 1em 2.6em minmax(0, 1fr) 3.4em;
    gap: 0.4em;
    align-items: baseline;
    min-height: 1.3em;
  }
  .sq .rank {
    color: var(--muted);
  }
  .sq b {
    text-align: right;
    font-weight: 750;
  }
  .sqf {
    color: #fff;
    border-radius: 4px;
    font-size: 0.72em;
    font-weight: 800;
    text-align: center;
    text-shadow: none;
  }
  .sqf.none {
    background: none;
  }
  .sq.own .who {
    color: var(--fc);
    font-weight: 750;
  }
  .sq b.dimmed {
    color: var(--muted);
    font-weight: 500;
  }
  .sq.empty .who {
    color: rgb(255 255 255 / 25%);
  }
  .style-slim .squad {
    border-top-color: rgb(255 255 255 / 7%);
  }
  .style-bare .squad {
    border-top-color: rgb(255 255 255 / 25%);
  }
  /* notification slot: fixed height, the window never grows for it */
  .slot {
    height: 3.4em;
    display: grid;
  }
  .toast {
    display: grid;
    align-content: center;
    gap: 0.1em;
    padding: 0.35em 0.8em;
    border-radius: 12px;
    background: rgb(var(--panel) / max(var(--bgA), 0.6));
    border: 1px solid var(--fc);
    border-left: 4px solid var(--fc);
    box-shadow: 0 4px 14px rgb(0 0 0 / 35%);
    animation: pop 0.22s ease-out;
    overflow: hidden;
  }
  .toast strong {
    font-size: 1.05em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .toast span {
    font-size: 0.8em;
    color: var(--muted);
  }
  .toast.packbg {
    background-color: rgb(var(--panel) / 0.35);
    background-size: contain;
    background-repeat: no-repeat;
    background-position: center;
    justify-items: center;
    text-align: center;
  }
  @keyframes pop {
    from {
      transform: translateY(6px);
      opacity: 0;
    }
  }
  /* slim: one flat panel, no frame, tighter, accent bar on the left */
  .style-slim .card {
    border: none;
    box-shadow: 0 2px 8px rgb(0 0 0 / 25%);
    border-radius: 8px;
    background: rgb(var(--panel) / var(--bgA));
    padding: 0.4em 0.6em 0.45em 0.8em;
    gap: 0.3em;
  }
  .style-slim .tile b {
    font-size: 1.3em;
  }
  .style-slim .feed,
  .style-slim .weapons,
  .style-slim .cmp {
    border-top-color: rgb(255 255 255 / 7%);
  }
  .style-slim .card::before {
    inset: 0 auto 0 0;
    width: 3px;
    height: auto;
  }
  /* bare: no panel at all, only text with contour */
  .style-bare .card {
    background: none;
    border: none;
    box-shadow: none;
  }
  .style-bare .card::before {
    display: none;
  }
  /* without a panel the text needs a real outline: 1 px in all directions plus a soft halo */
  .style-bare {
    --muted: #dde2ea;
    text-shadow:
      1px 0 0 #000,
      -1px 0 0 #000,
      0 1px 0 #000,
      0 -1px 0 #000,
      1px 1px 0 #000,
      -1px -1px 0 #000,
      1px -1px 0 #000,
      -1px 1px 0 #000,
      0 0 4px rgb(0 0 0 / 85%);
  }
  .style-bare .feed,
  .style-bare .weapons,
  .style-bare .cmp {
    border-top-color: rgb(255 255 255 / 25%);
  }
  .style-bare .hs {
    background: rgb(0 0 0 / 55%);
    text-shadow: none;
  }
  /* move mode: the real window keeps its size and type but shows nothing;
     the move layer draws the object */
  .ov.ghost {
    visibility: hidden;
  }
  /* screenshots only: game-like test backgrounds */
  .demo[data-bg="light"] {
    position: fixed;
    inset: 0;
    width: auto;
    padding: 24px;
    align-content: start;
    justify-content: start;
    background: linear-gradient(160deg, #e8eef2 0%, #c9d6c0 45%, #f4f1e6 100%);
  }
  .demo[data-bg="dark"] {
    position: fixed;
    inset: 0;
    width: auto;
    padding: 24px;
    align-content: start;
    justify-content: start;
    background: linear-gradient(160deg, #1d2a33 0%, #2e3b2a 50%, #0e1216 100%);
  }
</style>
