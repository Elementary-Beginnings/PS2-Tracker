<script lang="ts">
  import { t } from "../lib/i18n";
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, errorText, inTauri } from "../lib/api";
  import type { Config, CrosshairItem, MonitorInfo } from "../lib/types";
  import { draw, loadImage } from "../lib/crosshair";
  import HotkeyInput from "../lib/HotkeyInput.svelte";
  import GameWindowPicker from "../lib/GameWindowPicker.svelte";
  import HitFeedback from "./HitFeedback.svelte";
  import { demoCrosshairs } from "../lib/demo";

  let {
    config,
    save,
    notify,
  }: { config: Config; save: (c?: Config) => Promise<void>; notify: (t: string, e?: boolean) => void } = $props();

  let items = $state<CrosshairItem[]>([]);
  let monitors = $state<MonitorInfo[]>([]);
  let preview: HTMLCanvasElement | undefined = $state();
  const c = $derived(config.crosshair);
  const current = $derived(items.find((i) => i.file === c.file));

  const others = (mine: string) =>
    [config.overlay.hotkey_toggle, config.overlay.hotkey_edit, config.overlay.hotkey_finish_move, c.hotkey_toggle, c.hotkey_cycle].filter((h, i, a) => a.indexOf(mine) !== i);

  async function load() {
    if (!inTauri()) {
      items = demoCrosshairs();
      return;
    }
    items = await api.crosshairList();
    monitors = await api.monitors();
  }
  onMount(() => {
    load().catch((e) => notify(errorText(e), true));
  });

  $effect(() => {
    const it = current;
    const opts = { scale: c.scale, opacity: c.opacity, tint: c.tint, zoom: 4 };
    if (!it || !preview) return;
    const canvas = preview;
    loadImage(it.data_url).then((img) => draw(canvas, img, opts, 1));
  });

  async function importPng() {
    const path = await open({ multiple: false, filters: [{ name: "PNG", extensions: ["png"] }] });
    if (typeof path !== "string") return;
    try {
      const name = await api.crosshairImport(path);
      await load();
      c.file = name;
      await save();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function pick(file: string) {
    c.file = file;
    await save();
  }

</script>

<h1>{t.crosshair.title}</h1>
<p class="muted">{t.crosshair.intro}</p>

<div class="grid cols">
  <section class="card grid">
    <h3>{t.crosshair.library}</h3>
    <div class="lib">
      {#each items as it (it.file)}
        <button class="xh" class:sel={it.file === c.file} onclick={() => pick(it.file)} title={it.file}>
          <img src={it.data_url} alt={it.file} style:width={`${it.width * 3 <= 120 ? it.width * 3 : 120}px`} />
          <span class="small muted">{it.file.replace(/\.png$/, "")}<br />{t.crosshair.sizeLabel(it.width, it.height)}</span>
        </button>
      {/each}
    </div>
    <div class="row">
      <button onclick={importPng} disabled={!inTauri()}>{t.crosshair.importPng}</button>
      <span class="muted small">{t.crosshair.importHint}</span>
    </div>
    <h3>{t.crosshair.preview}</h3>
    <div class="pv"><canvas bind:this={preview}></canvas></div>
  </section>

  <section class="card grid">
    <h3>{t.crosshair.settings}</h3>
    <div class="row">
      <label class="check"><input type="checkbox" bind:checked={c.enabled} onchange={() => save()} /> {t.crosshair.show}</label>
      <button onclick={() => api.crosshairToggle()} disabled={!inTauri()}>{t.crosshair.toggle}</button>
    </div>
    <label class="field">{t.crosshair.scale(c.scale)}
      <input type="range" min="1" max="8" step="1" bind:value={c.scale} onchange={() => save()} />
    </label>
    <label class="field">{t.crosshair.opacity(Math.round(c.opacity * 100))}
      <input type="range" min="0.1" max="1" step="0.05" bind:value={c.opacity} onchange={() => save()} />
    </label>
    <div class="row">
      <label class="check"><input type="checkbox" checked={c.tint !== ""} onchange={(e) => { c.tint = e.currentTarget.checked ? "#50ff78" : ""; save(); }} /> {t.crosshair.tint}</label>
      {#if c.tint}<input type="color" bind:value={c.tint} onchange={() => save()} />{/if}
    </div>
    <div class="row">
      <label class="field">{t.crosshair.offsetX} <input type="number" min="-50" max="50" bind:value={c.offset_x} onchange={() => save()} /></label>
      <label class="field">{t.crosshair.offsetY} <input type="number" min="-50" max="50" bind:value={c.offset_y} onchange={() => save()} /></label>
    </div>
    <label class="field">{t.crosshair.monitor}
      <select bind:value={c.monitor} onchange={() => save()}>
        <option value="">{t.crosshair.primary}</option>
        {#each monitors as m (m.name)}
          <option value={m.name}>{t.crosshair.monitorOption(m.name, m.width, m.height, m.scale_factor)}</option>
        {/each}
      </select>
    </label>
    <div class="row">
      <HotkeyInput label={t.crosshair.hkToggle} bind:value={c.hotkey_toggle} others={others(c.hotkey_toggle)} onchange={() => save()} />
      <HotkeyInput label={t.crosshair.hkNext} bind:value={c.hotkey_cycle} others={others(c.hotkey_cycle)} onchange={() => save()} />
    </div>
    <label class="check"><input type="checkbox" bind:checked={c.auto_hide} onchange={() => save()} /> {t.crosshair.autoHide}</label>
    <GameWindowPicker {config} {save} {notify} />
    <details class="section small">
      <summary>{t.crosshair.aboveFullscreen} · {t.common.learnMore}</summary>
      <p class="muted">{t.crosshair.autoHideHint}</p>
      <p class="muted">{t.crosshair.backendHint}</p>
    </details>
  </section>
  <HitFeedback {config} {items} {save} {notify} />
</div>

<style>
  .cols {
    grid-template-columns: 1fr 1fr;
    align-items: start;
  }
  .lib {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    gap: 0.5rem;
  }
  .xh {
    display: grid;
    justify-items: center;
    gap: 0.35rem;
    padding: 0.6rem 0.3rem;
    background: repeating-conic-gradient(#2a3140 0% 25%, #222833 0% 50%) 50% / 12px 12px;
  }
  .xh.sel {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .xh img {
    image-rendering: pixelated;
  }
  .pv {
    display: grid;
    place-items: center;
    min-height: 180px;
    border-radius: 8px;
    background: linear-gradient(135deg, #6b7a55, #33424f 50%, #b8c2c9);
  }
  input[type="number"] {
    width: 6rem;
  }
</style>
