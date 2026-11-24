<script lang="ts">
  // Settings → Online sync. Off by default; nothing is sent to the stats
  // server until it is switched on and the user signed in.
  import { onMount, untrack } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { t } from "../lib/i18n";
  import { api, errorText, inTauri, on } from "../lib/api";
  import { dateTime } from "../lib/format";
  import type { Config, SyncStatus } from "../lib/types";

  let { config, save, notify }: { config: Config; save: (c?: Config) => Promise<void>; notify: (t: string, e?: boolean) => void } = $props();

  let status = $state<SyncStatus | null>(null);
  let busy = $state(false);
  let showAdvanced = $state(false);
  const o = $derived(config.online_sync);
  const idOk = $derived(o.discord_client_id === "" || /^\d{17,20}$/.test(o.discord_client_id));
  // the address saved last, to tell a change and to undo a cancelled one
  let savedBase = $state(untrack(() => config.online_sync.api_base));

  function normBase(b: string): string {
    return b.trim().replace(/\/+$/, "");
  }

  async function refresh() {
    if (!inTauri()) return;
    try {
      status = await api.syncStatus();
    } catch {
      // keep the last status
    }
  }

  onMount(() => {
    refresh();
    const un = on("sync-changed", refresh);
    return () => un.then((f) => f());
  });

  async function toggle(e: Event) {
    const box = e.currentTarget as HTMLInputElement;
    if (box.checked) {
      const ok = await ask(t.onlineSync.confirm, { title: t.onlineSync.confirmTitle, kind: "info", okLabel: t.onlineSync.enable });
      if (!ok) {
        box.checked = false;
        o.enabled = false;
        return;
      }
    }
    o.enabled = box.checked;
    await save();
    await refresh();
  }

  async function saveAdvanced() {
    if (!idOk) {
      notify(t.onlineSync.badClientId, true);
      return;
    }
    if (normBase(o.api_base) !== normBase(savedBase)) {
      // sign-in data goes to the new address: say so before saving
      const shown = normBase(o.api_base) || t.onlineSync.defaultApi;
      const ok = await ask(t.onlineSync.apiBaseConfirm(shown), {
        title: t.onlineSync.apiBaseConfirmTitle,
        kind: "warning",
        okLabel: t.onlineSync.apiBaseChange,
      });
      if (!ok) {
        o.api_base = savedBase;
        return;
      }
    }
    await save();
    savedBase = o.api_base;
    await refresh();
  }

  async function signIn() {
    busy = true;
    try {
      status = await api.syncLogin();
    } catch (e) {
      notify(errorText(e), true);
      await refresh();
    } finally {
      busy = false;
    }
  }

  async function signOut() {
    try {
      status = await api.syncLogout();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function uploadExisting() {
    if (!(await ask(t.onlineSync.uploadExistingConfirm, { title: t.onlineSync.title, kind: "info" }))) return;
    try {
      notify(t.onlineSync.uploadExistingDone(await api.syncUploadExisting()));
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function deleteAccount() {
    if (!(await ask(t.onlineSync.deleteConfirm, { title: t.onlineSync.deleteTitle, kind: "warning" }))) return;
    try {
      status = await api.syncDeleteAccount();
      notify(t.onlineSync.deleted);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function openPrivacy() {
    try {
      await api.syncOpenPrivacy();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  function size(bytes: number): string {
    return bytes >= 1048576 ? `${(bytes / 1048576).toFixed(1)} MiB` : `${Math.ceil(bytes / 1024)} KiB`;
  }
</script>

<section class="card grid narrow">
  <h3>{t.onlineSync.title}</h3>
  <p class="muted small">{t.onlineSync.intro}</p>
  <h4>{t.onlineSync.whatTitle}</h4>
  <ul class="small">
    {#each t.onlineSync.what as line (line)}<li>{line}</li>{/each}
  </ul>
  <div class="row">
    <button class="mini" onclick={openPrivacy} disabled={!inTauri()}>{t.onlineSync.privacy}</button>
    {#if status?.privacy_url}<span class="mono small muted">{status.privacy_url}</span>{/if}
  </div>
  <label class="check"><input type="checkbox" checked={o.enabled} onchange={toggle} /> {t.onlineSync.enable}</label>

  {#if status && !status.default_api}
    <p class="hint small warn" role="note">{t.onlineSync.otherApi(status.api_base)}</p>
  {/if}
  {#if o.enabled && status}
    {#if status.server_unavailable}<p class="hint small warn">{t.onlineSync.unavailable}</p>{/if}
    {#if status.keyring === "unavailable"}<p class="hint small warn">{t.onlineSync.noKeyring}</p>{/if}
    {#if status.signed_in}
      <p>{status.account ? t.onlineSync.signedInAs(status.account.discord_id) : t.onlineSync.signedInStored}</p>
      {#if status.account}
        {#if !status.account.upload_allowed}
          <p class="hint small warn">{t.onlineSync.uploadsClosed(status.account.purge_at ? dateTime(status.account.purge_at) : "–")}</p>
        {/if}
        <div class="small">
          <span class="muted">{t.onlineSync.characters}:</span>
          {#if status.account.characters.length}
            {status.account.characters.map((c) => c.name || c.character_id).join(", ")}
          {:else}
            {t.onlineSync.noCharacters}
          {/if}
        </div>
        <p class="small muted">{t.onlineSync.stored(status.account.storage.sessions, size(status.account.storage.bytes))}</p>
      {/if}
      <p class="small muted">{t.onlineSync.queue(status.counts.queued, status.counts.failed, status.counts.synced)}</p>
      <div class="row">
        <button onclick={uploadExisting} disabled={!status.account}>{t.onlineSync.uploadExisting}</button>
        <button onclick={signOut}>{t.onlineSync.signOut}</button>
        <button class="danger" onclick={deleteAccount}>{t.onlineSync.deleteAccount}</button>
      </div>
    {:else if !status.client_id_available}
      <p class="hint small warn">{status.default_api ? t.onlineSync.noClientId : t.onlineSync.noClientIdOtherApi}</p>
    {:else}
      <div class="row">
        {#if status.login_pending || busy}
          <span class="small">{t.onlineSync.signingIn}</span>
          <button onclick={() => api.syncCancelLogin()}>{t.onlineSync.cancel}</button>
        {:else}
          <button class="primary" onclick={signIn}>{t.onlineSync.signIn}</button>
        {/if}
      </div>
      <p class="hint small">{t.onlineSync.signInHint}</p>
    {/if}
    {#if status.last_error}<p class="hint small warn" role="status">{status.last_error}</p>{/if}
  {/if}

  <button class="mini link" onclick={() => (showAdvanced = !showAdvanced)} aria-expanded={showAdvanced}>{t.onlineSync.advanced}</button>
  {#if showAdvanced}
    <label class="field">{t.onlineSync.apiBase}
      <input class="mono" bind:value={o.api_base} spellcheck="false" autocomplete="off" onchange={saveAdvanced} />
    </label>
    <p class="hint small">{t.onlineSync.apiBaseHint}</p>
    <label class="field">{t.onlineSync.clientId}
      <input class="mono" bind:value={o.discord_client_id} inputmode="numeric" maxlength="20" onchange={saveAdvanced} aria-invalid={!idOk} />
    </label>
    <p class="hint small">{t.onlineSync.clientIdHint}</p>
  {/if}
</section>

<style>
  .row {
    display: flex;
    gap: var(--s1);
    align-items: center;
    flex-wrap: wrap;
  }
  ul {
    margin: 0;
    padding-left: 1.2rem;
    display: grid;
    gap: 4px;
  }
  h4 {
    margin: 0;
  }
  .warn {
    color: var(--warn, #e9b949);
  }
  .link {
    justify-self: start;
  }
</style>
