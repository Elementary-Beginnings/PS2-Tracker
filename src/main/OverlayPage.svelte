<script lang="ts">
  import { t } from "../lib/i18n";
  import { onMount } from "svelte";
  import { api, errorText, inTauri, on } from "../lib/api";
  import type { Config, HotkeyFailure, MonitorInfo } from "../lib/types";
  import HotkeyInput from "../lib/HotkeyInput.svelte";
  import GameWindowPicker from "../lib/GameWindowPicker.svelte";

  let {
    config,
    save,
    notify,
  }: { config: Config; save: (c?: Config) => Promise<void>; notify: (t: string, e?: boolean) => void } = $props();

  let monitors = $state<MonitorInfo[]>([]);
  let edit = $state(false);
  let failures = $state<HotkeyFailure[]>([]);

  onMount(() => {
    if (!inTauri()) return;
    api.monitors().then((m) => (monitors = m));
    api.overlayState().then((s) => (edit = s.edit));
    const loadFailures = () => api.hotkeyStatus().then((f) => (failures = f)).catch(() => {});
    loadFailures();
    const un = [on<boolean>("overlay-mode", (e) => (edit = e)), on("config-changed", loadFailures)];
    return () => un.forEach((p) => p.then((f) => f()));
  });

  async function toggleEdit() {
    try {
      await api.overlaySetEdit(!edit);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  const o = $derived(config.overlay);

  const ANCHORS = ["top-left", "top-center", "top-right", "center", "bottom-left", "bottom-center", "bottom-right"];
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function saveSoon() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => save(), 150);
  }
  type Obj = "stats" | "popup";
  const DEFAULTS: Record<Obj, { anchor: string; x: number; y: number }> = {
    stats: { anchor: "top-right", x: 24, y: 24 },
    popup: { anchor: "top-center", x: 0, y: 160 },
  };
  const getPos = (k: Obj) =>
    k === "stats" ? { anchor: o.anchor, x: o.x, y: o.y } : { anchor: o.popup_anchor, x: o.popup_x, y: o.popup_y };
  function setPos(k: Obj, p: { anchor: string; x: number; y: number }) {
    if (k === "stats") Object.assign(o, { anchor: p.anchor, x: p.x, y: p.y });
    else Object.assign(o, { popup_anchor: p.anchor, popup_x: p.x, popup_y: p.y });
  }
  async function setAnchor(k: Obj, a: string) {
    setPos(k, { ...getPos(k), anchor: a });
    await save();
  }
  async function resetPos(k: Obj) {
    setPos(k, DEFAULTS[k]);
    await save();
  }
  /** Arrow keys move the object on screen; the offset counts from the anchored edge. */
  function nudge(k: Obj, e: KeyboardEvent) {
    const step = e.shiftKey ? 10 : 1;
    const d: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const m = d[e.key];
    if (!m) return;
    e.preventDefault();
    const p = getPos(k);
    const right = p.anchor.endsWith("right");
    const bottom = p.anchor.startsWith("bottom");
    setPos(k, { anchor: p.anchor, x: p.x + (right ? -m[0] : m[0]), y: p.y + (bottom ? -m[1] : m[1]) });
    saveSoon();
  }
  const widgets: { key: "show_stats" | "show_comparison" | "show_killfeed" | "show_weapons" | "show_toasts" | "pack_background" | "compact" | "show_kd_revived" | "show_squad" | "show_my_class"; label: string }[] = [
    { key: "show_stats", label: t.overlay.wStats },
    { key: "show_comparison", label: t.overlay.wComparison },
    { key: "show_killfeed", label: t.overlay.wKillfeed },
    { key: "show_my_class", label: t.classes.showMine },
    { key: "show_weapons", label: t.overlay.wWeapons },
    { key: "show_toasts", label: t.overlay.wToasts },
    { key: "pack_background", label: t.overlay.wPackBackground },
    { key: "compact", label: t.overlay.wCompact },
    { key: "show_kd_revived", label: t.overlay.wKdRevived },
    { key: "show_squad", label: t.overlay.wSquad },
  ];

  type Preset = "minimal" | "streamer" | "full";
  const presets: Record<Preset, Partial<Record<"show_stats" | "show_comparison" | "show_killfeed" | "show_weapons" | "show_toasts" | "compact", boolean>>> = {
    minimal: { show_stats: true, show_comparison: false, show_killfeed: false, show_weapons: false, show_toasts: true, compact: true },
    streamer: { show_stats: true, show_comparison: false, show_killfeed: true, show_weapons: false, show_toasts: true, compact: false },
    full: { show_stats: true, show_comparison: true, show_killfeed: true, show_weapons: true, show_toasts: true, compact: false },
  };
  async function applyPreset(p: Preset) {
    Object.assign(config.overlay, presets[p]);
    await save();
  }
  const allHotkeys = $derived([
    config.overlay.hotkey_toggle,
    config.overlay.hotkey_edit,
    config.overlay.hotkey_finish_move,
    config.crosshair.hotkey_toggle,
    config.crosshair.hotkey_cycle,
  ]);
  const othersOf = (v: string) => {
    const i = allHotkeys.indexOf(v);
    return allHotkeys.filter((_, j) => j !== i);
  };
