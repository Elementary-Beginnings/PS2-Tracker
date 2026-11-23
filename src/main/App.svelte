<script lang="ts">
  import { t } from "../lib/i18n";
  import { onMount } from "svelte";
  import { api, errorText, inTauri, on } from "../lib/api";
  import type { Config, EssStatus, ServiceIdSource, Snapshot } from "../lib/types";
  import { appInfo, infoLine, loadAppInfo } from "../lib/appInfo";
  import { demoConfig, demoSnapshot, demoStatus } from "../lib/demo";
  import { applyAccent } from "../lib/theme";
  import { faction, locationLabel } from "../lib/format";
  import Live from "./Live.svelte";
  import Sessions from "./Sessions.svelte";
  import Characters from "./Characters.svelte";
  import OverlayPage from "./OverlayPage.svelte";
  import CrosshairPage from "./CrosshairPage.svelte";
  import VoicePage from "./VoicePage.svelte";
  import SettingsPage from "./SettingsPage.svelte";
  import Squad from "./Squad.svelte";
  import Wizard from "./Wizard.svelte";

  type Page = "live" | "sessions" | "analytics" | "squad" | "characters" | "overlay" | "crosshair" | "voice" | "settings";
  const groups: { title: string | null; items: { id: Page; label: string; icon: string }[] }[] = [
    {
      title: t.nav.groupSession,
      items: [
        { id: "live", label: t.nav.live, icon: "●" },
        { id: "sessions", label: t.nav.sessions, icon: "☰" },
        { id: "analytics", label: t.nav.analytics, icon: "▥" },
        { id: "squad", label: t.nav.squad, icon: "♜" },
      ],
    },
    {
      title: t.nav.groupCustomize,
      items: [
        { id: "overlay", label: t.nav.overlay, icon: "▣" },
        { id: "crosshair", label: t.nav.crosshair, icon: "✛" },
        { id: "voice", label: t.nav.voice, icon: "♪" },
      ],
    },
    {
      title: null,
      items: [
        { id: "characters", label: t.nav.characters, icon: "♟" },
        { id: "settings", label: t.nav.settings, icon: "⚙" },
      ],
    },
  ];

  const params = new URLSearchParams(location.search);
  let page = $state<Page>((params.get("page") as Page | null) ?? "live");
  let config = $state<Config | null>(null);
  let status = $state<EssStatus | null>(null);
  let snapshot = $state<Snapshot | null>(null);
  let message = $state<{ text: string; error: boolean } | null>(null);
  /** Move mode is on: banner with a way out (never stuck in it). */
  let moving = $state(false);
  async function finishMove() {
    try {
      await api.overlaySetEdit(false);
    } catch (e) {
      notify(errorText(e), true);
    }
  }
  let wizard = $state(false);
  let navOpen = $state(false);
  let msgTimer: ReturnType<typeof setTimeout> | undefined;

  function notify(text: string, error = false) {
    message = { text, error };
    clearTimeout(msgTimer);
    msgTimer = setTimeout(() => (message = null), error ? 8000 : 3500);
  }

  function go(p: Page) {
    page = p;
    navOpen = false;
  }

  async function save(next?: Config) {
    const c = next ?? config;
    if (!c) return;
    if (!inTauri()) {
      config = c;
      return;
    }
    try {
      await api.saveConfig($state.snapshot(c) as Config);
      config = await api.getConfig();
      notify(t.app.saved);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  // the active character: running session, else the first enabled character
  const activeChar = $derived(config?.character.find((c) => c.enabled && c.id) ?? null);
  const activeFaction = $derived(snapshot?.faction_id ?? activeChar?.faction_id ?? null);

  $effect(() => {
    const theme = config?.ui.theme ?? "dark";
    document.documentElement.dataset.theme = theme;
    applyAccent(config?.ui.accent ?? "faction", config?.ui.accent_custom ?? "", activeFaction, theme);
  });

  async function copyVersion() {
    if (!$appInfo) return;
    try {
      await navigator.clipboard.writeText(infoLine($appInfo));
      notify(t.about.copied);
    } catch {
      notify(infoLine($appInfo));
    }
  }

  onMount(() => {
    loadAppInfo();
    if (!inTauri()) {
      // browser preview (development, screenshots): sample data, see demo.ts
      config = demoConfig(params);
      status = demoStatus(params);
      snapshot = params.get("empty") ? null : demoSnapshot(params);
      wizard = params.has("wizard");
      return;
    }
    const un: Promise<() => void>[] = [];
    (async () => {
      config = await api.getConfig();
      sidSource = await api.serviceIdSource();
      status = await api.getStatus();
      snapshot = await api.getSnapshot();
      wizard = !config.ui.setup_done && config.character.length === 0;
      // messages from the start, e.g. a damaged database that was set aside
      for (const n of await api.startupNotices()) notify((t.notices[n.kind] ?? ((d: string) => d))(n.detail), true);
    })().catch((e) => notify(errorText(e), true));
    un.push(on<Snapshot | null>("snapshot", (s) => (snapshot = s)));
    un.push(on<EssStatus>("status", (s) => (status = s)));
    un.push(on<boolean>("overlay-mode", (e) => (moving = e)));
    un.push(
      on<{ via: string; saved: boolean }>("move-finished", (f) => {
        if (f.via === "timeout") notify(t.move.timedOut);
      }),
    );
    api.overlayState().then((s) => (moving = s.edit)).catch(() => {});
    un.push(
      on("config-changed", async () => {
        config = await api.getConfig();
        sidSource = await api.serviceIdSource();
      }),
    );
    return () => un.forEach((p) => p.then((f) => f()));
  });

  /** Where the active service ID comes from (the value never reaches the UI). */
  let sidSource = $state<ServiceIdSource | null>(null);
  const noServiceId = $derived(
    !config || (inTauri() ? sidSource === "example" : !config.census.service_id || config.census.service_id === "example"),
  );
  const conn = $derived.by(() => {
    if (noServiceId) return { cls: "warn", text: t.status.noServiceId };
    switch (status?.state) {
      case "connected":
        return { cls: "connected", text: t.status.connected };
      case "connecting":
        return { cls: "connecting", text: t.status.connecting };
      case "disconnected":
        return { cls: "disconnected", text: t.status.reconnecting(status.retry_in_secs) };
      default:
        return { cls: "idle", text: t.status.idle };
    }
  });
  const fac = $derived(faction(activeFaction));

  async function finishWizard(c: Config | null) {
    wizard = false;
    if (c) {
      c.ui.setup_done = true;
      await save(c);
    } else if (config) {
      config.ui.setup_done = true;
      await save();
    }
  }
</script>

<div class="shell" class:open={navOpen}>
  <button class="burger" aria-label={t.nav.menu} aria-expanded={navOpen} onclick={() => (navOpen = !navOpen)}>☰</button>
  <nav aria-label={t.nav.main}>
    <div class="brand">
      <span class="logo" aria-hidden="true"></span>
      <strong>{t.app.name}</strong>
    </div>
    {#each groups as g, gi (gi)}
      {#if g.title}<div class="group">{g.title}</div>{:else}<div class="sep"></div>{/if}
      {#each g.items as p (p.id)}
        <button class="nav" class:active={page === p.id} aria-current={page === p.id ? "page" : undefined} onclick={() => go(p.id)}>
          <span class="ico" aria-hidden="true">{p.icon}</span>{p.label}
        </button>
      {/each}
    {/each}
    <div class="spacer"></div>
    {#if $appInfo}
      <button class="link version small muted" title={`${infoLine($appInfo)} – ${t.about.copyHint}`} onclick={copyVersion}>v{$appInfo.version}</button>
    {/if}
    <div class="status" title={status?.detail ?? ""}>
      {#if snapshot || activeChar}
        <div class="who">
          <span class="fac" style:--fc={fac.color}>{fac.short}</span>
          <span class="name">{snapshot?.character_name ?? activeChar?.name}</span>
        </div>
        {#if snapshot && locationLabel(snapshot)}
          <div class="loc small muted">{locationLabel(snapshot)}</div>
        {/if}
      {:else}
        <button class="link" onclick={() => go("characters")}>{t.status.addCharacter}</button>
      {/if}
      <div class="conn" role="status">
        <span class="dot {conn.cls}" aria-hidden="true"></span>
        <span>{t.status.label}: {conn.text}</span>
      </div>
    </div>
  </nav>

  <main>
    {#if moving}
      <div class="move-banner" role="status">
        <span>{t.move.banner}</span>
        <button class="primary" onclick={finishMove}>{t.move.finish}</button>
      </div>
    {/if}
    {#if !config}
      <p class="muted">{t.app.loadingConfig}</p>
    {:else if page === "live"}
      <Live {snapshot} {config} {notify} {go} />
    {:else if page === "sessions"}
      <Sessions {notify} />
    {:else if page === "squad"}
      <Squad {config} {save} {notify} go={(p) => go(p)} />
    {:else if page === "analytics"}
      <!-- loaded on demand: Chart.js stays out of the start-up bundle -->
      {#await import("./Analytics.svelte")}
        <p class="muted" role="status">{t.analytics.loading}</p>
      {:then m}
        <m.default {notify} theme={config.ui.theme} />
      {/await}
    {:else if page === "characters"}
      <Characters {config} {save} {notify} />
    {:else if page === "overlay"}
      <OverlayPage {config} {save} {notify} />
    {:else if page === "crosshair"}
      <CrosshairPage {config} {save} {notify} />
    {:else if page === "voice"}
      <VoicePage {config} {save} {notify} />
    {:else if page === "settings"}
      <SettingsPage {config} {save} {notify} openWizard={() => (wizard = true)} />
    {/if}
  </main>

  {#if wizard && config}
    <Wizard {config} {notify} done={finishWizard} {go} />
  {/if}

  {#if message}
    <div class="toast" class:error={message.error} role={message.error ? "alert" : "status"}>
      <span aria-hidden="true">{message.error ? "⚠" : "✓"}</span>{message.text}
    </div>
  {/if}
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 232px 1fr;
    height: 100vh;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--s2) var(--s1);
    background: var(--bg-2);
    border-right: 1px solid var(--line);
    overflow-y: auto;
  }
  .brand {
    display: flex;
    gap: var(--s1);
    align-items: center;
    padding: 0 var(--s1) var(--s2);
  }
  .logo {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background:
      radial-gradient(circle at center, transparent 6px, var(--accent) 6px 9px, transparent 9px),
      var(--panel);
    border: 1px solid var(--line);
    flex: none;
  }
  .group {
    margin: var(--s2) var(--s1) 4px;
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--muted);
  }
  .sep {
    height: 1px;
    background: var(--line);
    margin: var(--s2) var(--s1) var(--s1);
  }
  .nav {
    display: flex;
    gap: var(--s1);
    align-items: center;
    text-align: left;
    background: transparent;
    border: 1px solid transparent;
    border-left: 3px solid transparent;
    padding: var(--s1) 12px;
  }
  .nav:hover {
    background: var(--panel);
  }
  .nav.active {
    background: var(--panel);
    border-left-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .ico {
    width: 1.1rem;
    text-align: center;
  }
  .spacer {
    flex: 1;
  }
  .status {
    display: grid;
    gap: var(--s1);
    padding: 12px var(--s1) 4px;
    border-top: 1px solid var(--line);
    font-size: 0.85rem;
  }
  .who {
    display: flex;
    gap: var(--s1);
    align-items: center;
    min-width: 0;
  }
  .who .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .move-banner {
    display: flex;
    gap: var(--s2);
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--s2);
    padding: var(--s1) var(--s2);
    border: 1px solid var(--accent, #4fc3c9);
    border-radius: 8px;
    background: color-mix(in srgb, var(--accent, #4fc3c9) 12%, transparent);
  }
  .status .loc {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: 2px;
  }
  .fac {
    flex: none;
    padding: 1px 6px;
    border-radius: 6px;
    font-size: 0.72rem;
    font-weight: 700;
    background: var(--fc);
    color: #fff;
  }
  .link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    text-align: left;
  }
  .link.version {
    color: var(--muted);
    padding: 0 var(--s1);
    font-variant-numeric: tabular-nums;
  }
  .conn {
    display: flex;
    gap: var(--s1);
    align-items: center;
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex: none;
    background: var(--muted);
  }
  .dot.connected {
    background: var(--good);
  }
  .dot.connecting,
  .dot.warn {
    background: var(--warn);
  }
  .dot.disconnected {
    background: var(--bad);
  }
  main {
    overflow: auto;
    padding: var(--s3) var(--s4) var(--s4);
  }
  .burger {
    display: none;
  }
  .toast {
    position: fixed;
    right: var(--s3);
    bottom: var(--s3);
    display: flex;
    gap: var(--s1);
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-left: 4px solid var(--good);
    padding: 12px var(--s2);
    border-radius: 8px;
    max-width: 440px;
    z-index: 50;
    box-shadow: 0 8px 24px rgb(0 0 0 / 30%);
  }
  .toast.error {
    border-left-color: var(--danger);
  }
  @media (max-width: 1100px) {
    .shell {
      grid-template-columns: 1fr;
    }
    nav {
      position: fixed;
      inset: 0 auto 0 0;
      width: 232px;
      z-index: 40;
      transform: translateX(-100%);
      transition: transform 0.15s;
    }
    .shell.open nav {
      transform: none;
    }
    .burger {
      display: block;
      position: fixed;
      top: var(--s1);
      left: var(--s1);
      z-index: 41;
    }
    main {
      padding-top: var(--s4);
    }
  }
</style>
