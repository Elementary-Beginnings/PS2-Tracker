<script lang="ts">
  import { t } from "../lib/i18n";
  import type { ClassId, FeedRow, Metric, Snapshot } from "../lib/types";
  import ClassIcon from "../lib/ClassIcon.svelte";
  import { playtime } from "../lib/resume";
  import { clock, dateTime, delta, duration, faction, feedIcon, feedLabel, feedTone, feedWho, num, pct } from "../lib/format";

  let { snap, now, primaryKd = "true", showMine = false }: { snap: Snapshot; now: number; primaryKd?: string; showMine?: boolean } = $props();
  /** Feed rows about another player get the class symbol. */
  const personKinds = new Set(["kill", "death", "teamkill", "teamkilled"]);
  /** Another player is involved (not the environment, not a suicide). */
  const hasOther = (e: FeedRow) => personKinds.has(e.kind) && e.other_label !== "environment" && e.other_label !== "self";
  const cls = $derived(snap.classes ?? { enemy: [], own: [] });
  const known = $derived(cls.enemy.some((r) => r.class));
  const className = (c: ClassId | null) => (c ? t.classes.names[c] : t.classes.unknown);
  const mins = (secs: number) => (secs >= 60 ? `${Math.round(secs / 60)} min` : secs > 0 ? "< 1 min" : "–");
  /** Bars relative to the largest value of the table. */
  const most = (rows: { kills: number; deaths: number }[], k: "kills" | "deaths") => Math.max(1, ...rows.map((r) => r[k]));
  const kdRev = $derived(snap.rates.kd_revived ?? snap.rates.kd);

  const f = $derived(faction(snap.faction_id));
  const dur = $derived(playtime(snap, now));
  const s = $derived(snap.stats);
  const T = t.session.tiles;
  const c = $derived(snap.comparison);

  // four headline numbers, each with its lifetime difference
  const primary = $derived([
    primaryKd === "revived"
      ? { label: T.kdRevived, value: num(kdRev, 2), m: null, life: null, help: t.session.help.kdRevived, sub: `${T.kdTrue}: ${num(snap.rates.kd, 2)}` }
      : { label: T.kd, value: num(snap.rates.kd, 2), m: c?.kd ?? null, life: c ? num(c.kd.lifetime, 2) : null, help: `${t.session.help.kd}\n${t.session.help.kdRevived}`, sub: `${T.kdRevived}: ${num(kdRev, 2)}` },
    { label: T.kpm, value: num(snap.rates.kpm, 2), m: c?.kpm ?? null, life: c ? num(c.kpm.lifetime, 2) : null, help: t.session.help.kpm },
    { label: T.hsr, value: pct(snap.rates.hsr), m: c?.hsr ?? null, life: c ? pct(c.hsr.lifetime) : null, help: t.session.help.hsr },
    { label: t.session.killsDeaths, value: `${num(s.kills)} / ${num(s.deaths)}`, m: null, life: null, help: t.session.help.kills(num(s.headshots), num(s.suicides)) },
  ]);
  const secondary = $derived([
    { label: T.streak, value: `${num(snap.streak)}`, sub: t.session.sub.best(num(snap.best_streak)) },
    { label: T.revives, value: num(s.revives), sub: t.session.sub.revivesShort(num(s.assists)) },
    { label: T.xph, value: num(snap.rates.xp_per_hour), sub: c ? delta(c.xp_per_hour.delta_pct) : "" },
    { label: T.vehicles, value: num(s.vehicle_kills), sub: t.session.sub.vehiclesLost(num(s.vehicles_lost)) },
    { label: T.teamkills, value: num(s.teamkills), sub: "" },
    { label: t.session.duration, value: duration(dur), sub: "" },
  ]);

  const dir = (m: Metric | null) => (!m || m.delta_pct === null || Math.abs(m.delta_pct) < 0.5 ? "" : m.delta_pct > 0 ? "up" : "down");
  const arrow = (m: Metric | null) => (dir(m) === "up" ? "▲" : dir(m) === "down" ? "▼" : "•");
</script>

