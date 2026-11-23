<script lang="ts">
  // Analytics: compare stored sessions (trend, weapons, progress, factions, A vs B).
  import { onMount } from "svelte";
  import type { ChartConfiguration } from "chart.js";
  import { t } from "../lib/i18n";
  import { api, errorText, inTauri, on } from "../lib/api";
  import type { Analysis, SessionSummary } from "../lib/types";
  import { dateTime, delta, num, pct } from "../lib/format";
  import { ACCENTS, effectiveMode, factionAccent } from "../lib/theme";
  import { demoAnalyses, demoSummaries } from "../lib/demoAnalysis";
  import ChartBox from "../lib/ChartBox.svelte";

  let { notify, theme }: { notify: (t: string, e?: boolean) => void; theme: string } = $props();

  const MAX = 50;
  let list = $state<SessionSummary[]>([]);
  let selected = $state<number[]>([]);
  let cache = $state<Record<number, Analysis>>({});
  let loading = $state(false);
  let character = $state("");
  let from = $state("");
  let to = $state("");
  let metric = $state<"kd" | "kd_revived" | "kpm" | "hsr" | "xp_per_hour">("kd");
  let progressId = $state<number | null>(null);
  let aId = $state<number | null>(null);
  let bId = $state<number | null>(null);
  let colors = $state({ text: "#e7e9ee", muted: "#9aa3b4", line: "#2b3342", accent: "#4fc3c9", kill: "#5fd99a", death: "#ffa557", hs: "#7fd4ff" });

  function readColors() {
    const cs = getComputedStyle(document.documentElement);
    const v = (n: string, d: string) => cs.getPropertyValue(n).trim() || d;
    colors = {
      text: v("--text", colors.text),
      muted: v("--muted", colors.muted),
      line: v("--line", colors.line),
      accent: v("--accent", colors.accent),
      kill: v("--kill", colors.kill),
      death: v("--death", colors.death),
      hs: v("--hs", colors.hs),
    };
  }

  async function loadList() {
    try {
      if (!inTauri()) {
        const demo = demoAnalyses();
        cache = Object.fromEntries(demo.map((a) => [a.id ?? 0, a]));
        list = demoSummaries(demo);
      } else {
        list = await api.listSessions(500, 0);
      }
      if (selected.length === 0) selected = list.slice(0, 10).map((s) => s.id);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  onMount(() => {
    readColors();
    loadList();
    const mo = new MutationObserver(readColors);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "style"] });
    const mq = matchMedia("(prefers-color-scheme: light)");
    mq.addEventListener("change", readColors);
    const un = on("sessions-changed", loadList);
    return () => {
      mo.disconnect();
      mq.removeEventListener("change", readColors);
      un.then((f) => f());
    };
  });

  // fetch evaluations of newly selected sessions (in chunks of 50)
  $effect(() => {
    const missing = selected.filter((id) => !cache[id]);
    if (!missing.length || !inTauri()) return;
    loading = true;
    api
      .sessionAnalysis(missing.slice(0, MAX))
      .then((res) => {
        const next = { ...cache };
        for (const a of res) if (a.id != null) next[a.id] = a;
        cache = next;
      })
      .catch((e) => notify(errorText(e), true))
      .finally(() => (loading = false));
  });

  const characters = $derived([...new Set(list.map((s) => s.character_name))].sort());
  const filtered = $derived(
    list.filter((s) => {
      if (character && s.character_name !== character) return false;
      const day = new Date(s.started_at * 1000).toISOString().slice(0, 10);
      if (from && day < from) return false;
      if (to && day > to) return false;
      return true;
    }),
  );
  /** Selected and loaded, oldest first. */
  const chosen = $derived(
    selected
      .map((id) => cache[id])
      .filter((a): a is Analysis => a != null)
      .sort((a, b) => a.started_at - b.started_at),
  );
  $effect(() => {
    const ids = chosen.map((a) => a.id);
    if (progressId == null || !ids.includes(progressId)) progressId = chosen.at(-1)?.id ?? null;
    if (aId == null || !ids.includes(aId)) aId = chosen.at(-2)?.id ?? chosen.at(-1)?.id ?? null;
    if (bId == null || !ids.includes(bId)) bId = chosen.at(-1)?.id ?? null;
  });

  function toggle(id: number, on: boolean) {
    if (on && selected.length >= MAX) {
      notify(t.analytics.tooMany(MAX), true);
      return;
    }
    selected = on ? [...selected, id] : selected.filter((x) => x !== id);
  }
  const selectShown = () => (selected = filtered.slice(0, MAX).map((s) => s.id));

  const mode = $derived(effectiveMode(theme));
  const factionColor = (id: number | null) => ACCENTS[mode][factionAccent(id)].accent;
  const label = (a: Analysis) => new Date(a.started_at * 1000).toLocaleDateString("en-GB", { day: "2-digit", month: "short" });

  const axes = $derived({
    x: { ticks: { color: colors.muted }, grid: { color: colors.line } },
    y: { ticks: { color: colors.muted }, grid: { color: colors.line }, beginAtZero: true },
  });
  const legend = $derived({ labels: { color: colors.text, boxWidth: 12 } });

  const trend = $derived<ChartConfiguration>({
    type: "line",
    data: {
      labels: chosen.map(label),
      datasets: [
        {
          label: t.analytics.metrics[metric],
          data: chosen.map((a) => (metric === "hsr" ? a.rates.hsr * 100 : a.rates[metric])),
          borderColor: colors.accent,
          backgroundColor: colors.accent,
          pointBackgroundColor: chosen.map((a) => factionColor(a.faction_id)),
          pointRadius: 4,
          tension: 0.25,
        },
      ],
    },
    options: { scales: axes, plugins: { legend } },
  });

  const weaponTotals = $derived.by(() => {
    const m = new Map<string, { kills: number; hs: number }>();
    for (const a of chosen)
      for (const w of a.weapons) {
        const x = m.get(w.name) ?? { kills: 0, hs: 0 };
        x.kills += w.kills;
        x.hs += w.headshots;
        m.set(w.name, x);
      }
    return [...m.entries()].filter(([, x]) => x.kills > 0).sort((a, b) => b[1].kills - a[1].kills).slice(0, 10);
  });
  const weaponsChart = $derived<ChartConfiguration>({
    type: "bar",
    data: {
      labels: weaponTotals.map(([n]) => n),
      datasets: [
        { label: t.analytics.kills, data: weaponTotals.map(([, x]) => x.kills), backgroundColor: colors.kill, yAxisID: "y" },
        { type: "line", label: t.analytics.hsrPct, data: weaponTotals.map(([, x]) => (x.hs / x.kills) * 100), borderColor: colors.hs, backgroundColor: colors.hs, yAxisID: "y2" },
      ],
    } as ChartConfiguration["data"],
    options: {
      scales: { ...axes, y2: { position: "right", ticks: { color: colors.muted }, grid: { display: false }, beginAtZero: true, max: 100 } },
      plugins: { legend },
    },
  });

  const progress = $derived(chosen.find((a) => a.id === progressId) ?? null);
  const progressChart = $derived<ChartConfiguration>({
    type: "line",
    data: {
      labels: (progress?.series ?? []).map((p) => `${p.minute}′`),
      datasets: [
        { label: t.analytics.kills, data: (progress?.series ?? []).map((p) => p.kills), borderColor: colors.kill, backgroundColor: colors.kill, pointRadius: 0, yAxisID: "y" },
        { label: t.analytics.deaths, data: (progress?.series ?? []).map((p) => p.deaths), borderColor: colors.death, backgroundColor: colors.death, pointRadius: 0, yAxisID: "y" },
        { label: t.analytics.metrics.kd, data: (progress?.series ?? []).map((p) => p.kd), borderColor: colors.accent, backgroundColor: colors.accent, pointRadius: 0, borderDash: [5, 4], yAxisID: "y2" },
      ],
    },
    options: {
      scales: { ...axes, y2: { position: "right", ticks: { color: colors.muted }, grid: { display: false }, beginAtZero: true } },
      plugins: { legend },
    },
  });

  const teamTotals = $derived.by(() => {
    const m = new Map<number, { kills: number; deaths: number }>();
    for (const a of chosen)
      for (const x of a.teams) {
        const v = m.get(x.team_id) ?? { kills: 0, deaths: 0 };
        v.kills += x.kills;
        v.deaths += x.deaths;
        m.set(x.team_id, v);
      }
    return [...m.entries()].sort((a, b) => a[0] - b[0]);
  });
  const TEAM = (id: number) => ({ 1: "VS", 2: "NC", 3: "TR", 4: "NSO" })[id] ?? t.analytics.unknownTeam;
  const teamsChart = $derived<ChartConfiguration>({
    type: "bar",
    data: {
      labels: teamTotals.map(([id]) => TEAM(id)),
      datasets: [
        { label: t.analytics.killsOf, data: teamTotals.map(([, v]) => v.kills), backgroundColor: teamTotals.map(([id]) => factionColor(id)) },
        { label: t.analytics.deathsBy, data: teamTotals.map(([, v]) => v.deaths), backgroundColor: teamTotals.map(([id]) => factionColor(id) + "80"), borderColor: teamTotals.map(([id]) => factionColor(id)), borderWidth: 1 },
      ],
    },
    options: { scales: axes, plugins: { legend } },
  });

  /** Kills/deaths by enemy class over the chosen sessions (data version 4). */
  const classTotals = $derived.by(() => {
    const m = new Map<string, { kills: number; deaths: number }>();
    for (const a of chosen)
      for (const x of a.classes?.enemy ?? []) {
        if (!x.class) continue;
        const v = m.get(x.class) ?? { kills: 0, deaths: 0 };
        v.kills += x.kills;
        v.deaths += x.deaths;
        m.set(x.class, v);
      }
    const order = ["infiltrator", "light_assault", "medic", "engineer", "heavy_assault", "max"];
    return [...m.entries()].sort((a, b) => order.indexOf(a[0]) - order.indexOf(b[0]));
  });
  const classesChart = $derived<ChartConfiguration>({
    type: "bar",
    data: {
      labels: classTotals.map(([c]) => t.classes.names[c] ?? c),
      datasets: [
        { label: t.analytics.killsOf, data: classTotals.map(([, v]) => v.kills), backgroundColor: colors.kill },
        { label: t.analytics.deathsBy, data: classTotals.map(([, v]) => v.deaths), backgroundColor: colors.death },
      ],
    },
    options: { scales: axes, plugins: { legend } },
  });

  const A = $derived(chosen.find((a) => a.id === aId) ?? null);
  const B = $derived(chosen.find((a) => a.id === bId) ?? null);
  const rows = $derived.by(() => {
    if (!A || !B) return [];
    const r = (l: string, a: number, b: number, fmt: (v: number) => string, higherBetter = true) => ({
      l, a: fmt(a), b: fmt(b), d: a ? ((b - a) / Math.abs(a)) * 100 : null, better: higherBetter ? b >= a : b <= a,
    });
    return [
      r(t.analytics.metrics.kd, A.rates.kd, B.rates.kd, (v) => num(v, 2)),
      r(t.analytics.metrics.kd_revived, A.rates.kd_revived, B.rates.kd_revived, (v) => num(v, 2)),
      r(t.analytics.metrics.kpm, A.rates.kpm, B.rates.kpm, (v) => num(v, 2)),
      r(t.analytics.metrics.hsr, A.rates.hsr, B.rates.hsr, (v) => pct(v, 1)),
      r(t.analytics.metrics.xp_per_hour, A.rates.xp_per_hour, B.rates.xp_per_hour, (v) => num(v, 0)),
      r(t.analytics.kills, A.stats.kills, B.stats.kills, (v) => num(v, 0)),
      r(t.analytics.deaths, A.stats.deaths, B.stats.deaths, (v) => num(v, 0), false),
      r(t.analytics.bestStreak, A.best_streak, B.best_streak, (v) => num(v, 0)),
      r(t.analytics.revives, A.stats.revives, B.stats.revives, (v) => num(v, 0)),
    ];
  });
  const sessionName = (a: Analysis) => `${dateTime(a.started_at)} · ${a.character_name}`;
  const oldData = $derived(chosen.some((a) => a.data_version < 2));
