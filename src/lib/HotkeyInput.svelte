<script lang="ts">
  // Hotkey recorder: focus it and press the combination (Tauri accelerator
  // format, e.g. "Ctrl+Shift+F9"). Backspace/Delete clears, Escape cancels.
  import { t } from "./i18n";

  let {
    value = $bindable(),
    label,
    others = [],
    onchange,
  }: { value: string; label: string; others?: string[]; onchange?: () => void } = $props();

  let recording = $state(false);
  const id = `hk-${Math.random().toString(36).slice(2, 8)}`;
  const conflict = $derived(!!value && others.some((o) => o && o.toLowerCase() === value.toLowerCase()));

  function keyName(e: KeyboardEvent): string | null {
    const k = e.key;
    if (["Control", "Shift", "Alt", "Meta", "AltGraph"].includes(k)) return null;
    if (/^F\d{1,2}$/.test(k)) return k;
    if (e.code.startsWith("Key")) return e.code.slice(3);
    if (e.code.startsWith("Digit")) return e.code.slice(5);
    if (e.code.startsWith("Numpad")) return e.code;
    const map: Record<string, string> = { " ": "Space", ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right" };
    return map[k] ?? (k.length === 1 ? k.toUpperCase() : k);
  }

  function onkeydown(e: KeyboardEvent) {
    if (!recording) {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        recording = true;
      }
      return;
    }
    e.preventDefault();
    if (e.key === "Escape") {
      recording = false;
      return;
    }
    if (e.key === "Backspace" || e.key === "Delete") {
      value = "";
      recording = false;
      onchange?.();
      return;
    }
    const k = keyName(e);
    if (!k) return;
    const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
    // single letters without a modifier would block typing everywhere
    if (mods.length === 0 && !/^F\d{1,2}$/.test(k)) return;
    value = [...mods, k].join("+");
    recording = false;
    onchange?.();
  }
</script>

<div class="hk">
  <label for={id} class="muted small">{label}</label>
  <button
    {id}
    type="button"
    class="rec"
    class:recording
    class:conflict
    aria-describedby={conflict ? `${id}-c` : undefined}
    onclick={() => (recording = !recording)}
    {onkeydown}
    onblur={() => (recording = false)}
  >
    {recording ? t.hotkey.press : value || t.hotkey.none}
  </button>
  {#if conflict}<span id="{id}-c" class="small warn">{t.hotkey.conflict}</span>{/if}
</div>

<style>
  .hk {
    display: grid;
    gap: 4px;
  }
  .rec {
    font-family: var(--mono);
    text-align: left;
    min-width: 12rem;
  }
  .rec.recording {
    border-color: var(--accent);
    color: var(--accent);
  }
  .rec.conflict {
    border-color: var(--warn);
  }
  .warn {
    color: var(--warn);
  }
</style>
