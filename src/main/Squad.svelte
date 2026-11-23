<script lang="ts">
  // Squad/outfit mode: live values of friends or outfit members next to your own.
  // Sounds and crosshair feedback stay with your own active character.
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { t } from "../lib/i18n";
  import { api, errorText, inTauri, on } from "../lib/api";
  import type { Config, OutfitMember, SquadRow } from "../lib/types";
  import ClassIcon from "../lib/ClassIcon.svelte";
  import { faction, locationLabel, num, pct } from "../lib/format";

  let { config, save, notify, go }: { config: Config; save: (c?: Config) => Promise<void>; notify: (t: string, e?: boolean) => void; go?: (p: "overlay") => void } = $props();

  const MAX = 20;
  let rows = $state<SquadRow[]>([]);
  let sortKey = $state<keyof SquadRow>("kd");
  let desc = $state(true);
  let name = $state("");
  let tag = $state("");
  let onlineOnly = $state(true);
  let busy = $state(false);
  let outfit = $state<{ outfit: string; members: OutfitMember[] } | null>(null);
  let picked = $state<string[]>([]);

  const DEMO: SquadRow[] = [
    { character_id: "1", name: "DemoSoldierTR", faction_id: 3, own: true, online: true, kills: 42, deaths: 21, revives: 3, kd: 2, kpm: 0.93, hsr: 0.26, started_at: 0, world_id: 10, zone_id: 2, world_name: "Miller", zone_name: "Indar", class: "heavy_assault" },
    { character_id: "2", name: "DemoMedicTR", faction_id: 3, own: false, online: true, kills: 18, deaths: 15, revives: 31, kd: 1.2, kpm: 0.4, hsr: 0.18, started_at: 0, world_id: 10, zone_id: 2, world_name: "Miller", zone_name: "Indar", class: "medic" },
    { character_id: "3", name: "DemoEngineerTR", faction_id: 3, own: false, online: true, kills: 25, deaths: 10, revives: 0, kd: 2.5, kpm: 0.55, hsr: 0.31, started_at: 0, world_id: 10, zone_id: 6, world_name: "Miller", zone_name: "Amerish", class: "engineer" },
    { character_id: "4", name: "DemoMaxTR", faction_id: 3, own: false, online: false, kills: 0, deaths: 0, revives: 0, kd: 0, kpm: 0, hsr: 0, started_at: null },
  ];

  async function load() {
    if (!inTauri()) {
      rows = DEMO;
      return;
    }
    try {
      rows = await api.squadRows();
    } catch (e) {
      notify(errorText(e), true);
    }
  }
  onMount(() => {
    load();
    const timer = setInterval(load, 5000);
    const un = [on("snapshot", load), on("config-changed", load)];
    return () => {
      clearInterval(timer);
      un.forEach((p) => p.then((f) => f()));
    };
  });

  const sorted = $derived(
    [...rows].sort((a, b) => {
      const x = a[sortKey];
      const y = b[sortKey];
      const c =
        typeof x === "string" || typeof y === "string"
          ? String(x ?? "").localeCompare(String(y ?? ""))
          : Number(x ?? 0) - Number(y ?? 0);
      return desc ? -c : c;
    }),
  );
  function sortBy(k: keyof SquadRow) {
    if (sortKey === k) desc = !desc;
    else {
      sortKey = k;
      desc = k !== "name";
    }
  }
  const known = (id: string) => config.character.some((c) => c.id === id) || config.squad.some((c) => c.id === id);
  const full = $derived(config.squad.length >= MAX);

  async function addByName(e: SubmitEvent) {
    e.preventDefault();
    if (!name.trim() || full) return;
    busy = true;
    try {
      const c = await api.lookupCharacter(name.trim());
      if (known(c.character_id)) notify(t.squad.duplicate, true);
      else {
        config.squad.push({ name: c.name, id: c.character_id, faction_id: c.faction_id, enabled: true });
        await save();
        name = "";
        notify(t.squad.added(c.name));
      }
    } catch (err) {
      notify(errorText(err), true);
    } finally {
      busy = false;
    }
  }

  async function findOutfit(e: SubmitEvent) {
    e.preventDefault();
    if (!tag.trim()) return;
    busy = true;
    try {
      outfit = await api.outfitMembers(tag.trim());
      picked = [];
    } catch (err) {
      notify(errorText(err), true);
    } finally {
      busy = false;
    }
  }

  async function addPicked() {
    if (!outfit) return;
    const room = MAX - config.squad.length;
    const list = outfit.members.filter((m) => picked.includes(m.character_id) && !known(m.character_id));
    if (list.length > room) {
      notify(t.squad.tooMany(MAX), true);
      return;
    }
    for (const m of list) config.squad.push({ name: m.name, id: m.character_id, faction_id: m.faction_id ?? undefined, enabled: true });
    await save();
    notify(t.squad.addedN(list.length));
    outfit = null;
  }

  async function remove(r: SquadRow) {
    const del = inTauri() ? await ask(t.squad.deleteAsk(r.name), { title: t.squad.removeTitle, kind: "warning", okLabel: t.squad.deleteData, cancelLabel: t.squad.keepData }) : false;
    try {
      const n = await api.squadRemove(r.character_id, del);
      notify(t.squad.removed(r.name, n));
      await load();
    } catch (err) {
      notify(errorText(err), true);
    }
  }

  const COLS: { k: keyof SquadRow; l: string; num?: boolean }[] = [
    { k: "name", l: t.squad.colName },
    { k: "online", l: t.squad.colStatus },
    { k: "class", l: t.classes.class },
    { k: "zone_name", l: t.squad.colLocation },
    { k: "kd", l: "K/D", num: true },
    { k: "kpm", l: "KPM", num: true },
    { k: "hsr", l: "HSR", num: true },
    { k: "kills", l: t.squad.colKills, num: true },
    { k: "deaths", l: t.squad.colDeaths, num: true },
    { k: "revives", l: t.squad.colRevives, num: true },
  ];
  const visible = $derived(onlineOnly ? (outfit?.members ?? []).filter((m) => m.online) : (outfit?.members ?? []));
