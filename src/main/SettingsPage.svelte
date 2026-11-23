<script lang="ts">
  import { t } from "../lib/i18n";
  import { onMount } from "svelte";
  import { api, errorText, inTauri } from "../lib/api";
  import type { Config, HotkeyFailure, PathsInfo, ServiceIdSource } from "../lib/types";
  import IntegrationsSettings from "./IntegrationsSettings.svelte";
  import OnlineSyncSettings from "./OnlineSyncSettings.svelte";
  import { appInfo, infoLine, loadAppInfo, ISSUES_URL, WIKI_URL } from "../lib/appInfo";

  let {
    config,
    save,
    notify,
    openWizard,
  }: {
    config: Config;
    save: (c?: Config) => Promise<void>;
    notify: (t: string, e?: boolean) => void;
    openWizard: () => void;
  } = $props();

  type Tab = "general" | "appearance" | "privacy" | "online" | "integrations" | "advanced";
  let tab = $state<Tab>((new URLSearchParams(location.search).get("tab") as Tab | null) ?? "general");
  let paths = $state<PathsInfo | null>(null);
  let sid = $state("");
  let showSid = $state(false);
  $effect.pre(() => {
    sid = config.census.service_id;
  });
  /** Hotkeys the desktop holds (e.g. KWin "Switch to Desktop 2" on Ctrl+F2). */
  let hotkeyFailures = $state<HotkeyFailure[]>([]);
  /** Origin of the active service ID; the built-in value itself never reaches the UI. */
  let sidSource = $state<ServiceIdSource | null>(null);
  async function copyInfo() {
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
    if (!inTauri()) return;
    api.serviceIdSource().then((x) => (sidSource = x)).catch(() => {});
    api.paths().then((p) => (paths = p));
    api.hotkeyStatus().then((f) => (hotkeyFailures = f)).catch(() => {});
  });

  async function saveSid() {
    config.census.service_id = sid.trim().replace(/^s:/, "");
    showSid = false;
    await save();
    if (inTauri()) sidSource = await api.serviceIdSource().catch(() => null);
  }
  async function openFolder(kind: string) {
    try {
      await api.openFolder(kind);
    } catch (e) {
      notify(errorText(e), true);
    }
  }
  const tabs: { id: Tab; label: string }[] = [
    { id: "general", label: t.settings.tabGeneral },
    { id: "appearance", label: t.settings.tabAppearance },
    { id: "privacy", label: t.settings.tabPrivacy },
    { id: "online", label: t.onlineSync.title },
    { id: "integrations", label: t.integrations.tab },
    { id: "advanced", label: t.settings.tabAdvanced },
  ];
</script>

<h1>{t.settings.title}</h1>

