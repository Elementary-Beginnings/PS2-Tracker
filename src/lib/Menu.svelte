<script lang="ts" module>
  export type { MenuItem } from "./menu";
</script>

<script lang="ts">
  import { tick, type Snippet } from "svelte";
  import { placeMenu, type MenuItem } from "./menu";

  let {
    items,
    label,
    ariaLabel,
    primary = false,
    minWidth = 260,
    autoOpen = false,
  }: { items: MenuItem[]; label: Snippet; ariaLabel?: string; primary?: boolean; minWidth?: number; autoOpen?: boolean } =
    $props();

  let open = $state(false);
  let trigger = $state<HTMLButtonElement>();
  let menu = $state<HTMLDivElement>();
  let pos = $state({ left: 0, top: 0 });
  const id = `menu-${Math.random().toString(36).slice(2, 9)}`;

  function buttons(): HTMLButtonElement[] {
    return menu ? [...menu.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")] : [];
  }
  function place() {
    if (!trigger || !menu) return;
    const r = trigger.getBoundingClientRect();
    pos = placeMenu(r, { width: menu.offsetWidth, height: menu.offsetHeight }, { width: innerWidth, height: innerHeight });
  }
  async function show() {
    open = true;
    await tick();
    place();
    buttons()[0]?.focus();
  }
  function hide(focusTrigger = true) {
    open = false;
    if (focusTrigger) trigger?.focus();
  }
  function onKey(e: KeyboardEvent) {
    const b = buttons();
    const i = b.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "Escape") {
      e.preventDefault();
      hide();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      b[(i + 1) % b.length]?.focus();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      b[(i - 1 + b.length) % b.length]?.focus();
    } else if (e.key === "Home") {
      e.preventDefault();
      b[0]?.focus();
    } else if (e.key === "End") {
      e.preventDefault();
      b[b.length - 1]?.focus();
    } else if (e.key === "Tab") {
      hide(false);
    }
  }
  $effect(() => {
    // browser preview only (screenshots): open on load
    if (autoOpen) setTimeout(show, 50);
  });
  function onTriggerKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" && !open) {
      e.preventDefault();
      show();
    }
  }
  function outside(e: PointerEvent) {
    const t = e.target as Node;
    if (open && !menu?.contains(t) && !trigger?.contains(t)) hide(false);
  }
</script>

<svelte:window onpointerdown={outside} onresize={() => open && place()} onscroll={() => open && place()} />

<button
  bind:this={trigger}
  class="menu-trigger"
  class:primary
  aria-haspopup="menu"
  aria-expanded={open}
  aria-controls={id}
  aria-label={ariaLabel}
  onclick={() => (open ? hide() : show())}
  onkeydown={onTriggerKey}
>
  {@render label()}
</button>
{#if open}
  <div
    bind:this={menu}
    {id}
    class="menu-pop"
    role="menu"
    tabindex="-1"
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    style:min-width="{minWidth}px"
    onkeydown={onKey}
  >
    {#each items as it, i (i)}
      {#if "separator" in it}
        <div class="sep" role="separator"></div>
      {:else}
        <button
          role="menuitem"
          class:danger={it.danger}
          disabled={it.disabled}
          title={it.title}
          onclick={() => {
            hide();
            it.onclick();
          }}>{it.label}</button
        >
      {/if}
    {/each}
  </div>
{/if}

<style>
  .menu-trigger {
    padding: 4px var(--s1);
    border-radius: 8px;
  }
  .menu-trigger.primary {
    background: var(--accent);
    color: var(--accent-ink);
    font-weight: 600;
    padding: 6px 14px;
    border-color: transparent;
  }
  .menu-pop {
    position: fixed;
    z-index: 1000;
    display: grid;
    max-width: calc(100vw - 16px);
    padding: 4px;
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 35%);
  }
  .menu-pop button {
    text-align: left;
    border: none;
    background: transparent;
    white-space: nowrap;
  }
  .menu-pop button:hover:not(:disabled),
  .menu-pop button:focus-visible {
    background: var(--panel);
  }
  .menu-pop button.danger {
    color: var(--bad);
  }
  .menu-pop button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .sep {
    height: 1px;
    margin: 4px 2px;
    background: var(--line);
  }
</style>
