<script lang="ts">
  import { t } from "../lib/i18n";
  import { onMount } from "svelte";
  import { api, errorText, inTauri } from "../lib/api";
  import type { Config, Snapshot } from "../lib/types";
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

  async function start() {
    try {
      await api.startSession(selected);
    } catch (e) {
      notify(errorText(e), true);
    }
  }
  async function stop() {
    if (!snapshot) return;
    try {
      await api.stopSession(snapshot.character_id);
      notify(t.live.stopped);
    } catch (e) {
      notify(errorText(e), true);
    }
  }
</script>

{#if snapshot}
  <div class="bar">
    {#if !inTauri()}<span class="badge">{t.app.previewBadge}</span>{/if}
    <div class="spacer"></div>
    {#if snapshot.active}
      <button onclick={stop} aria-label={t.live.stopSession}>■ {t.live.stopSession}</button>
    {/if}
  </div>
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
    </div>
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
</style>