<header class="head" style:--fc={f.color}>
  <span class="fac">{f.short}</span>
  <h1>{snap.character_name}</h1>
  {#if snap.active}<span class="badge live">{t.session.live}</span>{/if}
  <span class="muted small">{t.session.since(dateTime(snap.started_at), duration(dur))}</span>
</header>

<section class="primary" aria-label={t.session.headline}>
  {#each primary as p (p.label)}
    <div class="tile" title={p.help}>
      <div class="lbl">{p.label} <span class="info" aria-hidden="true">ⓘ</span></div>
      <div class="val num">{p.value}</div>
      {#if "sub" in p && p.sub}<div class="small muted num">{p.sub}</div>{/if}
      {#if p.m}
        <div class="row small">
          <span class="chip {dir(p.m)}" aria-label={t.session.vsLifetimeAria(delta(p.m.delta_pct))}>{arrow(p.m)} {delta(p.m.delta_pct)}</span>
          <span class="muted">{t.session.sub.lifetime(p.life ?? "–")}</span>
        </div>
      {:else}
        <div class="muted small">{p.help}</div>
      {/if}
      <span class="sr-only">{p.help}</span>
    </div>
  {/each}
</section>

<section class="secondary" aria-label={t.session.more}>
  {#each secondary as x (x.label)}
    <div><span class="muted small">{x.label}</span><b class="num">{x.value}</b>{#if x.sub}<span class="muted small">{x.sub}</span>{/if}</div>
  {/each}
</section>

<div class="cols">
  <div class="grid">
    <details class="card section" open>
      <summary><h3>{t.session.weapons}</h3></summary>
      {#if snap.weapons.length === 0}
        <p class="muted">{t.session.noKills}</p>
      {:else}
        <table>
          <thead>
            <tr><th>{t.session.colWeapon}</th><th class="num">{t.session.colKills}</th><th class="num">{t.session.colHs}</th><th class="num">{t.session.colHsr}</th><th class="num">{t.session.colHsrLifetime}</th></tr>
          </thead>
          <tbody>
            {#each snap.weapons as w (w.weapon_id)}
              <tr>
                <td>{w.name || `#${w.weapon_id}`}</td>
                <td class="num">{w.kills}</td>
                <td class="num">{w.headshots}</td>
                <td class="num">{pct(w.hsr, 0)}</td>
                <td class="num muted">{w.lifetime_hsr != null ? pct(w.lifetime_hsr, 0) : "–"}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
      {#if snap.vehicle_kills.length}
        <div class="row small vk">
          <span class="muted">{t.session.vehicleKills}</span>
          {#each snap.vehicle_kills as [name, n] (name)}<span class="badge">{name} × {n}</span>{/each}
        </div>
      {/if}
    </details>

    <details class="card section classes" open>
      <summary><h3>{t.classes.title}{#if snap.class}<span class="muted small now"> · {t.classes.mine}: <ClassIcon cls={snap.class} /> {className(snap.class)}</span>{/if}</h3></summary>
      {#if !known}
        <p class="muted">{t.classes.none}</p>
      {:else}
        <div class="ctables">
          {#each [{ h: t.classes.killsBy, k: "kills" as const }, { h: t.classes.deathsBy, k: "deaths" as const }] as tb (tb.k)}
            <table>
              <thead><tr><th>{tb.h}</th><th class="num">{tb.k === "kills" ? t.session.colKills : t.session.tiles.deaths}</th></tr></thead>
              <tbody>
                {#each cls.enemy.filter((r) => r[tb.k] > 0) as r (r.class ?? "none")}
                  <tr>
                    <td><ClassIcon cls={r.class} /> {className(r.class)}</td>
                    <td class="num"><span class="bar {tb.k}" style:width={`${(r[tb.k] / most(cls.enemy, tb.k)) * 3}em`}></span>{r[tb.k]}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          {/each}
        </div>
        <table>
          <thead><tr><th>{t.classes.ownUse}</th><th class="num">{t.session.colKills}</th><th class="num">{t.session.tiles.deaths}</th><th class="num">{t.classes.time}</th></tr></thead>
          <tbody>
            {#each cls.own as r (r.class ?? "none")}
              <tr><td><ClassIcon cls={r.class} /> {className(r.class)}</td><td class="num">{r.kills}</td><td class="num">{r.deaths}</td><td class="num muted">{mins(r.secs)}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </details>

    <details class="card section">
      <summary><h3>{t.session.achievements} <span class="muted">({snap.achievements.length})</span></h3></summary>
      {#if snap.achievements.length === 0}
        <p class="muted">{t.session.noAchievements}</p>
      {:else}
        <ul class="ach">
          {#each snap.achievements as a, i (i)}
            <li><strong>{a.name}</strong> <span class="muted small">{a.description} · {clock(a.timestamp)}</span></li>
          {/each}
        </ul>
      {/if}
    </details>

    {#if c}
      <details class="card section">
        <summary><h3>{t.session.comparison}</h3></summary>
        <table>
          <thead><tr><th>{t.session.colValue}</th><th class="num">{t.session.colSession}</th><th class="num">{t.session.colLifetime}</th><th class="num">{t.session.colDelta}</th></tr></thead>
          <tbody>
            {#each [{ l: t.session.metricKd, m: c.kd, f: (v: number) => num(v, 2) }, { l: t.session.metricKpm, m: c.kpm, f: (v: number) => num(v, 2) }, { l: t.session.metricHsr, m: c.hsr, f: (v: number) => pct(v) }, { l: t.session.metricXph, m: c.xp_per_hour, f: (v: number) => num(v) }] as r (r.l)}
              <tr>
                <td>{r.l}</td>
                <td class="num">{r.f(r.m.session)}</td>
                <td class="num muted">{r.f(r.m.lifetime)}</td>
                <td class="num"><span class="chip {dir(r.m)}">{arrow(r.m)} {delta(r.m.delta_pct)}</span></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </details>
    {/if}
  </div>

  <section class="card feedcard" aria-label={t.session.killfeed}>
    <h3>{t.session.killfeed}</h3>
    {#if snap.killfeed.length === 0}
      <p class="muted">{t.session.noEvents}</p>
    {:else}
      <ul class="feed">
        {#each snap.killfeed as e, i (i)}
          <li class={feedTone(e.kind)}>
            <span class="ic" aria-hidden="true">{feedIcon[e.kind] ?? "•"}</span>
            <span class="k">{feedLabel(e.kind)}</span>
            <span class="who" class:muted={!e.other_name && !e.vehicle_name}
              >{#if showMine && hasOther(e)}<ClassIcon cls={e.my_class} title={t.classes.mine} /><span class="sep">›</span>{/if}{#if hasOther(e)}<ClassIcon
                  cls={e.other_class}
                  title={t.classes.enemy}
                />
              {/if}{feedWho(e)}</span
            >
            <span class="w muted">{e.weapon_name}{#if e.headshot}<span class="hs" class:taken={feedTone(e.kind) !== "kill"} title={t.session.headshot}> ⌖ {feedTone(e.kind) === "kill" ? t.overlay.hs : t.overlay.hsTaken}</span>{/if}</span>
            <span class="t num muted">{clock(e.timestamp)}</span>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<style>
  .who :global(.ci) {
    margin-right: 0.3em;
  }
  .who .sep {
    margin: 0 0.15em;
    opacity: 0.6;
  }
  .ctables {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: var(--s2);
    margin-bottom: var(--s2);
  }
  .bar {
    display: inline-block;
    height: 0.55em;
    margin-right: 0.4em;
    border-radius: 2px;
    vertical-align: middle;
    background: var(--kill);
  }
  .bar.deaths {
    background: var(--death);
  }
  .now {
    font-weight: 500;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--s1) var(--s2);
    flex-wrap: wrap;
    margin-bottom: var(--s2);
  }
  .head h1 {
    margin: 0;
  }
  .fac {
    padding: 2px var(--s1);
    border-radius: 6px;
    font-weight: 700;
    font-size: 0.8rem;
    background: var(--fc);
    color: #fff;
  }
  .live {
    color: var(--accent);
    border-color: var(--accent);
  }
  .primary {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--s2);
  }
  .tile {
    background: var(--panel);
    border: 1px solid var(--line);
    border-top: 3px solid var(--accent);
    border-radius: var(--radius);
    padding: var(--s2);
    display: grid;
    gap: 4px;
  }
  .lbl {
    color: var(--muted);
    font-size: 0.8rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .info {
    font-size: 0.75rem;
  }
  .val {
    font-size: 2rem;
    font-weight: 700;
    line-height: 1.1;
  }
  .secondary {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: var(--s1);
    margin: var(--s2) 0;
    padding: 12px var(--s2);
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .secondary > div {
    display: grid;
  }
  .secondary b {
    font-size: 1.15rem;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(300px, 1fr);
    gap: var(--s2);
    align-items: stretch;
  }
  summary h3 {
    display: inline;
  }
  .feedcard {
    display: flex;
    flex-direction: column;
    min-height: 100%;
  }
  .feed {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
  }
  .feed li {
    display: grid;
    grid-template-columns: 1.2rem 4.5rem minmax(0, 1fr) auto;
    grid-template-areas: "ic k who t" "ic k w t";
    column-gap: var(--s1);
    padding: var(--s1) 0;
    border-bottom: 1px solid var(--line);
  }
  .ic {
    grid-area: ic;
    text-align: center;
  }
  .k {
    grid-area: k;
    font-weight: 600;
    font-size: 0.85rem;
  }
  .who {
    grid-area: who;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .w {
    grid-area: w;
    font-size: 0.85rem;
  }
  .t {
    grid-area: t;
    font-size: 0.8rem;
    text-align: right;
  }
  .feed li.kill :is(.k, .ic) {
    color: var(--kill);
  }
  .feed li.death :is(.k, .ic) {
    color: var(--death);
  }
  .feed li.tk :is(.k, .ic) {
    color: var(--tk);
  }
  .hs {
    color: var(--hs);
    font-weight: 700;
  }
  .hs.taken {
    color: var(--hs-taken);
  }
  .ach {
    margin: 0;
    padding-left: 1.1rem;
  }
  .vk {
    margin-top: var(--s1);
  }
  @media (max-width: 1100px) {
    .primary {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .secondary {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    .cols {
      grid-template-columns: 1fr;
    }
  }
</style>