</script>

<h1>{t.squad.title}</h1>
<p class="muted">{t.squad.intro}</p>

{#if config.squad.length > 0 && !config.overlay.show_squad}
  <p class="hint row">
    {t.squad.overlayOff}
    {#if go}<button class="link" onclick={() => go?.("overlay")}>{t.squad.overlayOffLink}</button>{/if}
  </p>
{/if}

<section class="card">
  {#if rows.length === 0}
    <p class="muted">{t.squad.empty}</p>
  {:else}
    <table class="squad">
      <thead>
        <tr>
          {#each COLS as c (c.k)}
            <th class:num={c.num} aria-sort={sortKey === c.k ? (desc ? "descending" : "ascending") : "none"}>
              <button class="th" onclick={() => sortBy(c.k)}>{c.l}{sortKey === c.k ? (desc ? " ▼" : " ▲") : ""}</button>
            </th>
          {/each}
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each sorted as r (r.character_id)}
          <tr class:own={r.own}>
            <td>
              <span class="fac" style:background={faction(r.faction_id).color}>{faction(r.faction_id).short}</span>
              {r.name}{#if r.own} <span class="badge">{t.squad.you}</span>{/if}
            </td>
            <td><span class="dot" class:on={r.online}></span> {r.online ? t.squad.online : t.squad.offline}</td>
            <td><ClassIcon cls={r.online ? r.class : null} /> {r.online && r.class ? t.classes.short[r.class] : ""}</td>
            <td class="loc">{(r.online && locationLabel(r)) || "–"}</td>
            <td class="num">{num(r.kd, 2)}</td>
            <td class="num">{num(r.kpm, 2)}</td>
            <td class="num">{pct(r.hsr, 0)}</td>
            <td class="num">{r.kills}</td>
            <td class="num">{r.deaths}</td>
            <td class="num">{r.revives}</td>
            <td>{#if !r.own}<button class="mini" onclick={() => remove(r)} aria-label={t.squad.removeAria(r.name)}>✕</button>{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
  <p class="hint small">{t.squad.privacy}</p>
</section>

<div class="grid cols">
  <section class="card grid">
    <h3>{t.squad.addTitle} <span class="muted">({config.squad.length}/{MAX})</span></h3>
    <form class="row" onsubmit={addByName}>
      <input bind:value={name} placeholder={t.squad.namePlaceholder} aria-label={t.squad.namePlaceholder} maxlength="32" />
      <button type="submit" disabled={busy || full || !inTauri()}>{busy ? t.squad.searching : t.squad.add}</button>
    </form>
  </section>
  <section class="card grid">
    <h3>{t.squad.outfitTitle}</h3>
    <form class="row" onsubmit={findOutfit}>
      <input bind:value={tag} placeholder={t.squad.outfitPlaceholder} aria-label={t.squad.outfitPlaceholder} maxlength="32" />
      <button type="submit" disabled={busy || !inTauri()}>{t.squad.find}</button>
    </form>
    {#if outfit}
      <p class="small"><strong>{outfit.outfit}</strong> · {t.squad.members(outfit.members.length, outfit.members.filter((m) => m.online).length)}</p>
      <label class="check small"><input type="checkbox" bind:checked={onlineOnly} /> {t.squad.onlineOnly}</label>
      <ul class="pick">
        {#each visible.slice(0, 200) as m (m.character_id)}
          <li>
            <label class="check small">
              <input type="checkbox" disabled={known(m.character_id)} checked={picked.includes(m.character_id)} onchange={(e) => (picked = e.currentTarget.checked ? [...picked, m.character_id] : picked.filter((x) => x !== m.character_id))} />
              {m.name} {#if m.online}<span class="dot on"></span>{/if}
            </label>
          </li>
        {/each}
      </ul>
      <button onclick={addPicked} disabled={picked.length === 0}>{t.squad.addPicked(picked.length)}</button>
    {/if}
  </section>
</div>

<style>
  .squad {
    width: 100%;
  }
  .th {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  tr.own td {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    font-weight: 600;
  }
  .fac {
    display: inline-block;
    min-width: 2.4em;
    margin-right: 0.4em;
    padding: 0 0.3em;
    border-radius: 4px;
    color: #fff;
    font-size: 0.75rem;
    font-weight: 700;
    text-align: center;
  }
  .dot {
    display: inline-block;
    width: 0.55em;
    height: 0.55em;
    border-radius: 50%;
    background: var(--muted);
  }
  .dot.on {
    background: var(--kill);
  }
  .cols {
    grid-template-columns: 1fr 1fr;
    align-items: start;
    margin-top: var(--s2);
  }
  .row {
    display: flex;
    gap: var(--s1);
  }
  .row input {
    flex: 1;
  }
  .pick {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 260px;
    overflow: auto;
    display: grid;
    gap: 2px;
  }
  button.mini {
    padding: 0.1rem 0.45rem;
    font-size: 0.8rem;
  }
  @media (max-width: 900px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>
