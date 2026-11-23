<script lang="ts">
  import { t } from "../lib/i18n";
  import { onMount } from "svelte";
  import { api, errorText, inTauri, on } from "../lib/api";
  import type { Config, LastSession, Snapshot } from "../lib/types";
  import { ago, dateTime, duration } from "../lib/format";
  import { demoLastSession } from "../lib/demo";
  import { canReopen, pauseLeft } from "../lib/resume";
  import SessionView from "./SessionView.svelte";

  type Page = "live" | "sessions" | "characters" | "overlay" | "crosshair" | "voice" | "settings";
  let {
    snapshot,
    config,
    notify,
    go,
  }: { snapshot: Snapshot | null; config: Config; notify: (t: string, e?: boolean) => void; go: (p: Page) => void } = $props();

  let now = $state(Math.floor(Date.now() / 1000));
  onMount(() => {
    const t = setInterval(() => (now = Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(t);
  });

  const chars = $derived(config.character.filter((c) => c.enabled && c.id));
  let selected = $state("");
  $effect(() => {
    if (!selected && chars[0]?.id) selected = String(chars[0].id);
  });

  // seconds left in the resume window while logged out (null = playing)
  const left = $derived(snapshot?.active ? pauseLeft(snapshot, now) : null);

  // last ended session of the selected character ("Resume last session")
  let last = $state<LastSession | null>(null);
  async function loadLast(id: string) {
    if (!id) return;
    if (!inTauri()) {
      last = demoLastSession(new URLSearchParams(location.search));
      return;
    }
    try {
      last = await api.lastSession(id);
    } catch {
      last = null;
    }
  }
  $effect(() => {
    // again when the selection changes or the running session ends
    if (!snapshot) loadLast(selected);
  });
  onMount(() => {
    if (!inTauri()) return;
    const un = on("sessions-changed", () => {
      if (!snapshot) loadLast(selected);
    });
    return () => {
      un.then((f) => f());
    };
  });
  const reopenOk = $derived(canReopen(last, now));
  const reopenTitle = $derived(
    !last
      ? t.live.resumeLastNone
      : reopenOk
        ? t.live.resumeLastHint(ago(last.ended_at, now), dateTime(last.resumable_until))
        : t.live.resumeLastTooOld(ago(last.ended_at, now), Math.round((last.resumable_until - last.ended_at) / 3600)),
  );

  async function resumeNow() {
    if (!snapshot) return;
    try {
      await api.resumeSession(snapshot.character_id);
      notify(t.live.resumed);
    } catch (e) {
      notify(errorText(e), true);
    }
  }
  async function reopen() {
    try {
      await api.reopenSession(selected);
      notify(t.live.reopened);
    } catch (e) {
      notify(errorText(e), true);
      loadLast(selected);
    }
  }

  async function start() {
    try {
      await api.startSession(selected);
    } catch (e) {
      notify(errorText(e), true);
    }
  }
  async function stop() {
    if (!snapshot) return;
    const paused = left !== null;
    try {
      await api.stopSession(snapshot.character_id);
      notify(paused ? t.live.stoppedAtLogout : t.live.stopped);
    } catch (e) {
      notify(errorText(e), true);
    }
  }
</script>

{#if snapshot}
  <div class="bar">
    {#if !inTauri()}<span class="badge">{t.app.previewBadge}</span>{/if}
    <div class="spacer"></div>
    {#if snapshot.active && left === null}
      <button onclick={stop} aria-label={t.live.stopSession}>■ {t.live.stopSession}</button>
    {/if}
  </div>
  {#if left !== null}
    <div class="card paused" role="status">
      <strong>{t.live.pausedTitle(duration(left))}</strong>
      <p class="muted small">{t.live.pausedHint}</p>
      <div class="row">
        <button class="primary" onclick={resumeNow} disabled={!inTauri()}>▶ {t.live.resumeNow}</button>
        <button onclick={stop} disabled={!inTauri()}>■ {t.live.stopSession}</button>
      </div>
    </div>
  {/if}
  <SessionView snap={snapshot} {now} primaryKd={config.ui.primary_kd} showMine={config.overlay.show_my_class} />
{:else if chars.length === 0}
  <div class="card empty-state">
    <h2>{t.live.noCharacterTitle}</h2>
    <p class="muted">{t.live.noCharacterHint}</p>
    <button class="primary" onclick={() => go("characters")}>{t.live.addCharacter}</button>
  </div>
{:else}
  <div class="card empty-state">
    <h2>{t.live.noSessionTitle}</h2>
    <p class="muted">{t.live.autoStartHint}</p>
    <div class="row">
      {#if chars.length > 1}
        <select bind:value={selected} aria-label={t.live.characterSelect}>
          {#each chars as c (c.id)}<option value={String(c.id)}>{c.name}</option>{/each}
        </select>
      {/if}
      <button class="primary" onclick={start} disabled={!inTauri()}>▶ {t.live.startSession}</button>
      {#if last}
        <!-- the wrapper carries the tooltip: disabled buttons get no hover events -->
        <span title={reopenTitle}>
          <button onclick={reopen} disabled={!inTauri() || !reopenOk} aria-label={`${t.live.resumeLast}: ${reopenTitle}`}
            >↻ {t.live.resumeLast}</button
          >
        </span>
      {/if}
    </div>
    {#if last && !reopenOk}<p class="muted small">{reopenTitle}</p>{/if}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: var(--s1);
    margin-bottom: var(--s1);
  }
  .spacer {
    flex: 1;
  }
  .empty-state {
    max-width: 640px;
  }
  .paused {
    border-color: var(--warn);
    margin-bottom: var(--s1);
  }
  .paused strong {
    color: var(--warn);
  }
</style>
