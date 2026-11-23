<script lang="ts">
  import { t } from "../lib/i18n";
  import { onMount } from "svelte";
  import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { api, errorText, inTauri } from "../lib/api";
  import type { Config, EventInfo, MappingProposal, PackEntry, PackFormat, PackInfo, TriggerRule, VoiceOverview, VoicePack } from "../lib/types";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { demoOverview } from "../lib/demo";
  import Menu from "../lib/Menu.svelte";
  import TriggerEditor from "./TriggerEditor.svelte";
  import { ago } from "../lib/format";
  import type { ImportResult, RemovedPack } from "../lib/types";

  const STANDARD_ID = "standard-tones";
  /** Browser preview only: `?menu=import|pack|remove|trigger` opens that UI for screenshots. */
  const demoMenu = inTauri() ? "" : (new URLSearchParams(location.search).get("menu") ?? "");

  let {
    config,
    save,
    notify,
  }: { config: Config; save: (c?: Config) => Promise<void>; notify: (t: string, e?: boolean) => void } = $props();

  const v = $derived(config.voice);
  let overview = $state<VoiceOverview | null>(null);
  let devices = $state<string[]>([]);
  let editing = $state<VoicePack | null>(null);
  let wizard = $state<{ folder: string; mapping: MappingProposal[]; name: string; author: string; license: string } | null>(null);
  let newPack = $state({ name: "", author: "", license: t.voice.defaultLicense });
  let importReport = $state<ImportResult | null>(null);
  let removing = $state<{ pack: PackInfo; files: number; bytes: number } | null>(null);
  let removeDialog = $state<HTMLDialogElement>();
  let undoable = $state<{ removed: RemovedPack; name: string } | null>(null);
  let undoTimer: ReturnType<typeof setTimeout> | undefined;
  let showAll = $state(false);
  let editingTrigger = $state<EventInfo | null>(null);

  const events = $derived(overview?.events ?? []);
  const eventLabel = (k: string) => events.find((e) => e.key === k)?.label ?? k;
  const shownEvents = $derived(events.filter((e) => showAll || !e.recursion || e.trigger));
  const openCount = $derived(events.filter((e) => !e.trigger).length);

  async function load() {
    if (!inTauri()) {
      overview = demoOverview();
      return;
    }
    overview = await api.voiceOverview();
    devices = await api.voiceOutputDevices();
  }
  onMount(() => {
    load()
      .then(() => {
        const last = sortedPacks[sortedPacks.length - 1];
        if (demoMenu === "remove" && last) askRemove(last);
        if (demoMenu === "trigger") editingTrigger = events.find((e) => e.key === "HEAD_SHOT") ?? null;
      })
      .catch((e) => notify(errorText(e), true));
  });

  async function run<T>(f: () => Promise<T>, ok?: string): Promise<T | undefined> {
    try {
      const r = await f();
      if (ok) notify(ok);
      return r;
    } catch (e) {
      notify(errorText(e), true);
      return undefined;
    }
  }

  async function setActive(id: string, on: boolean) {
    v.active_packs = on ? [...v.active_packs.filter((x) => x !== id), id] : v.active_packs.filter((x) => x !== id);
    await save();
    await load();
  }
  async function move(id: string, dir: -1 | 1) {
    const a = [...v.active_packs];
    const i = a.indexOf(id);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= a.length) return;
    [a[i], a[j]] = [a[j]!, a[i]!];
    v.active_packs = a;
    await save();
    await load();
  }

  async function importPack(folder: boolean) {
    const path = await open(
      folder
        ? { directory: true }
        : { filters: [{ name: t.voice.importDialogFilter, extensions: ["rtst_vpk", "zip", "rar"] }] },
    );
    if (typeof path !== "string") return;
    await importPath(path);
  }

  /** A freshly imported pack becomes active at rank 1. */
  async function afterImport(r: ImportResult) {
    importReport = r;
    v.active_packs = [r.id, ...v.active_packs.filter((x) => x !== r.id)];
    await save();
    notify(t.voice.imported(r.name, r.format === "recursion"));
    await load();
  }

  function formatLabel(r: ImportResult): string {
    if (r.source.includes("protobuf")) return t.voice.fmtPb;
    if (r.source.startsWith("recursion")) return `${t.voice.fmtPak} (${r.source.replace(/^recursion:\s*/, "")})`;
    return r.format === "recursion" ? t.voice.fmtXml : t.voice.fmtOpen;
  }

  async function askRemove(p: PackInfo) {
    const size = inTauri() ? await run(() => api.voicePackSize(p.id)) : { files: p.files, bytes: 0 };
    if (!size) return;
    removing = { pack: p, ...size };
    await Promise.resolve();
    removeDialog?.showModal();
  }
  function closeRemove() {
    removeDialog?.close();
    removing = null;
  }
  async function confirmRemove() {
    if (!removing) return;
    const p = removing.pack;
    closeRemove();
    const r = await run(() => api.voiceRemovePack(p.id));
    if (!r) return;
    // mirror what the backend changed
    v.active_packs = v.active_packs.filter((x) => x !== p.id);
    const o = { ...v.overrides };
    for (const k of Object.keys(r.overrides)) delete o[k];
    v.overrides = o;
    clearTimeout(undoTimer);
    undoable = { removed: r, name: p.meta.name };
    undoTimer = setTimeout(() => (undoable = null), 9500);
    notify(t.voice.removed(p.meta.name));
    await load();
  }
  async function undoRemove() {
    if (!undoable) return;
    const u = undoable;
    undoable = null;
    clearTimeout(undoTimer);
    if (await run(() => api.voiceUndoRemove(u.removed).then(() => true))) {
      const a = v.active_packs.filter((x) => x !== u.removed.id);
      if (u.removed.position != null) a.splice(Math.min(u.removed.position, a.length), 0, u.removed.id);
      v.active_packs = a;
      v.overrides = { ...v.overrides, ...u.removed.overrides };
      notify(t.voice.undone(u.name));
      await load();
    }
  }

  async function exportPack(id: string, format: PackFormat) {
    const path = await saveDialog({ defaultPath: `${id}${format === "recursion" ? "-recursion" : ""}.zip`, filters: [{ name: "ZIP", extensions: ["zip"] }] });
    if (path) await run(() => api.voiceExport(id, path, format), t.voice.exported(path));
  }

  async function createPack() {
    if (!newPack.name.trim()) return;
    const id = await run(() => api.voiceNewPack(newPack.name, newPack.author, newPack.license), t.voice.created);
    if (id) {
      newPack.name = "";
      await load();
      editing = (await run(() => api.voiceGetPack(id))) ?? null;
    }
  }

  async function edit(id: string) {
    editing = (await run(() => api.voiceGetPack(id))) ?? null;
  }

  async function addFile(event: string) {
    if (!editing) return;
    const path = await open({ filters: [{ name: t.voice.audioFilter, extensions: ["ogg", "mp3", "wav", "flac"] }] });
    if (typeof path !== "string") return;
    const id = editing.id;
    await run(() => api.voiceAddFile(id, event, path));
    editing = (await run(() => api.voiceGetPack(id))) ?? null;
  }

  function removeFile(entry: PackEntry, file: string) {
    entry.files = entry.files.filter((f) => f !== file);
  }

  async function saveEditing() {
    if (!editing) return;
    const m = $state.snapshot(editing.manifest);
    m.event = m.event.filter((e) => e.files.length > 0);
    const id = editing.id;
    await run(() => api.voiceSaveManifest(id, m), t.voice.packSaved);
    editing = (await run(() => api.voiceGetPack(id))) ?? null;
    await load();
  }

  async function setOverride(event: string) {
    const path = await open({ filters: [{ name: t.voice.audioFilter, extensions: ["ogg", "mp3", "wav", "flac"] }] });
    if (typeof path !== "string") return;
    // the backend copies the file into the app's data folder
    const copy = await run(() => api.voiceSetOverride(event, path));
    if (copy) v.overrides = { ...v.overrides, [event]: [copy] };
  }
  async function clearOverride(event: string) {
    if (inTauri() && (await run(() => api.voiceClearOverride(event).then(() => true))) !== true) return;
    const o = { ...v.overrides };
    delete o[event];
    v.overrides = o;
  }
  async function toggleMute(event: string, muted: boolean) {
    v.muted = muted ? [...v.muted, event] : v.muted.filter((e) => e !== event);
    await save();
  }

  async function startWizard() {
    const folder = await open({ directory: true });
    if (typeof folder !== "string") return;
    const mapping = await run(() => api.voiceProposeMapping(folder));
    if (mapping) wizard = { folder, mapping, name: folder.split("/").pop() ?? t.voice.defaultPackName, author: "", license: t.voice.defaultLicense };
  }
  async function finishWizard() {
    if (!wizard) return;
    const w = $state.snapshot(wizard);
    const id = await run(
      () => api.voiceCreateFromMapping(w.folder, { name: w.name, author: w.author, version: "1.0.0", license: w.license, description: "" }, w.mapping),
      t.voice.fromFolder,
    );
    if (id) {
      wizard = null;
      await load();
    }
  }

  type Tab = "packs" | "events" | "playback";
  type Filter = "all" | "sound" | "notrigger";
  let tab = $state<Tab>((new URLSearchParams(location.search).get("tab") as Tab | null) ?? "packs");
  let query = $state("");
  let filter = $state<Filter>("all");
  let dragOver = $state(false);

  const sortedPacks = $derived(
    [...(overview?.packs ?? [])].sort(
      (a, b) => (a.position ?? 999) - (b.position ?? 999) || a.meta.name.localeCompare(b.meta.name),
    ),
  );

  function formatBadge(p: PackInfo): string {
    const src = p.meta.source ?? "";
    if (src.includes("protobuf")) return t.voice.fmtPb;
    if (src.startsWith("recursion")) return t.voice.fmtPak;
    return p.format === "recursion" ? t.voice.fmtXml : t.voice.fmtOpen;
  }

  /** Pack keys that serve a base event (triggers.default.toml); the standard
   *  tones are only the last resort. */
  const BASE_KEYS: Record<string, string[]> = {
    kill: ["PER_KILL"],
    headshot: ["HEAD_SHOT", "PER_KILL"],
    death: ["DEATH_INSULT"],
    teamkill: ["TEAM_KILLER"],
    teamkilled: ["WHOSE_SIDE", "PREVENT_FF"],
    session_start: ["SESSION_STARTED"],
  };
  /** Pack key → the base event it serves (shown on the key's row instead of a
   *  second, identical description). */
  const SERVES: Record<string, string> = Object.fromEntries(
    Object.entries(BASE_KEYS).flatMap(([base, keys]) => keys.map((k) => [k, base] as const)).reverse(),
  );
  const realPackActive = $derived(sortedPacks.some((p) => p.active && p.id !== STANDARD_ID));

  /** Where the sound of an event comes from right now. */
  function sourceOf(key: string): string {
    if (v.overrides[key]?.length) return t.voice.srcCustom;
    const keys = [...(BASE_KEYS[key] ?? []), ...(key === "kill" && v.kill_key ? [v.kill_key] : [])];
    for (const k of keys) {
      const p = sortedPacks.find((x) => x.active && x.id !== STANDARD_ID && (x.sounds ?? []).includes(k));
      if (p) return t.voice.srcPackKey(k, p.meta.name);
    }
    const p = sortedPacks.find(
      (x) => x.active && (x.sounds ?? []).includes(key) && (x.id !== STANDARD_ID || !realPackActive || v.builtin_fallback),
    );
    return p ? t.voice.srcPack(p.meta.name) : t.voice.srcNone;
  }

  /** Recursion keys with a sound in the active packs (for the kill key choice). */
  /** Per pack: chosen key for "every kill" (preset: the backend's proposal). */
  let killChoice = $state<Record<string, string>>({});
  $effect(() => {
    for (const p of sortedPacks) if (p.suggested_kill_key && !(p.id in killChoice)) killChoice[p.id] = p.suggested_kill_key;
  });
  async function useKillKey(k: string) {
    if (!k) return;
    config.voice.kill_key = k;
    await save();
  }

  const packKeys = $derived(
    [...new Set(sortedPacks.filter((p) => p.active && p.id !== STANDARD_ID).flatMap((p) => p.sounds ?? []))]
      .filter((k) => /^[A-Z]/.test(k))
      .sort(),
  );

  function category(key: string): string {
    const C = t.voice.cats;
    const own: Record<string, string> = {
      kill: C.kills, headshot: C.kills, first_kill: C.kills, multikill: C.multikills, streak: C.streaks,
      streak_ended: C.streaks, death: C.deaths, suicide: C.deaths, teamkilled: C.deaths, nemesis: C.deaths,
      death_streak: C.deaths, revive: C.support, revived: C.support, vehicle_kill: C.vehicles, login: C.session,
      session_start: C.session,
    };
    if (own[key]) return own[key];
    const k = key.toUpperCase();
    const has = (...w: string[]) => w.some((x) => k.includes(x));
    if (has("SESSION", "WELCOME", "LOGIN")) return C.session;
    if (has("DOUBLE", "TRIPLE", "QUAD", "PENTA", "MULTI", "MEGA", "ULTRA", "MONSTER", "HOLY")) return C.multikills;
    if (has("SPREE", "RAMPAGE", "DOMINAT", "UNSTOPPABLE", "GODLIKE", "STREAK", "LEGENDARY", "WICKED")) return C.streaks;
    if (has("MEDIC", "HEAL", "REVIV", "ENGINEER", "REPAIR", "RESUPPLY", "MIRACLE", "ZOMBIE", "SHADOW", "MALPRACTICE")) return C.support;
    if (has("VEHICLE", "TANK", "ROAD", "AIR", "SUNDERER", "GALAXY", "LIB", "HARASSER", "FLASH", "MAGRIDER", "VANGUARD", "PROWLER", "MBT")) return C.vehicles;
    if (has("DEATH", "DIED", "SUICIDE", "KILLED", "NEMESIS", "TEAM_KILLER")) return C.deaths;
    if (has("KILL", "HEAD", "KNIFE", "SNIPER", "PISTOL", "SIDEARM", "MAX_", "BOOMSTICK", "DAKKA", "BLOOD")) return C.kills;
    return C.other;
  }

  /** Base events, shown first with their key chain. */
  const BASE_LIST = ["kill", "headshot", "death", "teamkill", "teamkilled", "suicide", "session_start"];
  const baseEvents = $derived(
    BASE_LIST.map((k) => events.find((e) => e.key === k)).filter((e): e is EventInfo => e != null),
  );
  /** Keys tried for a base event, most specific first; the own key (standard tone) last. */
  function chainOf(key: string): string[] {
    return [...(BASE_KEYS[key] ?? []), ...(key === "kill" && v.kill_key ? [v.kill_key] : []), key];
  }
  /** Highest ranked active pack with a sound for a key (standard tones only as fallback). */
  function packWith(key: string): string | null {
    const p = sortedPacks.find(
      (x) => x.active && (x.sounds ?? []).includes(key) && (x.id !== STANDARD_ID || !realPackActive || v.builtin_fallback),
    );
    return p?.meta.name ?? null;
  }
  function playedText(key: string): string {
    const ts = events.find((e) => e.key === key)?.last_played;
    return ts ? t.voice.lastPlayed(ago(ts)) : t.voice.neverPlayed;
  }
  async function saveTrigger(ev: EventInfo, rules: TriggerRule[] | null) {
    const o = { ...v.trigger_overrides };
    if (rules == null) delete o[ev.key];
    else o[ev.key] = rules;
    config.voice.trigger_overrides = o;
    editingTrigger = null;
    await save();
    if (inTauri()) overview = await api.voiceOverview();
    notify(t.voice.trig.saved(ev.label));
  }

  const grouped = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const order = Object.values(t.voice.cats);
    const map = new Map<string, typeof events>();
    for (const ev of events) {
      if (BASE_LIST.includes(ev.key)) continue;
      if (q && !ev.label.toLowerCase().includes(q) && !ev.key.toLowerCase().includes(q)) continue;
      if (filter === "sound" && sourceOf(ev.key) === t.voice.srcNone) continue;
      if (filter === "notrigger" && ev.trigger) continue;
      if (filter === "all" && !showAll && ev.recursion && !ev.trigger && sourceOf(ev.key) === t.voice.srcNone) continue;
      const c = category(ev.key);
      map.set(c, [...(map.get(c) ?? []), ev]);
    }
    return [...map.entries()].sort((a, b) => order.indexOf(a[0]) - order.indexOf(b[0]));
  });

  async function importPath(path: string) {
    const r = await run(() => api.voiceImport(path));
    if (r) await afterImport(r);
  }
  const noActive = $derived(overview != null && !sortedPacks.some((p) => p.active));
  function onDrop(e: DragEvent) {
    // the file path comes from Tauri's drag-and-drop event (see onMount)
    e.preventDefault();
    dragOver = false;
  }
  onMount(() => {
    if (!inTauri()) return;
    const un = getCurrentWebview().onDragDropEvent((ev) => {
      if (ev.payload.type === "drop" && tab === "packs" && ev.payload.paths[0]) importPath(ev.payload.paths[0]);
      dragOver = ev.payload.type === "over" || ev.payload.type === "enter";
    });
    return () => {
      un.then((f) => f());
    };
  });
