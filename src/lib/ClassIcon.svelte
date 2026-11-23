<!--
  Class symbol for kill feed, squad and live page. Own, plain shapes (CC0,
  drawn for this project, no game graphics): scope = Infiltrator, double
  chevron = Light Assault, cross = Combat Medic, nut = Engineer, shield =
  Heavy Assault, block with visor = MAX. Unknown class: an en dash of the same
  width, so rows keep their layout. `short`: the abbreviation instead of the
  symbol (INF, LA, MED, ENG, HA, MAX).
  When the official icon is cached (loaded at runtime from Census by the
  backend, never part of the repo) it is shown instead of the own symbol.
-->
<script lang="ts">
  import { t } from "./i18n";
  import type { ClassId } from "./types";
  import { classIcons, loadClassIcons } from "./classIcons";

  loadClassIcons();

  let { cls, short = false, title = "" }: { cls?: ClassId | null; short?: boolean; title?: string } = $props();
  const label = $derived(cls ? t.classes.names[cls] : t.classes.unknown);
  const tip = $derived(title ? `${title}: ${label}` : label);
</script>

{#if !cls}
  <span class="ci none" title={tip} aria-label={tip}>–</span>
{:else if short}
  <span class="ci txt" title={tip} aria-label={tip}>{t.classes.short[cls]}</span>
{:else if $classIcons[cls]}
  <!-- the icon (white on transparent) as a mask, so it takes the text colour in every theme -->
  <span class="ci img" role="img" aria-label={tip} title={tip} style:--icon={`url("${$classIcons[cls]}")`}></span>
{:else}
  <svg class="ci" viewBox="0 0 16 16" role="img" aria-label={tip}>
    <title>{tip}</title>
    {#if cls === "infiltrator"}
      <circle cx="8" cy="8" r="5.6" fill="none" stroke="currentColor" stroke-width="1.8" />
      <circle cx="8" cy="8" r="1.7" fill="currentColor" />
    {:else if cls === "light_assault"}
      <path d="M2.5 8.5 8 3l5.5 5.5M2.5 13.5 8 8l5.5 5.5" fill="none" stroke="currentColor" stroke-width="2" />
    {:else if cls === "medic"}
      <path d="M6.3 1.5h3.4v4.8h4.8v3.4H9.7v4.8H6.3V9.7H1.5V6.3h4.8z" fill="currentColor" />
    {:else if cls === "engineer"}
      <path
        d="M8 1.2l5.9 3.4v6.8L8 14.8l-5.9-3.4V4.6zM8 5.4a2.6 2.6 0 1 0 0 5.2 2.6 2.6 0 0 0 0-5.2z"
        fill="currentColor"
        fill-rule="evenodd"
      />
    {:else if cls === "heavy_assault"}
      <path d="M8 1.2 14 3.6v4.6c0 3.5-2.6 5.9-6 6.8-3.4-.9-6-3.3-6-6.8V3.6z" fill="currentColor" />
    {:else}
      <path d="M2.5 2.5h11v11h-11zM5 6v2.4h6V6z" fill="currentColor" fill-rule="evenodd" />
    {/if}
  </svg>
{/if}

<style>
  .ci {
    display: inline-block;
    width: 1em;
    height: 1em;
    flex: none;
    vertical-align: -0.12em;
  }
  .img {
    width: 1.15em;
    height: 1.15em;
    vertical-align: -0.2em;
    background: currentColor;
    -webkit-mask: var(--icon) center / contain no-repeat;
    mask: var(--icon) center / contain no-repeat;
  }
  .none {
    text-align: center;
    opacity: 0.6;
  }
  .txt {
    width: auto;
    min-width: 2.2em;
    font-size: 0.85em;
    font-weight: 700;
    letter-spacing: 0.02em;
  }
</style>