<div class="tabs" role="tablist">
  {#each tabs as x (x.id)}
    <button role="tab" aria-selected={tab === x.id} onclick={() => (tab = x.id)}>{x.label}</button>
  {/each}
</div>

{#if hotkeyFailures.length}
  <div class="card warn-box" role="alert">
    {#each hotkeyFailures as f (f.hotkey)}<p class="small">{t.overlay.hotkeyFailed(f.hotkey, f.action)}</p>{/each}
  </div>
{/if}

{#if tab === "general"}
  <section class="card grid narrow">
    <h3>{t.settings.census}</h3>
    <label class="field">{t.settings.serviceId}
      <div class="row">
        {#if showSid}
          <input class="mono" bind:value={sid} autocomplete="off" spellcheck="false" />
          <button class="primary" onclick={saveSid}>{t.settings.apply}</button>
        {:else}
          <span class="masked mono" aria-label={t.settings.maskedAria}>{sid ? "••••••••••" : t.settings.emptyDefault}</span>
          <button onclick={() => (showSid = true)}>{t.settings.showEdit}</button>
        {/if}
      </div>
    </label>
    {#if sidSource}<p class="hint">{t.settings.serviceIdInUse[sidSource]}</p>{/if}
    <p class="hint">{t.settings.serviceIdHint}</p>
    <div class="row"><button onclick={openWizard}>{t.settings.runWizard}</button></div>

    <h3>{t.settings.sessions}</h3>
    <div class="row">
      <label class="field">{t.settings.idle}
        <input type="number" min="5" max="240" bind:value={config.session.idle_timeout_minutes} onchange={() => save()} /></label>
      <label class="field">{t.settings.resume}
        <input type="number" min="0" max="120" bind:value={config.session.resume_minutes} onchange={() => save()} /></label>
      <label class="field">{t.settings.streakStep}
        <input type="number" min="0" max="50" bind:value={config.session.streak_step} onchange={() => save()} /></label>
      <label class="field">{t.settings.feedLen}
        <input type="number" min="3" max="50" bind:value={config.session.killfeed_len} onchange={() => save()} /></label>
    </div>
    <label class="check"><input type="checkbox" bind:checked={config.ui.start_minimized} onchange={() => save()} /> {t.settings.startTray}</label>
    <label class="field">{t.settings.primaryKd}
      <select bind:value={config.ui.primary_kd} onchange={() => save()}>
        {#each Object.entries(t.settings.primaryKds) as [k, l] (k)}<option value={k}>{l}</option>{/each}
      </select>
    </label>
  </section>
{:else if tab === "appearance"}
  <section class="card grid narrow">
    <label class="field">{t.settings.theme}
      <select bind:value={config.ui.theme} onchange={() => save()}>
        <option value="dark">{t.settings.themeDark}</option>
        <option value="light">{t.settings.themeLight}</option>
        <option value="system">{t.settings.themeSystem}</option>
      </select>
    </label>
    <fieldset class="grid">
      <legend>{t.settings.accent}</legend>
      <label class="check"><input type="radio" name="accent" value="faction" bind:group={config.ui.accent} onchange={() => save()} /> {t.settings.accentFaction}</label>
      <label class="check"><input type="radio" name="accent" value="neutral" bind:group={config.ui.accent} onchange={() => save()} /> {t.settings.accentNeutral}</label>
      <label class="check"><input type="radio" name="accent" value="custom" bind:group={config.ui.accent} onchange={() => save()} /> {t.settings.accentCustom}
        {#if config.ui.accent === "custom"}<input type="color" bind:value={config.ui.accent_custom} onchange={() => save()} aria-label={t.settings.accentCustom} />{/if}
      </label>
    </fieldset>
    <label class="check"><input type="checkbox" bind:checked={config.ui.x11_backend} onchange={() => save()} /> {t.settings.x11Backend}</label>
  </section>
{:else if tab === "privacy"}
  <section class="card grid narrow">
    <p>{t.settings.footer}</p>
    <h3>{t.settings.obs}</h3>
    <label class="check"><input type="checkbox" bind:checked={config.obs.enabled} onchange={() => save()} /> {t.settings.obsEnable}</label>
    <label class="field">{t.settings.port} <input type="number" min="1024" max="65535" bind:value={config.obs.port} onchange={() => save()} /></label>
    {#if config.obs.enabled}
      <p class="small">{t.settings.obsUrl} <span class="mono">http://127.0.0.1:{config.obs.port}/</span></p>
    {/if}
    <p class="muted small">{t.app.name} · {t.app.tagline}</p>
  </section>
{:else if tab === "online"}
  <OnlineSyncSettings {config} {save} {notify} />
{:else if tab === "integrations"}
  <IntegrationsSettings {config} {save} {notify} />
{:else}
  <section class="card grid narrow">
    <h3>{t.about.title}</h3>
    {#if $appInfo}
      <table>
        <tbody>
          <tr><td class="muted">{t.about.version}</td><td class="mono">{$appInfo.version}</td></tr>
          <tr><td class="muted">{t.about.commit}</td><td class="mono">{$appInfo.commit}</td></tr>
          <tr><td class="muted">{t.about.built}</td><td class="mono">{$appInfo.build_date || "–"}</td></tr>
          <tr><td class="muted">{t.about.help}</td><td class="mono small">{WIKI_URL}</td></tr>
          <tr><td class="muted">{t.about.issues}</td><td class="mono small">{ISSUES_URL}</td></tr>
        </tbody>
      </table>
      <div class="row"><button onclick={copyInfo}>{t.about.copy}</button></div>
    {/if}
  </section>
  <section class="card grid narrow">
    <label class="check"><input type="checkbox" bind:checked={config.ui.debug_console} onchange={() => save()} /> {t.settings.debugConsole}</label>
    <p class="muted small">{t.settings.debugConsoleHint}</p>
  </section>
  <section class="card grid">
    {#if paths}
      <table>
        <tbody>
          {#each [{ l: t.settings.fConfig, p: paths.config_file, k: "config" }, { l: t.settings.fAchievements, p: paths.achievements_file, k: "config" }, { l: t.settings.fDatabase, p: paths.database, k: "data" }, { l: t.settings.fVoice, p: paths.voicepacks, k: "voicepacks" }, { l: t.settings.fCrosshairs, p: paths.crosshairs, k: "crosshairs" }, { l: t.settings.fLog, p: paths.log_file, k: "logs" }] as r (r.l)}
            <tr>
              <td class="muted">{r.l}</td>
              <td class="mono small path">{r.p}</td>
              <td><button class="mini" onclick={() => openFolder(r.k)}>{t.settings.openFolder}</button></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="muted">{t.settings.pathsOnlyInApp}</p>
    {/if}
  </section>
{/if}

<style>
  .warn-box {
    border-color: var(--warn, #e9b949);
    margin-bottom: var(--s2);
  }
  .narrow {
    max-width: 720px;
  }
  .masked {
    padding: 6px 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--bg-2);
    letter-spacing: 0.15em;
  }
  fieldset {
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 12px var(--s2);
  }
  legend {
    color: var(--muted);
    padding: 0 4px;
  }
  .path {
    word-break: break-all;
  }
  input[type="number"] {
    width: 7rem;
  }
</style>
