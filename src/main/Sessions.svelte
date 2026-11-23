<script lang="ts">
  import { t } from "../lib/i18n";
  import { onMount } from "svelte";
  import { ask, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { api, errorText, inTauri, on } from "../lib/api";
  import type { SessionSummary, Snapshot, SyncEntry } from "../lib/types";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { dateTime, duration, num, pct } from "../lib/format";
  import SessionView from "./SessionView.svelte";
  import { demoAnalyses, demoSummaries } from "../lib/demoAnalysis";

  let { notify }: { notify: (t: string, e?: boolean) => void } = $props();

  let list = $state<SessionSummary[]>([]);
  /** Online sync is switched on (shows the column and the upload buttons). */
  let syncOn = $state(false);
  let detail = $state<Snapshot | null>(null);
  let detailId = $state<number | null>(null);

  async function load() {
    if (!inTauri()) {
      // browser preview (screenshots): synthetic sessions
      if (!new URLSearchParams(location.search).has("empty")) list = demoSummaries(demoAnalyses());
      return;
    }
    try {
      list = await api.listSessions(200, 0);
      syncOn = (await api.syncStatus()).enabled;
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  onMount(() => {
    load();
    const un = [on("sessions-changed", load), on("sync-changed", load)];
    return () => un.forEach((u) => u.then((f) => f()));
  });

  const detailSync = $derived(detailId == null ? null : (list.find((s) => s.id === detailId)?.sync ?? null));
  const detailOwn = $derived(detailId == null ? null : list.find((s) => s.id === detailId));

  async function upload() {
    if (detailId == null) return;
    try {
      await api.syncUploadSession(detailId);
      notify(t.sessions.uploadQueued);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  function syncTitle(e: SyncEntry): string {
    if (e.state === "synced") return t.sessions.sync.syncedTitle(e.synced_at ? dateTime(e.synced_at) : "");
    if (e.state === "failed") return t.sessions.sync.failedTitle(e.last_error);
    return t.sessions.sync.queuedTitle(e.last_error);
  }

  async function open(id: number) {
    try {
      detail = await api.getSession(id);
      detailId = id;
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function importFile() {
    // only through the file dialog: the backend reads exactly the chosen file
    const path = await openDialog({ multiple: false, filters: [{ name: t.sessions.importTitle, extensions: ["json", "xlsx"] }] });
    if (typeof path !== "string") return;
    try {
      const r = await api.importSession(path);
      notify((r.duplicate ? t.sessions.duplicate : t.sessions.imported)(r.character_name, dateTime(r.started_at)), r.duplicate);
      await load();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function exportAs(format: "json" | "csv" | "xlsx") {
    if (detailId == null || !detail) return;
    const day = new Date(detail.started_at * 1000).toISOString().slice(0, 10);
    const path = await saveDialog({
      defaultPath: `ps2-session-${detail.character_name}-${day}.${format}`,
      filters: [{ name: format === "xlsx" ? "Excel" : format.toUpperCase(), extensions: [format] }],
    });
    if (!path) return;
    try {
      await api.exportSession(detailId, format, path);
      notify(t.sessions.exported(path));
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function remove() {
    if (detailId == null) return;
    if (!(await ask(t.sessions.deleteConfirm, { title: t.sessions.deleteTitle, kind: "warning" }))) return;
    try {
      await api.deleteSession(detailId);
      detail = null;
      detailId = null;
      notify(t.sessions.deleted);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  const kd = (s: SessionSummary) => (s.deaths > 0 ? s.kills / s.deaths : s.kills);
</script>

{#if detail}
  <div class="row bar">
    <button onclick={() => (detail = null)}>{t.sessions.back}</button>
    <div class="spacer"></div>
    <button onclick={() => exportAs("xlsx")} title={t.sessions.exportTitle}>{t.sessions.exportXlsx}</button>
    <button onclick={() => exportAs("json")} title={t.sessions.exportTitle}>{t.sessions.exportJson}</button>
    <button onclick={() => exportAs("csv")}>{t.sessions.exportCsv}</button>
    {#if syncOn && detailOwn?.ended_at != null && detailSync?.state !== "queued"}
      <button onclick={upload}>{detailSync?.state === "failed" ? t.sessions.retry : t.sessions.uploadNow}</button>
    {/if}
    <button class="danger" onclick={remove}>{t.sessions.delete}</button>
  </div>
  <SessionView snap={detail} now={Math.floor(Date.now() / 1000)} />
{:else}
  <div class="row bar">
    <h1>{t.sessions.title}</h1>
    <div class="spacer"></div>
    <button onclick={importFile} disabled={!inTauri()} title={t.sessions.importTitle}>{t.sessions.import}</button>
  </div>
  {#if list.length === 0}
    <p class="muted">{t.sessions.empty}</p>
  {:else}
    <div class="card">
      <table>
        <thead>
          <tr>
            <th>{t.sessions.colStart}</th><th>{t.sessions.colCharacter}</th><th class="num">{t.sessions.colDuration}</th><th class="num">{t.sessions.colKills}</th><th class="num">{t.sessions.colDeaths}</th>
            <th class="num">{t.sessions.colKd}</th><th class="num">{t.sessions.colHsr}</th><th class="num">{t.sessions.colXp}</th>
            {#if syncOn}<th>{t.sessions.colSync}</th>{/if}
          </tr>
        </thead>
        <tbody>
          {#each list as s (s.id)}
            <tr class="click" tabindex="0" onclick={() => open(s.id)} onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), open(s.id))}>
              <td>
                {dateTime(s.started_at)}{#if s.ended_at == null} <span class="badge">{t.sessions.running}</span>{/if}
                {#if (s.data_version ?? 2) < 2}<span class="badge muted" title={t.sessions.basicDataTitle}>{t.sessions.basicData}</span>{/if}
              </td>
              <td>{s.character_name}</td>
              <td class="num">{s.ended_at ? duration(Math.max(0, s.ended_at - s.started_at - (s.paused_secs ?? 0))) : "–"}</td>
              <td class="num">{s.kills}</td>
              <td class="num">{s.deaths}</td>
              <td class="num">{num(kd(s), 2)}</td>
              <td class="num">{s.kills ? pct(s.headshots / s.kills, 0) : "–"}</td>
              <td class="num">{num(s.xp)}</td>
              {#if syncOn}
                <td>
                  {#if s.sync}<span class="badge sync-{s.sync.state}" title={syncTitle(s.sync)}>{t.sessions.sync[s.sync.state]}</span>{:else}<span class="muted" title={t.sessions.sync.none}>–</span>{/if}
                </td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
{/if}

<style>
  .bar {
    margin-bottom: 0.9rem;
  }
  .spacer {
    flex: 1;
  }
  tr.click {
    cursor: pointer;
  }
  tr.click:hover td,
  tr.click:focus-visible td {
    background: var(--panel-2);
  }
  tr.click:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .bar h1 {
    margin: 0;
  }
  .sync-failed {
    color: var(--bad, #e5534b);
  }
  .sync-queued {
    opacity: 0.8;
  }
</style>
