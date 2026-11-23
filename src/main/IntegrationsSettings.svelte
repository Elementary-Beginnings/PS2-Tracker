<script lang="ts">
  // Settings → Updates & Discord. Both off by default.
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { t } from "../lib/i18n";
  import { api, errorText, inTauri, on } from "../lib/api";
  import type { Config, UpdateInfo, UpdateStatus } from "../lib/types";

  let { config, save, notify }: { config: Config; save: (c?: Config) => Promise<void>; notify: (t: string, e?: boolean) => void } = $props();

  let status = $state<UpdateStatus | null>(null);
  let checking = $state(false);
  let found = $state<UpdateInfo | null>(null);
  const d = $derived(config.discord);
  const idOk = $derived(/^\d{17,20}$/.test(d.app_id));

  onMount(() => {
    if (!inTauri()) {
      status = { available: false, reason: "no_key", current: "0.2.0" };
      return;
    }
    api.updateStatus().then((s) => (status = s)).catch(() => {});
    const un = on<UpdateInfo>("update-available", (u) => (found = u));
    return () => un.then((f) => f());
  });

  async function checkNow() {
    checking = true;
    try {
      found = await api.updateCheck();
      if (!found) notify(t.integrations.upToDate);
    } catch (e) {
      notify(errorText(e), true);
    } finally {
      checking = false;
    }
  }

  async function install() {
    if (!found) return;
    const ok = await ask(t.integrations.confirm(found.version, found.notes || t.integrations.noNotes), {
      title: t.integrations.confirmTitle,
      kind: "info",
      okLabel: t.integrations.install,
    });
    if (!ok) return;
    try {
      await api.updateInstall();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function saveDiscord() {
    if (d.app_id && !idOk) {
      notify(t.integrations.badId, true);
      return;
    }
    await save();
  }
</script>

<section class="card grid narrow">
  <h3>{t.integrations.updatesTitle}</h3>
  {#if status}
    <p class="small muted">{t.integrations.current(status.current)}</p>
    {#if status.available}
      <label class="check"><input type="checkbox" bind:checked={config.updates.check_on_start} onchange={() => save()} /> {t.integrations.checkOnStart}</label>
      <p class="hint small">{t.integrations.optInHint}</p>
      <div class="row">
        <button onclick={checkNow} disabled={checking}>{checking ? t.integrations.checking : t.integrations.checkNow}</button>
        {#if found}
          <span class="small">{t.integrations.found(found.version)}</span>
          <button class="primary" onclick={install}>{t.integrations.install}</button>
        {/if}
      </div>
    {:else}
      <p class="hint small">{t.integrations.reasons[status.reason] ?? status.reason}</p>
    {/if}
  {/if}
</section>

<section class="card grid narrow">
  <h3>{t.integrations.discordTitle}</h3>
  <p class="muted small">{t.integrations.discordIntro}</p>
  <label class="field">{t.integrations.appId}
    <input bind:value={d.app_id} inputmode="numeric" maxlength="20" placeholder="e.g. 112233445566778899" onchange={saveDiscord} aria-invalid={d.app_id !== "" && !idOk} />
  </label>
  <p class="hint small">{t.integrations.appIdHint}</p>
  <label class="check"><input type="checkbox" bind:checked={d.enabled} disabled={!idOk} onchange={saveDiscord} /> {t.integrations.discordEnable}</label>
  <fieldset class="opts" disabled={!d.enabled || !idOk}>
    <legend class="small">{t.integrations.show}</legend>
    <label class="check small"><input type="checkbox" bind:checked={d.show_character} onchange={saveDiscord} /> {t.integrations.showCharacter}</label>
    <label class="check small"><input type="checkbox" bind:checked={d.show_kd} onchange={saveDiscord} /> K/D</label>
    <label class="check small"><input type="checkbox" bind:checked={d.show_kills} onchange={saveDiscord} /> {t.integrations.showKills}</label>
    <label class="check small"><input type="checkbox" bind:checked={d.show_duration} onchange={saveDiscord} /> {t.integrations.showDuration}</label>
    <label class="check small"><input type="checkbox" bind:checked={d.show_location} onchange={saveDiscord} /> {t.integrations.showLocation}</label>
  </fieldset>
  <p class="muted small">{t.integrations.discordQuiet}</p>
</section>

<style>
  .row {
    display: flex;
    gap: var(--s1);
    align-items: center;
    flex-wrap: wrap;
  }
  .opts {
    display: grid;
    gap: 4px;
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: var(--s1) var(--s2);
  }
</style>
