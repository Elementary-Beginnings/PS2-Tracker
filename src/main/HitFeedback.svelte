<script lang="ts">
  // Crosshair page, section "Hit feedback": per ESS event a tint flash or a
  // swapped crosshair PNG, with a static peak frame per event and live preview.
  import { t } from "../lib/i18n";
  import { api, errorText, inTauri } from "../lib/api";
  import { draw, loadImage } from "../lib/crosshair";
  import type { Config, CrosshairItem, FxKind } from "../lib/types";

  let {
    config,
    items,
    save,
    notify,
  }: { config: Config; items: CrosshairItem[]; save: (c?: Config) => Promise<void>; notify: (t: string, e?: boolean) => void } = $props();

  const KINDS: FxKind[] = ["kill", "headshot", "assist", "vehicle_kill", "vehicle_damage"];
  const h = $derived(config.crosshair.hit_feedback);
  const c = $derived(config.crosshair);
  let canvases = $state<Record<string, HTMLCanvasElement | undefined>>({});

  // static peak frame per event (what the crosshair looks like at the flash)
  $effect(() => {
    for (const k of KINDS) {
      const e = h[k];
      const cv = canvases[k];
      const base = items.find((i) => i.file === c.file);
      const src = e.mode === "swap" ? (items.find((i) => i.file === e.file) ?? base) : base;
      if (!cv || !src) continue;
      const opts = {
        scale: c.scale,
        opacity: e.peak,
        tint: e.mode === "tint" ? e.color : c.tint,
        tintMode: e.mode === "tint" ? ("recolor" as const) : ("multiply" as const),
        zoom: 3,
      };
      loadImage(src.data_url).then((img) => draw(cv, img, opts, 1));
    }
  });

  async function preview(k: FxKind) {
    if (!c.enabled) {
      notify(t.crosshair.fx.needCrosshair, true);
      return;
    }
    try {
      await api.crosshairFxPreview(k, $state.snapshot(h[k]));
    } catch (e) {
      notify(errorText(e), true);
    }
  }
</script>

<section class="card grid fx">
  <h3>{t.crosshair.fx.title}</h3>
  <p class="hint small">{t.crosshair.fx.limits}</p>
  <label class="check"><input type="checkbox" bind:checked={h.enabled} onchange={() => save()} /> {t.crosshair.fx.enabled}</label>
  <div class="list" class:off={!h.enabled}>
    {#each KINDS as k (k)}
      {@const e = h[k]}
      <div class="item">
        <div class="pv"><canvas bind:this={canvases[k]} aria-label={t.crosshair.fx.frameAria(t.crosshair.fx.kinds[k] ?? k)}></canvas></div>
        <div class="grid">
          <label class="check"><input type="checkbox" bind:checked={e.enabled} onchange={() => save()} /> <strong>{t.crosshair.fx.kinds[k]}</strong></label>
          <span class="muted small">{t.crosshair.fx.sources[k]}</span>
          <div class="row">
            <select bind:value={e.mode} onchange={() => save()} aria-label={t.crosshair.fx.mode}>
              <option value="tint">{t.crosshair.fx.tint}</option>
              <option value="swap">{t.crosshair.fx.swap}</option>
            </select>
            {#if e.mode === "tint"}
              <input type="color" bind:value={e.color} onchange={() => save()} aria-label={t.crosshair.fx.color} />
            {:else}
              <select bind:value={e.file} onchange={() => save()} aria-label={t.crosshair.fx.file}>
                <option value="" disabled>{t.crosshair.fx.pickFile}</option>
                {#each items as it (it.file)}<option value={it.file}>{it.file.replace(/\.png$/, "")}</option>{/each}
              </select>
            {/if}
            <button class="mini" onclick={() => preview(k)} disabled={!inTauri()} aria-label={t.crosshair.fx.previewAria(t.crosshair.fx.kinds[k] ?? k)}>▶ {t.crosshair.fx.preview}</button>
          </div>
          <div class="row">
            <label class="field">{t.crosshair.fx.duration(e.duration_ms)}
              <input type="range" min="40" max="1000" step="10" bind:value={e.duration_ms} onchange={() => save()} />
            </label>
            <label class="field">{t.crosshair.fx.peak(Math.round(e.peak * 100))}
              <input type="range" min="0.1" max="1" step="0.05" bind:value={e.peak} onchange={() => save()} />
            </label>
            <label class="check"><input type="checkbox" bind:checked={e.fade} onchange={() => save()} /> {t.crosshair.fx.fade}</label>
          </div>
        </div>
      </div>
    {/each}
  </div>
  <p class="muted small">{t.crosshair.fx.swapHint}</p>
</section>

<style>
  .fx {
    grid-column: 1 / -1;
  }
  .list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
    gap: var(--s2);
  }
  .list.off {
    opacity: 0.55;
  }
  .item {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr);
    gap: var(--s2);
    padding: var(--s1);
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .pv {
    display: grid;
    place-items: center;
    min-height: 96px;
    border-radius: 6px;
    background: linear-gradient(135deg, #6b7a55, #33424f 50%, #b8c2c9);
    overflow: hidden;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s1) var(--s2);
    align-items: center;
  }
  .field input[type="range"] {
    width: 9rem;
  }
  button.mini {
    padding: 0.15rem 0.45rem;
    font-size: 0.82rem;
  }
</style>
