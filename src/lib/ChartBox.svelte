<script lang="ts">
  // One Chart.js chart in a card, with PNG export. Chart.js draws on a canvas
  // and needs no eval (CSP stays "script-src 'self'").
  import {
    BarController,
    BarElement,
    CategoryScale,
    Chart,
    Filler,
    Legend,
    LinearScale,
    LineController,
    LineElement,
    PointElement,
    Tooltip,
    type ChartConfiguration,
  } from "chart.js";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { api, errorText, inTauri } from "./api";
  import { t } from "./i18n";

  Chart.register(BarController, BarElement, CategoryScale, Filler, Legend, LinearScale, LineController, LineElement, PointElement, Tooltip);

  let {
    title,
    config,
    file,
    empty = false,
    notify,
  }: { title: string; config: ChartConfiguration; file: string; empty?: boolean; notify: (t: string, e?: boolean) => void } = $props();

  let canvas = $state<HTMLCanvasElement>();
  let chart: Chart | null = null;

  $effect(() => {
    const c = canvas;
    const cfg = config;
    if (!c || empty) return;
    chart?.destroy();
    chart = new Chart(c, { ...cfg, options: { ...cfg.options, responsive: true, maintainAspectRatio: false } });
    return () => {
      chart?.destroy();
      chart = null;
    };
  });

  /** PNG with an opaque background in the current theme colour. */
  function png(): string | null {
    if (!canvas) return null;
    const out = document.createElement("canvas");
    out.width = canvas.width;
    out.height = canvas.height;
    const ctx = out.getContext("2d");
    if (!ctx) return null;
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue("--panel").trim() || "#1a202a";
    ctx.fillRect(0, 0, out.width, out.height);
    ctx.drawImage(canvas, 0, 0);
    return out.toDataURL("image/png");
  }

  async function exportPng() {
    const data = png();
    if (!data) return;
    if (!inTauri()) {
      const a = document.createElement("a");
      a.href = data;
      a.download = `${file}.png`;
      a.click();
      return;
    }
    const path = await saveDialog({ defaultPath: `${file}.png`, filters: [{ name: "PNG", extensions: ["png"] }] });
    if (!path) return;
    try {
      await api.savePng(path, data);
      notify(t.analytics.pngSaved(path));
    } catch (e) {
      notify(errorText(e), true);
    }
  }
</script>

<section class="card chart">
  <div class="head">
    <h3>{title}</h3>
    <button class="mini" onclick={exportPng} disabled={empty} aria-label={t.analytics.pngAria(title)}>PNG</button>
  </div>
  {#if empty}
    <p class="muted small">{t.analytics.noData}</p>
  {:else}
    <div class="box" role="img" aria-label={title}><canvas bind:this={canvas}></canvas></div>
  {/if}
</section>

<style>
  .chart {
    display: grid;
    gap: var(--s1);
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s1);
  }
  .head h3 {
    margin: 0;
  }
  .box {
    position: relative;
    height: 260px;
  }
  button.mini {
    padding: 0.15rem 0.5rem;
    font-size: 0.8rem;
  }
</style>
