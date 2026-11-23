<script lang="ts">
  import { t } from "../lib/i18n";
  import { api, errorText, inTauri } from "../lib/api";
  import type { CharacterInfo, Config } from "../lib/types";
  import { faction } from "../lib/format";

  let {
    config,
    save,
    notify,
  }: { config: Config; save: (c?: Config) => Promise<void>; notify: (t: string, e?: boolean) => void } = $props();

  let name = $state("");
  let found = $state<CharacterInfo | null>(null);
  let busy = $state(false);

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

  async function add() {
    if (!found) return;
    if (config.character.some((c) => String(c.id) === found!.character_id)) {
      notify(t.characters.duplicate, true);
      return;
    }
    config.character.push({
      name: found.name,
      id: found.character_id,
      faction_id: found.faction_id,
      enabled: true,
    });
    found = null;
    name = "";
    await save();
  }

  async function remove(i: number) {
    config.character.splice(i, 1);
    await save();
  }
</script>

<h1>{t.characters.title}</h1>
<p class="muted">{t.characters.intro}</p>

<div class="grid cols">
  <section class="card">
    <h3>{t.characters.add}</h3>
    <form class="row" onsubmit={(e) => { e.preventDefault(); lookup(); }}>
      <input placeholder={t.characters.placeholder} bind:value={name} disabled={!inTauri()} />
      <button type="submit" disabled={busy || !inTauri()}>{busy ? t.characters.searching : t.characters.search}</button>
    </form>
    {#if found}
      {@const f = faction(found.faction_id)}
      <div class="found" style:--fc={f.color}>
        <div>
          <strong>{found.name}</strong>
          <div class="muted small">
            {f.name} · {t.characters.battleRank(found.battle_rank, found.prestige_level)}{#if found.world_id}{t.characters.server(found.world_id)}{/if}
          </div>
          <div class="muted small mono">{t.characters.id(found.character_id)}</div>
        </div>
        <button class="primary" onclick={add}>{t.characters.adopt}</button>
      </div>
    {/if}
  </section>

  <section class="card">
    <h3>{t.characters.listed}</h3>
    {#if config.character.length === 0}
      <p class="muted">{t.characters.none}</p>
    {:else}
      <table>
        <thead><tr><th>{t.characters.colActive}</th><th>{t.characters.colName}</th><th>{t.characters.colFaction}</th><th>{t.characters.colId}</th><th></th></tr></thead>
        <tbody>
          {#each config.character as c, i (c.name + i)}
            {@const f = faction(c.faction_id)}
            <tr>
              <td><input type="checkbox" bind:checked={c.enabled} onchange={() => save()} aria-label={t.characters.colActive} /></td>
              <td>{c.name}</td>
              <td><span class="badge" style:color={f.color}>{f.short}</span></td>
              <td class="mono small muted">{c.id ?? t.characters.pendingId}</td>
              <td><button class="danger" onclick={() => remove(i)}>{t.characters.remove}</button></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
</div>

<style>
  .cols {
    grid-template-columns: minmax(320px, 1fr) 1.4fr;
    align-items: start;
  }
  .found {
    margin-top: 0.8rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-left: 4px solid var(--fc);
    padding: 0.6rem 0.8rem;
    background: var(--panel-2);
    border-radius: 0 8px 8px 0;
  }
</style>