</script>

<h1>{t.voice.title}</h1>

{#if editing}
  <div class="row bar">
    <button onclick={() => (editing = null)}>{t.voice.back}</button>
    <h2>{t.voice.editTitle(editing.manifest.pack.name)}</h2>
    <div class="spacer"></div>
    <button class="primary" onclick={saveEditing}>{t.voice.save}</button>
  </div>
  <div class="card grid">
    <div class="row">
      <label class="field">{t.voice.fName} <input bind:value={editing.manifest.pack.name} /></label>
      <label class="field">{t.voice.fAuthor} <input bind:value={editing.manifest.pack.author} /></label>
      <label class="field">{t.voice.fVersion} <input bind:value={editing.manifest.pack.version} /></label>
      <label class="field">{t.voice.fLicense} <input bind:value={editing.manifest.pack.license} /></label>
    </div>
    <table>
      <thead><tr><th>{t.voice.colEvent}</th><th>{t.voice.colFiles}</th><th class="num">{t.voice.colPriority}</th><th class="num">{t.voice.colCooldown}</th><th class="num">{t.voice.colVolume}</th><th>{t.voice.colCondition}</th><th></th></tr></thead>
      <tbody>
        {#each events.filter((x) => showAll || !x.recursion || x.trigger || editing!.manifest.event.some((e) => e.event === x.key)) as ev (ev.key)}
          {@const key = ev.key}
          {@const entries = editing.manifest.event.filter((e) => e.event === key)}
          {#if entries.length === 0}
            <tr><td>{eventLabel(key)}</td><td class="muted">–</td><td></td><td></td><td></td><td></td>
              <td><button onclick={() => addFile(key)}>{t.voice.addFile}</button></td></tr>
          {/if}
          {#each entries as e, i (key + i)}
            <tr>
              <td>{eventLabel(key)}</td>
              <td>
                {#each e.files as f (f)}
                  <div class="file">
                    <button class="mini" title={t.voice.preview} onclick={() => run(() => api.voicePreviewFile(editing!.id, f))}>▶</button>
                    <span class="mono small">{f}</span>
                    <button class="mini danger" title={t.voice.remove} onclick={() => removeFile(e, f)}>✕</button>
                  </div>
                {/each}
              </td>
              <td class="num"><input class="n" type="number" min="0" max="100" placeholder={t.voice.defaultPlaceholder} bind:value={e.priority} /></td>
              <td class="num"><input class="n" type="number" min="0" step="50" bind:value={e.cooldown_ms} /></td>
              <td class="num"><input class="n" type="number" min="0" max="2" step="0.1" bind:value={e.volume} /></td>
              <td class="small muted">
                {#if e.when?.min_count != null}≥{e.when.min_count} {/if}{#if e.when?.max_count != null}≤{e.when.max_count} {/if}
                {#if e.when?.headshot != null}{t.voice.condHeadshot(e.when.headshot)}{/if}
                {#if e.when?.weapon_category?.length}{t.voice.condCategory(e.when.weapon_category.join(", "))}{/if}
                {#if e.when?.vehicle?.length}{t.voice.condVehicle(e.when.vehicle.join(", "))}{/if}
              </td>
              <td><button onclick={() => addFile(key)}>{t.voice.addFile}</button></td>
            </tr>
          {/each}
        {/each}
      </tbody>
    </table>
    <p class="hint">{t.voice.conditionsHint}</p>
  </div>
{:else if wizard}
  <div class="row bar">
    <button onclick={() => (wizard = null)}>{t.voice.cancel}</button>
    <h2>{t.voice.wizardTitle}</h2>
    <div class="spacer"></div>
    <button class="primary" onclick={finishWizard}>{t.voice.createPack}</button>
  </div>
  <div class="card grid">
    <p class="muted small">{t.voice.wizardFolder(wizard.folder)} {t.voice.wizardHint}</p>
    <div class="row">
      <label class="field">{t.voice.packName} <input bind:value={wizard.name} /></label>
      <label class="field">{t.voice.fAuthor} <input bind:value={wizard.author} /></label>
      <label class="field">{t.voice.fLicense} <input bind:value={wizard.license} /></label>
    </div>
    <table>
      <thead><tr><th></th><th>{t.voice.colFile}</th><th>{t.voice.colEvent}</th><th class="num">{t.voice.colCount}</th></tr></thead>
      <tbody>
        {#each wizard.mapping as m (m.file)}
          <tr>
            <td><button class="mini" onclick={() => run(() => api.voicePreviewForeign(wizard!.folder, m.file))}>▶</button></td>
            <td class="mono small">{m.file}</td>
            <td>
              <select bind:value={m.event}>
                <option value={null}>{t.voice.notUsed}</option>
                {#each events as ev (ev.key)}<option value={ev.key}>{ev.label}</option>{/each}
              </select>
            </td>
            <td class="num"><input class="n" type="number" min="0" bind:value={m.count} /></td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  <div class="tabs" role="tablist">
    {#each [{ id: "packs", l: t.voice.tabPacks }, { id: "events", l: t.voice.tabEvents }, { id: "playback", l: t.voice.tabPlayback }] as x (x.id)}
      <button role="tab" aria-selected={tab === x.id} onclick={() => (tab = x.id as Tab)}>{x.l}</button>
    {/each}
  </div>

  {#if tab === "packs"}
    <div
      class="dropzone"
      class:over={dragOver}
      role="region"
      aria-label={t.voice.dropHint}
      ondragover={(e) => { e.preventDefault(); dragOver = true; }}
      ondragleave={() => (dragOver = false)}
      ondrop={onDrop}
    >
      <div class="row">
        <Menu
          primary
          autoOpen={demoMenu === "import"}
          items={[
            { label: t.voice.importFileZip, onclick: () => importPack(false), disabled: !inTauri() },
            { label: t.voice.importFolder, onclick: () => importPack(true), disabled: !inTauri() },
            { separator: true },
            { label: t.voice.mapFolder, onclick: () => startWizard(), disabled: !inTauri() },
          ]}
        >
          {#snippet label()}⤓ {t.voice.import} ▾{/snippet}
        </Menu>
        <span class="muted small">{t.voice.dropHint}</span>
      </div>

      {#if noActive}
        <div class="banner warn" role="status">
          <span>{t.voice.noActivePack}</span>
          {#if sortedPacks.some((p) => p.id === STANDARD_ID)}
            <button class="mini" onclick={() => setActive(STANDARD_ID, true)}>{t.voice.activateStandard}</button>
          {/if}
        </div>
      {/if}

      {#if undoable}
        <div class="banner" role="status">
          <span>{t.voice.removed(undoable.name)}</span>
          <button class="mini" onclick={undoRemove}>{t.voice.undo}</button>
        </div>
      {/if}

      {#if importReport}
        <div class="hint">
          <strong>{t.voice.importReport(importReport.name)}</strong>:
          {t.voice.importSummary(formatLabel(importReport), importReport.events, importReport.stats.embedded, importReport.stats.loose, importReport.stats.background, importReport.stats.unreferenced)}
          · {importReport.warnings.length === 0 ? t.voice.noNotes : t.voice.notes(importReport.warnings.length)}
          {#if importReport.warnings.length}
            <ul class="small">{#each importReport.warnings as w (w)}<li>{w}</li>{/each}</ul>
          {/if}
          <button class="mini" onclick={() => (importReport = null)}>{t.voice.ok}</button>
        </div>
      {/if}

      <div class="packs">
        {#each sortedPacks as p, i (p.id)}
          <article class="pack card" class:active={p.active}>
            {#if p.background}<img class="bg" src={p.background} alt={t.voice.bgAlt} />{/if}
            <div class="ph">
              <div class="title">
                <strong>{p.meta.name}</strong>
                <span class="muted small">{p.meta.author || t.voice.unknownAuthor}</span>
              </div>
              <Menu
                ariaLabel={t.voice.moreActions}
                autoOpen={demoMenu === "pack" && i === sortedPacks.length - 1}
                items={[
                  { label: t.voice.edit, onclick: () => edit(p.id) },
                  { label: t.voice.exportZip, onclick: () => exportPack(p.id, "open") },
                  { label: t.voice.exportRecursion, title: t.voice.exportRecursionTitle, onclick: () => exportPack(p.id, "recursion") },
                  { separator: true },
                  p.id === STANDARD_ID
                    ? { label: t.voice.removePack, danger: true, disabled: true, title: t.voice.removeBuiltIn, onclick: () => {} }
                    : { label: t.voice.removePack, danger: true, onclick: () => askRemove(p) },
                ]}
              >
                {#snippet label()}…{/snippet}
              </Menu>
            </div>
            <div class="row small">
              <span class="badge">{formatBadge(p)}</span>
              <span class="muted num">{t.voice.soundCount(p.files)}</span>
              {#if p.warnings.length}<span class="badge warn" title={p.warnings.join("\n")}>{t.voice.warnings(p.warnings.length)}</span>{/if}
            </div>
            {#if p.active && p.id !== STANDARD_ID && p.suggested_kill_key && !v.kill_key}
              <div class="row small perkill" role="note">
                <span class="warn">{t.voice.noPerKill}</span>
                <select bind:value={killChoice[p.id]} aria-label={t.voice.killKey}>
                  {#each (p.sounds ?? []).filter((k) => /^[A-Z]/.test(k)).sort() as k (k)}<option value={k}>{k}</option>{/each}
                </select>
                <button class="mini" onclick={() => useKillKey(killChoice[p.id] ?? p.suggested_kill_key ?? "")}>{t.voice.useKey}</button>
              </div>
            {/if}
            <div class="row foot">
              <label class="check"><input type="checkbox" role="switch" checked={p.active} onchange={(e) => setActive(p.id, e.currentTarget.checked)} /> {t.voice.active}</label>
              <div class="spacer"></div>
              {#if p.active}
                <span class="badge">{t.voice.rank((p.position ?? 0) + 1)}</span>
                <button class="mini" aria-label={t.voice.up} onclick={() => move(p.id, -1)}>↑</button>
                <button class="mini" aria-label={t.voice.down} onclick={() => move(p.id, 1)}>↓</button>
              {/if}
            </div>
          </article>
        {/each}
      </div>
      {#each overview?.invalid ?? [] as [dir, err] (dir)}
        <div class="small bad">{t.voice.brokenPack(dir, err)}</div>
      {/each}
      <details class="section">
        <summary>{t.voice.createOwn}</summary>
        <form class="row" onsubmit={(e) => { e.preventDefault(); createPack(); }}>
          <input placeholder={t.voice.newPackName} bind:value={newPack.name} aria-label={t.voice.newPackName} />
          <input placeholder={t.voice.newPackAuthor} bind:value={newPack.author} aria-label={t.voice.newPackAuthor} />
          <button type="submit" disabled={!inTauri()}>{t.voice.create}</button>
        </form>
      </details>
    </div>
  {:else if tab === "events"}
    <div class="row filters">
      <input type="search" placeholder={t.voice.search} bind:value={query} aria-label={t.voice.search} />
      <div class="seg" role="radiogroup" aria-label={t.voice.filter}>
        {#each [{ id: "all", l: t.voice.fAll }, { id: "sound", l: t.voice.fWithSound }, { id: "notrigger", l: t.voice.fNoTrigger }] as x (x.id)}
          <button role="radio" aria-checked={filter === x.id} class:on={filter === x.id} onclick={() => (filter = x.id as Filter)}>{x.l}</button>
        {/each}
      </div>
    </div>
    <label class="check killkey"><input type="checkbox" bind:checked={v.builtin_fallback} onchange={() => save()} /> {t.voice.builtinFallback}</label>
    <p class="hint small">{t.voice.builtinFallbackHint}</p>
    <label class="field killkey">{t.voice.chainStyle}
      <select bind:value={v.chain_style} onchange={() => save()}>
        {#each Object.entries(t.voice.chainStyles) as [k, l] (k)}<option value={k}>{l}</option>{/each}
      </select>
    </label>
    <label class="field killkey">{t.voice.killKey}
      <select bind:value={v.kill_key} onchange={() => save()}>
        <option value="">{t.voice.killKeyNone}</option>
        {#each packKeys as k (k)}<option value={k}>{k}</option>{/each}
      </select>
      <span class="hint small">{t.voice.killKeyHint}</span>
    </label>
    {#snippet row(ev: EventInfo)}
      {@const key = ev.key}
      <div class="ev">
        <div class="evname">
          <strong>{ev.label}{#if v.trigger_overrides[key]}<span class="badge">{t.voice.customTrigger}</span>{/if}</strong>
          <span class="muted small">{SERVES[key] ? t.voice.servesBase(key, eventLabel(SERVES[key])) : (ev.trigger ?? t.voice.noTrigger)}</span>
        </div>
        <span class="small src">{sourceOf(key)}</span>
        <label class="check small"><input type="checkbox" role="switch" checked={!v.muted.includes(key)} onchange={(e) => toggleMute(key, !e.currentTarget.checked)} aria-label={t.voice.toggleAria(ev.label)} /> {t.voice.on}</label>
        <button class="mini" disabled={!inTauri()} onclick={() => run(() => api.voicePreviewEvent(key), undefined)} aria-label={t.voice.previewAria(ev.label)}>▶</button>
        <Menu
          ariaLabel={t.voice.moreActions}
          items={[
            { label: ev.rules.length ? t.voice.editTrigger : t.voice.chooseCondition, onclick: () => (editingTrigger = ev) },
            { label: t.voice.chooseCustom, disabled: !inTauri(), onclick: () => setOverride(key) },
            ...(v.overrides[key] ? [{ label: t.voice.clearCustom, danger: true, onclick: () => clearOverride(key) }] : []),
          ]}
        >
          {#snippet label()}…{/snippet}
        </Menu>
      </div>
    {/snippet}

    <section class="card cat">
      <h3>{t.voice.baseTitle}</h3>
      <p class="hint small">{t.voice.baseHint}</p>
      {#each baseEvents as ev (ev.key)}
        {@render row(ev)}
        <details class="chain">
          <summary class="small muted">{t.voice.chain}: {chainOf(ev.key).join(" → ")} · {playedText(ev.key)}</summary>
          <ol class="small">
            {#each chainOf(ev.key) as k (k)}
              {@const p = packWith(k)}
              <li>
                <span class="mono">{k}</span> – <span class:muted={!p}>{p ? t.voice.chainHas(p) : t.voice.chainNone}</span>
                · <span class="muted">{playedText(k)}</span>
              </li>
            {/each}
          </ol>
        </details>
      {/each}
    </section>

    <h3 class="subtitle">{t.voice.specialTitle}</h3>
    <p class="hint small">{t.voice.specialHint}</p>
    {#each grouped as [cat, list] (cat)}
      <section class="card cat">
        <h3>{cat} <span class="muted">({list.length})</span></h3>
        {#each list as ev (ev.key)}
          {@render row(ev)}
        {/each}
      </section>
    {/each}
    {#if editingTrigger}
      <TriggerEditor
        event={editingTrigger}
        custom={editingTrigger.key in v.trigger_overrides}
        onclose={() => (editingTrigger = null)}
        onsave={(rules) => saveTrigger(editingTrigger!, rules)}
      />
    {/if}
    <details class="section">
      <summary>{t.voice.advanced}</summary>
      <p class="muted small">{t.voice.triggersHint(overview?.triggers_file ?? "triggers.toml")}</p>
      {#each Object.entries(v.overrides) as [k, files] (k)}
        <div class="small"><span class="mono">{k}</span>: <span class="mono muted">{files[0]}</span></div>
      {/each}
    </details>
  {:else}
    <section class="card grid narrow">
      <label class="check"><input type="checkbox" bind:checked={v.enabled} onchange={() => save()} /> {t.voice.enabled}</label>
      <label class="field">{t.voice.volume(Math.round(v.volume * 100))}
        <input type="range" min="0" max="1" step="0.05" bind:value={v.volume} onchange={() => save()} />
      </label>
      <label class="field">{t.voice.device}
        <div class="row">
          <select bind:value={v.output_device} onchange={() => save()}>
            <option value="">{t.voice.defaultDevice}</option>
            {#each devices as d (d)}<option value={d}>{d}</option>{/each}
          </select>
          <button disabled={!inTauri()} onclick={() => run(() => api.voicePreviewEvent("kill"), t.voice.testPlayed)}>▶ {t.voice.testTone}</button>
        </div>
      </label>
      <div class="row">
        <label class="field">{t.voice.multikillWindow} <input class="n" type="number" min="1" max="15" bind:value={v.multikill_window_secs} onchange={() => save()} /></label>
        <label class="field">{t.voice.maxConcurrent}
          <select bind:value={v.max_concurrent} onchange={() => save()}>
            {#each [1, 2, 3, 4] as n (n)}<option value={n}>{n}</option>{/each}
          </select>
        </label>
      </div>
      <p class="hint">{t.voice.queueHint}</p>
    </section>
  {/if}
{/if}

<dialog bind:this={removeDialog} class="modal" aria-labelledby="rm-title" onclose={() => (removing = null)}>
  {#if removing}
    <h3 id="rm-title">{t.voice.removeTitle}</h3>
    <p>{t.voice.removeText(removing.pack.meta.name, removing.files, (removing.bytes / 1048576).toFixed(1))}</p>
    <div class="row end">
      <!-- svelte-ignore a11y_autofocus -->
      <button autofocus onclick={closeRemove}>{t.voice.cancelPlain}</button>
      <button class="danger-btn" onclick={confirmRemove}>{t.voice.removeConfirm}</button>
    </div>
  {/if}
</dialog>

<style>
  .killkey {
    max-width: 520px;
    margin-bottom: var(--s2);
  }
  .modal {
    max-width: 480px;
    padding: var(--s3);
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: 0 16px 48px rgb(0 0 0 / 45%);
  }
  .modal::backdrop {
    background: rgb(0 0 0 / 55%);
  }
  .modal h3 {
    margin-top: 0;
  }
  .end {
    justify-content: flex-end;
  }
  .danger-btn {
    background: var(--bad);
    color: #111;
    border-color: transparent;
    font-weight: 600;
  }
  .banner {
    display: flex;
    gap: var(--s2);
    align-items: center;
    padding: var(--s1) var(--s2);
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--panel-2);
  }
  .banner.warn {
    border-color: var(--warn);
  }
  .bar {
    margin-bottom: 0.8rem;
  }
  .bar h2 {
    margin: 0;
  }
  .spacer {
    flex: 1;
  }
  button.mini {
    padding: 0.15rem 0.45rem;
    font-size: 0.82rem;
  }
  input.n {
    width: 5.2rem;
  }
  .file {
    display: flex;
    gap: 0.35rem;
    align-items: center;
  }
  .badge.warn {
    color: var(--warn);
    border-color: var(--warn);
  }
  .perkill {
    flex-wrap: wrap;
    align-items: center;
  }
  .perkill .warn {
    color: var(--warn);
  }
  .tabs {
    margin-top: var(--s1);
  }
  .narrow {
    max-width: 720px;
  }
  .dropzone {
    display: grid;
    gap: var(--s2);
    border: 2px dashed transparent;
    border-radius: var(--radius);
  }
  .dropzone.over {
    border-color: var(--accent);
  }
  .packs {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: var(--s2);
  }
  .pack {
    display: grid;
    gap: var(--s1);
    align-content: start;
    position: relative;
  }
  .pack.active {
    border-color: var(--accent);
  }
  .pack .bg {
    width: 100%;
    max-height: 64px;
    object-fit: contain;
    border-radius: 6px;
    background: repeating-conic-gradient(#2a3140 0% 25%, #222833 0% 50%) 50% / 12px 12px;
  }
  .ph {
    display: flex;
    gap: var(--s1);
    align-items: start;
  }
  .ph .title {
    display: grid;
    flex: 1;
    min-width: 0;
  }
  .foot {
    border-top: 1px solid var(--line);
    padding-top: var(--s1);
  }
  .filters {
    margin-bottom: var(--s2);
  }
  .filters input[type="search"] {
    min-width: 260px;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 8px;
    overflow: hidden;
  }
  .seg button {
    border: none;
    border-radius: 0;
    background: transparent;
  }
  .seg button.on {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .cat {
    margin-bottom: var(--s2);
  }
  .subtitle {
    margin: var(--s3) 0 0;
  }
  .chain {
    padding: 0 0 var(--s1) var(--s2);
    border-bottom: 1px solid var(--line);
  }
  .chain summary {
    cursor: pointer;
    padding: 2px 0;
  }
  .chain ol {
    margin: var(--s1) 0 0;
    padding-left: 1.4rem;
  }
  .evname .badge {
    margin-left: var(--s1);
    font-size: 0.72rem;
    font-weight: 600;
    padding: 0 6px;
    border: 1px solid var(--accent);
    color: var(--accent);
    border-radius: 999px;
  }
  .ev {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(120px, 220px) auto auto auto;
    gap: var(--s1) var(--s2);
    align-items: center;
    padding: var(--s1) 0;
    border-bottom: 1px solid var(--line);
  }
  .ev:last-child {
    border-bottom: none;
  }
  .evname {
    display: grid;
    min-width: 0;
  }
  .src {
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
