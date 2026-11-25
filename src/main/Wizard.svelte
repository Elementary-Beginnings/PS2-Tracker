<script lang="ts">
  import { t } from "../lib/i18n";
  import { onMount } from "svelte";
  import { api, errorText, inTauri } from "../lib/api";
  import type { CharacterInfo, Config } from "../lib/types";
  import { faction, worldName } from "../lib/format";

  type Page = "live" | "sessions" | "characters" | "overlay" | "crosshair" | "voice" | "settings";
  let {
    config,
    notify,
    done,
    go,
  }: {
    config: Config;
    notify: (t: string, e?: boolean) => void;
    done: (c: Config | null) => Promise<void>;
    go: (p: Page) => void;
  } = $props();

  let step = $state(1);
  // svelte-ignore state_referenced_locally
  // browser preview: a fresh official build (empty field, built-in default)
  let sid = $state(config.census.service_id === "example" || !inTauri() ? "" : config.census.service_id);
  let showSid = $state(false);
  let name = $state("");
  let found = $state<CharacterInfo | null>(null);
  let busy = $state(false);
  // svelte-ignore state_referenced_locally
  let draft = $state<Config>($state.snapshot(config) as Config);

  /** Builds with a built-in project service ID can go on without an own one. */
  let hasBuiltIn = $state(!inTauri());
  onMount(() => {
    if (inTauri()) api.serviceIdSource().then((x) => (hasBuiltIn = x === "builtin")).catch(() => {});
  });

  async function saveSid() {
    const v = sid.trim().replace(/^s:/, "");
    if (!v && !hasBuiltIn) return;
    draft.census.service_id = v;
    busy = true;
    try {
      if (inTauri()) await api.saveConfig($state.snapshot(draft) as Config);
      step = 2;
    } catch (e) {
      notify(errorText(e), true);
    } finally {
      busy = false;
    }
  }

  async function lookup() {
    if (!name.trim()) return;
    busy = true;
    found = null;
    try {
      found = await api.lookupCharacter(name.trim());
    } catch (e) {
      notify(errorText(e), true);
    } finally {
      busy = false;
    }
  }

  function addFound() {
    if (!found) return;
    if (!draft.character.some((c) => String(c.id) === found!.character_id)) {
      draft.character.push({ name: found.name, id: found.character_id, faction_id: found.faction_id, enabled: true });
    }
    step = 3;
  }

  async function finish(next?: Page) {
    await done(draft);
    if (next) go(next);
  }
</script>

<div class="backdrop">
  <div class="dialog card" role="dialog" aria-modal="true" aria-labelledby="wiz-title">
    <div class="steps" aria-hidden="true">
      {#each [1, 2, 3] as n (n)}<span class:on={step >= n}>{n}</span>{/each}
    </div>
    {#if step === 1}
      <h2 id="wiz-title">{t.wizard.step1Title}</h2>
      <p class="muted">{t.wizard.step1Text}</p>
      <p class="mono small">https://census.daybreakgames.com/#devSignup</p>
      <form class="row" onsubmit={(e) => { e.preventDefault(); saveSid(); }}>
        <label class="field grow">{t.settings.serviceId}
          <input type={showSid ? "text" : "password"} bind:value={sid} autocomplete="off" spellcheck="false" />
        </label>
        <button type="button" onclick={() => (showSid = !showSid)}>{showSid ? t.settings.hide : t.settings.show}</button>
      </form>
      <p class="hint">{t.wizard.storedLocally}</p>
      {#if hasBuiltIn}<p class="hint">{t.wizard.builtInHint}</p>{/if}
      <div class="row end">
        <button onclick={() => done(null)}>{t.wizard.skip}</button>
        <button class="primary" disabled={(!sid.trim() && !hasBuiltIn) || busy} onclick={saveSid}>{t.wizard.next}</button>
      </div>
    {:else if step === 2}
      <h2 id="wiz-title">{t.wizard.step2Title}</h2>
      <form class="row" onsubmit={(e) => { e.preventDefault(); lookup(); }}>
        <input class="grow" placeholder={t.characters.placeholder} bind:value={name} aria-label={t.characters.placeholder} />
        <button type="submit" disabled={busy || !name.trim()}>{t.wizard.search}</button>
      </form>
      {#if found}
        {@const f = faction(found.faction_id)}
        <div class="result" style:--fc={f.color}>
          <span class="fac">{f.short}</span>
          <strong>{found.name}</strong>
          <span class="muted">{f.name} · {worldName(found.world_id)} · BR {found.battle_rank}{found.prestige_level ? ` (ASP ${found.prestige_level})` : ""}</span>
        </div>
      {/if}
      <div class="row end">
        <button onclick={() => (step = 1)}>{t.wizard.back}</button>
        <button onclick={() => (step = 3)}>{t.wizard.later}</button>
        <button class="primary" disabled={!found} onclick={addFound}>{t.wizard.addCharacter}</button>
      </div>
    {:else}
      <h2 id="wiz-title">{t.wizard.step3Title}</h2>
      <p class="muted">{t.wizard.step3Text}</p>
      <div class="row">
        <button onclick={() => finish("voice")}>♪ {t.wizard.importPack}</button>
        <button onclick={() => finish("crosshair")}>✛ {t.wizard.chooseCrosshair}</button>
      </div>
      <div class="row end">
        <button class="primary" onclick={() => finish("live")}>{t.wizard.finish}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 55%);
  }
  .dialog {
    width: min(560px, calc(100vw - 32px));
    display: grid;
    gap: var(--s2);
    padding: var(--s3);
  }
  .steps {
    display: flex;
    gap: var(--s1);
  }
  .steps span {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    border: 1px solid var(--line);
    color: var(--muted);
    font-weight: 700;
  }
  .steps span.on {
    background: var(--accent);
    color: var(--accent-ink);
    border-color: transparent;
  }
  .grow {
    flex: 1;
  }
  .end {
    justify-content: flex-end;
  }
  .result {
    display: flex;
    gap: var(--s1);
    align-items: center;
    flex-wrap: wrap;
    padding: 12px;
    border: 1px solid var(--line);
    border-left: 4px solid var(--fc);
    border-radius: 8px;
  }
  .fac {
    padding: 1px 6px;
    border-radius: 6px;
    font-weight: 700;
    font-size: 0.75rem;
    background: var(--fc);
    color: #fff;
  }
</style>
