<script lang="ts">
  import type { TimerSnapshot } from "../bindings";
  import { formatTimer, phaseLabel } from "../format";

  type Tab = "focus" | "insights";

  let {
    tab,
    timer,
    onTab = () => {},
    onToggleMute = () => {},
  }: {
    tab: Tab;
    timer: TimerSnapshot | null;
    onTab?: (tab: Tab) => void;
    onToggleMute?: (muted: boolean) => void;
  } = $props();
</script>

<header class="app-header">
  <div class="brand">
    <span class="brand-mark" aria-hidden="true"></span>
    <span class="brand-name">Pomodoro</span>
  </div>

  <nav class="tabs" aria-label="Views">
    <button
      type="button"
      class="tab"
      class:active={tab === "focus"}
      aria-current={tab === "focus" ? "page" : undefined}
      onclick={() => onTab("focus")}
    >
      Focus
    </button>
    <button
      type="button"
      class="tab"
      class:active={tab === "insights"}
      aria-current={tab === "insights" ? "page" : undefined}
      onclick={() => onTab("insights")}
    >
      Insights
    </button>
  </nav>

  <div class="header-status">
    {#if timer}
      <div class="compact-timer" aria-label="Timer status">
        <span class="compact-phase">{phaseLabel(timer)}</span>
        <span class="compact-time">{formatTimer(timer.remainingMs)}</span>
        {#if timer.persistenceWarning}
          <span class="warning-dot" title="Saving is delayed"></span>
        {/if}
      </div>
      <button
        type="button"
        class="mute-toggle"
        class:is-muted={timer.muted}
        aria-pressed={timer.muted}
        onclick={() => onToggleMute(!timer.muted)}
      >
        {timer.muted ? "Unmute" : "Mute"}
      </button>
    {:else}
      <span class="compact-placeholder">Loading…</span>
    {/if}
  </div>
</header>

<style>
  .app-header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 1rem;
    padding: 0.85rem 1.5rem;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 88%, transparent);
    backdrop-filter: blur(12px);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    min-width: 0;
  }

  .brand-mark {
    width: 0.9rem;
    height: 0.9rem;
    border-radius: 999px;
    background: linear-gradient(135deg, var(--accent-focus), var(--accent-focus-strong));
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--accent-focus) 16%, transparent);
  }

  .brand-name {
    font-size: 0.95rem;
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .tabs {
    display: flex;
    gap: 0.25rem;
    padding: 0.25rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-muted);
  }

  .tab {
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.85rem;
    font-weight: 600;
    padding: 0.4rem 0.95rem;
    cursor: pointer;
    transition: background 150ms ease, color 150ms ease;
  }

  .tab:hover {
    color: var(--text);
  }

  .tab.active {
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-sm);
  }

  .header-status {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.9rem;
    min-width: 0;
  }

  .compact-timer {
    display: flex;
    align-items: baseline;
    gap: 0.45rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .compact-phase {
    color: var(--text-muted);
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.07em;
  }

  .compact-time {
    font-size: 0.95rem;
    font-weight: 700;
  }

  .warning-dot {
    align-self: center;
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 999px;
    background: var(--warning);
  }

  .compact-placeholder {
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .mute-toggle {
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text-muted);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
    padding: 0.4rem 0.8rem;
    cursor: pointer;
    transition: border-color 150ms ease, color 150ms ease;
  }

  .mute-toggle:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }

  .mute-toggle.is-muted {
    border-color: color-mix(in srgb, var(--warning) 45%, var(--border));
    color: var(--warning-text);
    background: color-mix(in srgb, var(--warning) 10%, var(--surface));
  }

  @media (max-width: 620px) {
    .app-header {
      grid-template-columns: 1fr auto;
      gap: 0.6rem;
      padding: 0.75rem 1rem;
    }

    .brand-name {
      display: none;
    }

    .tabs {
      order: 3;
      grid-column: 1 / -1;
      justify-self: center;
    }
  }
</style>