</script>

<h1>{t.analytics.title}</h1>
<p class="muted">{t.analytics.intro}</p>

{#if list.length === 0}
  <section class="card empty">
    <p>{t.analytics.empty}</p>
  </section>
{:else}
  <div class="layout">
    <section class="card pick" aria-label={t.analytics.pickTitle}>
      <h3>{t.analytics.pickTitle} <span class="muted">({selected.length}/{MAX})</span></h3>
      <div class="filters">
        <label class="field">{t.analytics.character}
          <select bind:value={character}>
            <option value="">{t.analytics.allCharacters}</option>
            {#each characters as c (c)}<option value={c}>{c}</option>{/each}
          </select>
        </label>
        <div class="row">
          <label class="field">{t.analytics.from}<input type="date" bind:value={from} /></label>
          <label class="field">{t.analytics.to}<input type="date" bind:value={to} /></label>
        </div>
        <div class="row">
          <button class="mini" onclick={selectShown}>{t.analytics.selectShown}</button>
          <button class="mini" onclick={() => (selected = [])}>{t.analytics.selectNone}</button>
        </div>
      </div>
      <ul class="sessions">
        {#each filtered as s (s.id)}
          <li>
            <label class="check small">
              <input type="checkbox" checked={selected.includes(s.id)} onchange={(e) => toggle(s.id, e.currentTarget.checked)} />
              <span>{dateTime(s.started_at)} · {s.character_name}</span>
              <span class="muted">{s.kills}/{s.deaths}</span>
            </label>
          </li>
        {/each}
      </ul>
    </section>

    <div class="charts">
      {#if loading}<p class="muted small" role="status">{t.analytics.loading}</p>{/if}
      {#if oldData}<p class="hint small">{t.analytics.oldData}</p>{/if}
      {#if chosen.length === 0}
        <section class="card"><p class="muted">{t.analytics.selectSome}</p></section>
      {:else}
        <div class="seg" role="radiogroup" aria-label={t.analytics.metric}>
          {#each Object.entries(t.analytics.metrics) as [k, l] (k)}
            <button role="radio" aria-checked={metric === k} class:on={metric === k} onclick={() => (metric = k as typeof metric)}>{l}</button>
          {/each}
        </div>
        <div class="grid2">
          <ChartBox title={t.analytics.trendTitle(t.analytics.metrics[metric])} config={trend} file={`ps2-trend-${metric}`} {notify} />
          <ChartBox title={t.analytics.weaponsTitle} config={weaponsChart} file="ps2-weapons" empty={weaponTotals.length === 0} {notify} />
          <div class="stack">
            <label class="field small">{t.analytics.progressOf}
              <select bind:value={progressId}>
                {#each chosen as a (a.id)}<option value={a.id}>{sessionName(a)}</option>{/each}
              </select>
            </label>
            <ChartBox title={t.analytics.progressTitle} config={progressChart} file="ps2-session-progress" empty={!progress || progress.series.length < 2} {notify} />
          </div>
          <ChartBox title={t.analytics.teamsTitle} config={teamsChart} file="ps2-factions" empty={teamTotals.length === 0} {notify} />
          <ChartBox title={t.analytics.classesTitle} config={classesChart} file="ps2-classes" empty={classTotals.length === 0} {notify} />
        </div>
        <section class="card">
          <h3>{t.analytics.compareTitle}</h3>
          <div class="row">
            <label class="field small">A
              <select bind:value={aId}>{#each chosen as a (a.id)}<option value={a.id}>{sessionName(a)}</option>{/each}</select>
            </label>
            <label class="field small">B
              <select bind:value={bId}>{#each chosen as a (a.id)}<option value={a.id}>{sessionName(a)}</option>{/each}</select>
            </label>
          </div>
          <table class="cmp">
            <thead><tr><th></th><th class="num">A</th><th class="num">B</th><th class="num">{t.analytics.change}</th></tr></thead>
            <tbody>
              {#each rows as r (r.l)}
                <tr>
                  <td>{r.l}</td><td class="num">{r.a}</td><td class="num">{r.b}</td>
                  <td class="num" class:up={r.d != null && r.d !== 0 && r.better} class:down={r.d != null && r.d !== 0 && !r.better}>
                    {r.d == null ? "–" : `${r.d > 0 ? "▲" : r.d < 0 ? "▼" : ""} ${delta(r.d)}`}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </section>
      {/if}
    </div>
  </div>
{/if}

<style>
  .layout {
    display: grid;
    grid-template-columns: minmax(240px, 300px) minmax(0, 1fr);
    gap: var(--s2);
    align-items: start;
  }
  .pick {
    display: grid;
    gap: var(--s1);
    position: sticky;
    top: var(--s2);
    max-height: calc(100vh - 2 * var(--s3));
    overflow: auto;
  }
  .filters {
    display: grid;
    gap: var(--s1);
  }
  .row {
    display: flex;
    gap: var(--s1);
    flex-wrap: wrap;
    align-items: end;
  }
  .sessions {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
  }
  .sessions label {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: var(--s1);
    align-items: center;
  }
  .charts {
    display: grid;
    gap: var(--s2);
    min-width: 0;
  }
  .grid2 {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
    gap: var(--s2);
  }
  .stack {
    display: grid;
    gap: var(--s1);
    align-content: start;
  }
  .cmp {
    width: 100%;
    max-width: 640px;
  }
  .up {
    color: var(--good);
  }
  .down {
    color: var(--bad);
  }
  button.mini {
    padding: 0.15rem 0.5rem;
    font-size: 0.82rem;
  }
  @media (max-width: 900px) {
    .layout {
      grid-template-columns: 1fr;
    }
    .pick {
      position: static;
      max-height: 320px;
    }
  }
</style>
