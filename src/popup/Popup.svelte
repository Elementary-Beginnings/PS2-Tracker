<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api, inTauri, on } from "../lib/api";
  import type { OverlaySettings, PackBackground, Toast } from "../lib/types";
  import { t } from "../lib/i18n";

  /** preview: drawn inside the move layer (sample text, never reports a size). */
  let { preview = false }: { preview?: boolean } = $props();

  // Achievement / voice pack pop-up: the pack image exactly as it is (own
  // alpha, no frame, no panel, no shadow), the text on it with a text shadow.
  // Without a pack image: text only (or the app style). Fixed size: only the
  // settings and the image change it, never a notification.
  const params = new URLSearchParams(location.search);
  const demo = !inTauri() && params.has("demo");
  let o = $state<Partial<OverlaySettings>>({ popup_scale: 1, popup_duration_ms: 4000, popup_text: true, popup_style: "plain", popup_opacity: 1, pack_background: true });
  let bg = $state<PackBackground | null>(null);
  /** Move mode: the real window shows nothing (the move layer draws it). */
  let edit = $state(false);
  let toast = $state<Toast | null>(null);
  let shown = $state(false);
  let factor = $state(1);
  let root = $state<HTMLDivElement>();
  let ready = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

  if (demo) {
    toast = { title: "Double kill", text: "2 kills in 4 s", kind: "voice" };
    shown = true;
    if (params.has("img")) {
      // synthetic PNG with alpha for screenshots (drawn here, nothing from a pack)
      const c = document.createElement("canvas");
      c.width = 454;
      c.height = 90;
      const g = c.getContext("2d")!;
      const grad = g.createLinearGradient(0, 0, 454, 0);
      grad.addColorStop(0, "rgba(220,40,40,0)");
      grad.addColorStop(0.5, "rgba(220,40,40,0.85)");
      grad.addColorStop(1, "rgba(220,40,40,0)");
      g.fillStyle = grad;
      g.fillRect(0, 10, 454, 70);
      g.strokeStyle = "rgba(255,255,255,0.9)";
      g.lineWidth = 3;
      g.beginPath();
      g.moveTo(40, 12);
      g.lineTo(414, 12);
      g.moveTo(40, 78);
      g.lineTo(414, 78);
      g.stroke();
      bg = { pack: "demo", data_url: c.toDataURL("image/png"), width: 454, height: 90 };
    }
  }

  function show(x: Toast) {
    toast = x;
    shown = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (shown = false), Math.max(800, o.popup_duration_ms ?? 4000));
  }

  async function load() {
    o = (await api.getConfig()).overlay;
    bg = await api.toastBackground();
    const s = await api.overlayState();
    edit = preview ? false : s.edit;
    factor = s.size_factor;
  }

  onMount(() => {
    if (!inTauri()) return;
    const un = [
      on<Toast>("toast", (x) => {
        if (!edit && !preview) show(x);
      }),
      on<boolean>("overlay-mode", (e) => (edit = preview ? false : e)),
      on("config-changed", load),
    ];
    load().then(() => (ready = true));
    return () => un.forEach((p) => p.then((f) => f()));
  });

  const useImage = $derived(!!(o.pack_background && bg));
  const scale = $derived(Math.max(1, Math.round(o.popup_scale ?? 1)));
  const dpr = typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;

  // the content size depends on the image and the settings only
  $effect(() => {
    void [useImage, scale, bg, o.popup_style, factor];
    if (!inTauri() || preview || !root || !ready) return;
    tick().then(() => {
      if (!root) return;
      const r = root.getBoundingClientRect();
      api.overlayContentSize("popup", Math.ceil(r.width * dpr), Math.ceil(r.height * dpr)).catch(() => {});
    });
  });

  const sample: Toast = { title: t.overlay.popupSample, text: t.overlay.popupSampleText, kind: "voice" };
  const content = $derived(preview ? sample : edit ? null : toast);
  const visible = $derived(preview || (!edit && shown && !!toast));
</script>

<div
  bind:this={root}
  class="pop"
  class:demo={demo && !preview}
  data-bg={demo && !preview ? (params.get("bg") ?? "dark") : null}
  style:font-size={`${16 * factor}px`}
  role="presentation"
>
  {#if useImage && bg}
    <div
      class="img"
      class:pixel={scale > 1}
      class:on={visible}
      style:width={`${(bg.width * scale) / dpr}px`}
      style:height={`${(bg.height * scale) / dpr}px`}
      style:background-image={`url(${bg.data_url})`}
      style:--op={o.popup_opacity ?? 1}
    >
      {#if o.popup_text && content}
        <div class="txt"><strong>{content.title}</strong>{#if content.text}<span>{content.text}</span>{/if}</div>
      {/if}
    </div>
  {:else}
    <div class="box style-{o.popup_style ?? 'plain'}" class:on={visible} style:--op={o.popup_opacity ?? 1}>
      {#if content}<div class="txt"><strong>{content.title}</strong>{#if content.text}<span>{content.text}</span>{/if}</div>{/if}
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
  .pop {
    position: absolute;
    top: 0;
    left: 0;
    display: inline-block;
    color: #fff;
    font-family: "Inter", "Noto Sans", "Segoe UI", system-ui, sans-serif;
  }
  .img,
  .box {
    position: relative;
    opacity: 0;
    transition: opacity 0.25s ease;
  }
  .img.on,
  .box.on {
    opacity: var(--op);
  }
  .img {
    background-size: 100% 100%;
    background-repeat: no-repeat;
  }
  .img.pixel {
    image-rendering: pixelated;
  }
  .box {
    width: 22em;
    height: 3.6em;
  }
  .box.style-card {
    background: rgb(15 18 24 / 85%);
    border: 1px solid rgb(255 255 255 / 15%);
    border-radius: 12px;
  }
  .txt {
    position: absolute;
    inset: 0;
    display: grid;
    align-content: center;
    justify-items: center;
    text-align: center;
    gap: 0.1em;
    padding: 0 0.6em;
    text-shadow:
      0 0 3px #000,
      0 0 2px #000,
      0 1px 3px #000;
  }
  .txt strong {
    font-size: 1.15em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .txt span {
    font-size: 0.85em;
  }
  .demo[data-bg="light"],
  .demo[data-bg="dark"] {
    position: fixed;
    inset: 0;
    padding: 24px;
  }
  .demo[data-bg="light"] {
    background: linear-gradient(160deg, #e8eef2 0%, #c9d6c0 45%, #f4f1e6 100%);
  }
  .demo[data-bg="dark"] {
    background: linear-gradient(160deg, #1d2a33 0%, #2e3b2a 50%, #0e1216 100%);
  }
</style>