</script>

<h1>{t.overlay.title}</h1>
<p class="muted">{t.overlay.intro}</p>

<div class="row presets" role="group" aria-label={t.overlay.presets}>
  <span class="muted small">{t.overlay.presets}</span>
  <button onclick={() => applyPreset("minimal")}>{t.overlay.pMinimal}</button>
  <button onclick={() => applyPreset("streamer")}>{t.overlay.pStreamer}</button>
  <button onclick={() => applyPreset("full")}>{t.overlay.pFull}</button>
</div>

<div class="grid cols">
  <section class="card grid">
    <h3>{t.overlay.preview}</h3>
    <div class="mini" style:--bgA={o.background_opacity} aria-hidden="true">
      <div class="mp head"><b>DemoSoldierTR</b><span>12:34</span></div>
      {#if o.show_stats}<div class="mp stats"><span>K/D <b>2.00</b></span><span>KPM <b>0.93</b></span><span>HSR <b>26 %</b></span></div>{/if}
      {#if o.show_comparison}<div class="mp small">K/D ▲ +82 % · KPM ▲ +107 %</div>{/if}
      {#if o.show_killfeed}<div class="mp small">✕ Enemy A · ⌖ HS<br />☠ Enemy B{#if !o.compact}<br />⛟ Sunderer{/if}</div>{/if}
      {#if o.show_weapons && !o.compact}<div class="mp small">Weapon 1 · 30 · 30 %</div>{/if}
      {#if o.show_squad}<div class="mp small">1 DemoMedic 3.44<br />2 DemoSoldierTR 2.00<br />3 –</div>{/if}
    </div>
    <h3>{t.overlay.display}</h3>
    <label class="check"><input type="checkbox" bind:checked={o.enabled} onchange={() => save()} /> {t.overlay.show}</label>
    <label class="check"><input type="checkbox" bind:checked={o.click_through} onchange={() => save()} /> {t.overlay.clickThrough}</label>
    <label class="check"><input type="checkbox" bind:checked={o.auto_hide} onchange={() => save()} /> {t.overlay.autoHide}</label>
    {#if o.auto_hide}<GameWindowPicker {config} {save} {notify} />{/if}
    <h3>{t.overlay.position}</h3>
    <div class="row">
      <button class:primary={!edit} onclick={toggleEdit}>{edit ? t.overlay.finishMove : t.overlay.moveMode(o.hotkey_edit)}</button>
    </div>
    <p class="muted small">{t.overlay.moveHint}</p>
    {#each [{ k: "stats" as Obj, l: t.overlay.statsObject }, { k: "popup" as Obj, l: t.overlay.popupObject }] as ob (ob.k)}
      {@const p = getPos(ob.k)}
      <fieldset class="obj">
        <legend>{ob.l}</legend>
        <div class="row">
          <div class="anchors" role="radiogroup" aria-label={`${ob.l}: ${t.overlay.anchor}`}>
            {#each ANCHORS as a (a)}
              <button role="radio" aria-checked={p.anchor === a} class:on={p.anchor === a} class="a-{a}" onclick={() => setAnchor(ob.k, a)} title={t.overlay.anchors[a]}>
                <span class="sr-only">{t.overlay.anchors[a]}</span>
              </button>
            {/each}
          </div>
          <div class="grid">
            <label class="field">{t.overlay.offsetX} <input class="n" type="number" step="1" value={p.x} onchange={(e) => { setPos(ob.k, { ...p, x: Number(e.currentTarget.value) }); save(); }} /></label>
            <label class="field">{t.overlay.offsetY} <input class="n" type="number" step="1" value={p.y} onchange={(e) => { setPos(ob.k, { ...p, y: Number(e.currentTarget.value) }); save(); }} /></label>
          </div>
        </div>
        <div class="row">
          <button type="button" class="nudge" aria-label={`${ob.l}: ${t.overlay.nudgeAria}`} onkeydown={(e) => nudge(ob.k, e)}>{t.overlay.nudge}</button>
          <button type="button" onclick={() => resetPos(ob.k)}>{t.overlay.resetPosition}</button>
        </div>
      </fieldset>
    {/each}
    <fieldset class="obj">
      <legend>{t.overlay.popupObject}</legend>
      <label class="check"><input type="checkbox" bind:checked={o.popup_text} onchange={() => save()} /> {t.overlay.popupText}</label>
      <div class="row">
        <label class="field">{t.overlay.popupScale}
          <select bind:value={o.popup_scale} onchange={() => save()}>
            {#each [1, 2, 3] as n (n)}<option value={n}>{n}×</option>{/each}
          </select>
        </label>
        <label class="field">{t.overlay.popupStyle}
          <select bind:value={o.popup_style} onchange={() => save()}>
            {#each Object.entries(t.overlay.popupStyles) as [k, l] (k)}<option value={k}>{l}</option>{/each}
          </select>
        </label>
      </div>
      <label class="field">{t.overlay.popupDuration(Math.round(o.popup_duration_ms / 100) / 10)}
        <input type="range" min="1000" max="10000" step="500" bind:value={o.popup_duration_ms} onchange={() => save()} />
      </label>
      <label class="field">{t.overlay.popupOpacity(Math.round(o.popup_opacity * 100))}
        <input type="range" min="0.2" max="1" step="0.05" bind:value={o.popup_opacity} onchange={() => save()} />
      </label>
    </fieldset>
    <label class="field">{t.overlay.monitor}
      <select bind:value={o.monitor} onchange={() => save()}>
        <option value="">{t.overlay.primary}</option>
        {#each monitors as m (m.name)}
          <option value={m.name}>{t.overlay.monitorOption(m.name, m.width, m.height, m.primary, m.scale_factor)}</option>
        {/each}
      </select>
    </label>
    <label class="field">{t.overlay.bgOpacity(Math.round(o.background_opacity * 100))}
      <input type="range" min="0" max="1" step="0.05" bind:value={o.background_opacity} onchange={() => save()} />
    </label>
    <div class="row">
      <label class="field">{t.overlay.size}
        <select bind:value={o.size} onchange={() => save()}>
          <option value="auto">{t.overlay.sizeAuto}</option>
          {#each ["S", "M", "L", "XL"] as s (s)}<option value={s}>{s}</option>{/each}
        </select>
      </label>
      <label class="field">{t.overlay.style}
        <select bind:value={o.style} onchange={() => save()}>
          {#each Object.entries(t.overlay.styles) as [k, l] (k)}<option value={k}>{l}</option>{/each}
        </select>
      </label>
    </div>
    <label class="field">{t.overlay.fontSize(Math.round(o.scale * 100))}
      <input type="range" min="0.7" max="1.6" step="0.05" bind:value={o.scale} onchange={() => save()} />
    </label>
  </section>

  <section class="card grid">
    <h3>{t.overlay.widgets}</h3>
    {#each widgets as w (w.key)}
      <label class="check"><input type="checkbox" bind:checked={o[w.key]} onchange={() => save()} /> {w.label}</label>
    {/each}
    {#if o.show_squad}
      <fieldset class="obj">
        <legend>{t.overlay.squadTitle}</legend>
        <div class="row">
          <label class="field">{t.overlay.squadMetric}
            <select bind:value={o.squad_metric} onchange={() => save()}>
              {#each Object.entries(t.overlay.squadMetrics) as [k, l] (k)}<option value={k}>{l}</option>{/each}
            </select>
          </label>
          <label class="field">{t.overlay.squadMinKills}
            <input class="n" type="number" min="0" max="100" step="1" bind:value={o.squad_min_kills} onchange={() => save()} />
          </label>
        </div>
        <p class="muted small">{t.overlay.squadHint}</p>
      </fieldset>
    {/if}
    <h3>{t.overlay.hotkeys}</h3>
    <HotkeyInput label={t.overlay.hkToggle} bind:value={o.hotkey_toggle} others={othersOf(o.hotkey_toggle)} onchange={() => save()} />
    <HotkeyInput label={t.overlay.hkEdit} bind:value={o.hotkey_edit} others={othersOf(o.hotkey_edit)} onchange={() => save()} />
    <HotkeyInput label={t.overlay.hkFinish} bind:value={o.hotkey_finish_move} others={othersOf(o.hotkey_finish_move)} onchange={() => save()} />
    <label class="field">{t.overlay.idleTimeout}
      <select value={String(o.move_idle_timeout_secs)} onchange={(e) => { o.move_idle_timeout_secs = Number(e.currentTarget.value); save(); }}>
        {#each Object.entries(t.overlay.idleOptions) as [k, l] (k)}<option value={k}>{l}</option>{/each}
      </select>
    </label>
    {#each failures as f (f.hotkey)}<p class="hint small warn" role="alert">{t.overlay.hotkeyFailed(f.hotkey, f.action)}</p>{/each}
    <details class="section small">
      <summary>{t.overlay.waylandShort} · {t.common.learnMore}</summary>
      <p class="muted">{t.overlay.waylandHint}</p>
    </details>
  </section>
</div>

<style>
  .obj {
    display: grid;
    gap: var(--s1);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: var(--s1) var(--s2) var(--s2);
  }
  .obj legend {
    padding: 0 4px;
    color: var(--muted);
    font-size: 0.9em;
  }
  .anchors {
    display: grid;
    grid-template-columns: repeat(3, 44px);
    grid-template-rows: repeat(3, 30px);
    gap: 4px;
    padding: 6px;
    width: max-content;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--bg-2);
  }
  .anchors button {
    padding: 0;
    border-radius: 5px;
  }
  .anchors button.on {
    background: var(--accent);
    border-color: var(--accent);
  }
  .a-top-left { grid-area: 1 / 1; }
  .a-top-center { grid-area: 1 / 2; }
  .a-top-right { grid-area: 1 / 3; }
  .a-center { grid-area: 2 / 2; }
  .a-bottom-left { grid-area: 3 / 1; }
  .a-bottom-center { grid-area: 3 / 2; }
  .a-bottom-right { grid-area: 3 / 3; }
  input.n {
    width: 6rem;
  }
  .nudge {
    padding: var(--s1) var(--s2);
    border: 1px dashed var(--line);
    border-radius: 8px;
    color: var(--muted);
    font-size: 0.9em;
    cursor: pointer;
  }
  .nudge:focus-visible {
    border-color: var(--accent);
    color: var(--text);
  }
  .cols {
    grid-template-columns: 1fr 1fr;
    align-items: start;
  }
  .presets {
    margin-bottom: var(--s2);
  }
  .mini {
    display: grid;
    gap: 4px;
    padding: 12px;
    border-radius: 8px;
    min-height: 140px;
    align-content: start;
    background: linear-gradient(135deg, #6b7a55, #33424f 50%, #b8c2c9);
    color: #f2f3f5;
    text-shadow:
      0 0 2px #000,
      0 1px 2px #000;
  }
  .mp {
    background: rgb(10 12 16 / var(--bgA));
    border-left: 3px solid var(--accent);
    border-radius: 6px;
    padding: 4px 8px;
  }
  .mp.head,
  .mp.stats {
    display: flex;
    gap: 12px;
    justify-content: space-between;
  }
</style>
