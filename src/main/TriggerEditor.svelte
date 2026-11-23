<script lang="ts">
  // Edits the trigger condition(s) of one sound event: a template (the base fact
  // from the ESS data) plus parameters. Saved in the config, no TOML needed.
  import { untrack } from "svelte";
  import { t } from "../lib/i18n";
  import { COUNTED_FACTS, FACTS, KILL_FACTS, describeRule } from "../lib/triggers";
  import type { EventInfo, TriggerRule } from "../lib/types";

  let {
    event,
    custom,
    onsave,
    onclose,
  }: {
    event: EventInfo;
    /** true when the rules come from the Events page (not the defaults). */
    custom: boolean;
    onsave: (rules: TriggerRule[] | null) => void;
    onclose: () => void;
  } = $props();

  const MAX = 8;
  const blank = (): TriggerRule => ({ event: event.key, on: "kill", description: "" });
  // a working copy; lists are edited as comma-separated text
  // the dialog is created per event, so the initial value is all it needs
  let rules = $state<(TriggerRule & { wc: string; veh: string })[]>(
    untrack(() =>
      (event.rules.length ? event.rules : [blank()]).map((r) => ({
        ...structuredClone($state.snapshot(r)),
        wc: (r.weapon_category ?? []).join(", "),
        veh: (r.vehicle ?? []).join(", "),
      })),
    ),
  );
  let dialog = $state<HTMLDialogElement>();
  $effect(() => {
    dialog?.showModal();
  });

  const list = (s: string) =>
    s
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean)
      .slice(0, 32);
  const num = (v: unknown): number | undefined => {
    const n = typeof v === "number" ? v : parseInt(String(v ?? ""), 10);
    return Number.isFinite(n) && n >= 0 ? n : undefined;
  };
  const tri = (v: boolean | undefined | null) => (v == null ? "" : v ? "yes" : "no");
  const fromTri = (s: string) => (s === "" ? undefined : s === "yes");

  function clean(r: (typeof rules)[number]): TriggerRule {
    const out: TriggerRule = { event: event.key, on: r.on, description: "" };
    const counted = COUNTED_FACTS.includes(r.on);
    const killish = KILL_FACTS.includes(r.on);
    if (counted && num(r.count) != null) out.count = num(r.count);
    if (counted && num(r.min_count) != null) out.min_count = num(r.min_count);
    if (killish && r.headshot != null) out.headshot = r.headshot;
    if (killish && r.roadkill != null) out.roadkill = r.roadkill;
    if (killish && list(r.wc).length) out.weapon_category = list(r.wc);
    if ((killish || r.on === "vehicle_kill") && list(r.veh).length) out.vehicle = list(r.veh);
    if (num(r.session_count)) out.session_count = num(r.session_count);
    if (num(r.priority) != null) out.priority = Math.min(100, num(r.priority) ?? 0);
    out.description = r.description?.trim() || describeRule(out);
    return out;
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    onsave(rules.map(clean));
  }
</script>

<dialog bind:this={dialog} class="modal wide" aria-labelledby="tr-title" onclose={onclose}>
  <form onsubmit={submit}>
    <h3 id="tr-title">{t.voice.trig.title(event.label)}</h3>
    <p class="muted small">{t.voice.trig.intro}</p>
    {#each rules as r, i (i)}
      <fieldset class="rule">
        <legend>{t.voice.trig.rule(i + 1)}</legend>
        <label class="field">{t.voice.trig.template}
          <select bind:value={r.on}>
            {#each FACTS as f (f)}<option value={f}>{t.voice.facts[f] ?? f}</option>{/each}
          </select>
        </label>
        {#if COUNTED_FACTS.includes(r.on)}
          <div class="row">
            <label class="field">{t.voice.trig.count}<input class="n" type="number" min="1" max="1000" bind:value={r.count} /></label>
            <label class="field">{t.voice.trig.minCount}<input class="n" type="number" min="1" max="1000" bind:value={r.min_count} /></label>
          </div>
        {/if}
        {#if KILL_FACTS.includes(r.on)}
          <div class="row">
            <label class="field">{t.voice.trig.headshot}
              <select value={tri(r.headshot)} onchange={(e) => (r.headshot = fromTri(e.currentTarget.value))}>
                <option value="">{t.voice.trig.any}</option><option value="yes">{t.voice.trig.yes}</option><option value="no">{t.voice.trig.no}</option>
              </select>
            </label>
            <label class="field">{t.voice.trig.roadkill}
              <select value={tri(r.roadkill)} onchange={(e) => (r.roadkill = fromTri(e.currentTarget.value))}>
                <option value="">{t.voice.trig.any}</option><option value="yes">{t.voice.trig.yes}</option><option value="no">{t.voice.trig.no}</option>
              </select>
            </label>
          </div>
          <label class="field">{t.voice.trig.weaponCategory}<input bind:value={r.wc} placeholder="Sniper Rifle, Knife" /></label>
        {/if}
        {#if KILL_FACTS.includes(r.on) || r.on === "vehicle_kill"}
          <label class="field">{t.voice.trig.vehicle}<input bind:value={r.veh} placeholder="Reaver, Mosquito, Scythe" /></label>
        {/if}
        <div class="row">
          <label class="field">{t.voice.trig.sessionCount}<input class="n" type="number" min="1" max="1000" bind:value={r.session_count} /></label>
          <label class="field">{t.voice.trig.priority}<input class="n" type="number" min="0" max="100" bind:value={r.priority} placeholder={String(event.priority)} /></label>
        </div>
        <label class="field">{t.voice.trig.description}<input bind:value={r.description} maxlength="200" placeholder={describeRule(clean(r))} /></label>
        {#if rules.length > 1}
          <button type="button" class="mini" onclick={() => rules.splice(i, 1)}>{t.voice.trig.removeRule}</button>
        {/if}
      </fieldset>
    {/each}
    <div class="row">
      <button type="button" disabled={rules.length >= MAX} onclick={() => rules.push({ ...blank(), wc: "", veh: "" })}>+ {t.voice.trig.addRule}</button>
      <span class="hint small">{t.voice.trig.orHint}</span>
    </div>
    <div class="row end">
      {#if custom}
        <button type="button" onclick={() => onsave(null)} title={t.voice.trig.resetTitle}>{t.voice.trig.reset}</button>
      {/if}
      <button type="button" onclick={() => onsave([])} title={t.voice.trig.disableTitle}>{t.voice.trig.disable}</button>
      <span class="spacer"></span>
      <button type="button" onclick={() => dialog?.close()}>{t.voice.cancelPlain}</button>
      <button type="submit" class="primary">{t.voice.trig.save}</button>
    </div>
  </form>
</dialog>

<style>
  .modal {
    width: min(620px, 92vw);
    max-height: 88vh;
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
  h3 {
    margin-top: 0;
  }
  form {
    display: grid;
    gap: var(--s2);
  }
  .rule {
    display: grid;
    gap: var(--s1);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: var(--s1) var(--s2) var(--s2);
  }
  .row {
    display: flex;
    gap: var(--s2);
    align-items: end;
    flex-wrap: wrap;
  }
  .end {
    align-items: center;
  }
  .spacer {
    flex: 1;
  }
  input.n {
    width: 7rem;
  }
  button.mini {
    justify-self: start;
    padding: 0.15rem 0.45rem;
    font-size: 0.82rem;
  }
</style>
