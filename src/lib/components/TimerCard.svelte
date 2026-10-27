<script lang="ts">
  import type { TimerSnapshot } from "../bindings";
  import { formatTimer, phaseLabel } from "../format";

  type TimerCardProps = {
    timer: TimerSnapshot;
    onStartUnassigned?: () => void;
    onPause?: () => void;
    onResume?: () => void;
    onEnd?: () => void;
    onStartBreak?: () => void;
    onSkipBreak?: () => void;
  };

  let {
    timer,
    onStartUnassigned = () => {},
    onPause = () => {},
    onResume = () => {},
    onEnd = () => {},
    onStartBreak = () => {},
    onSkipBreak = () => {},
  }: TimerCardProps = $props();

  const radius = 92;
  const circumference = 2 * Math.PI * radius;

  let total = $derived(timer.totalMs || 1);
  let progress = $derived(Math.max(0, Math.min(1, timer.remainingMs / total)));
  let dashOffset = $derived(circumference * (1 - progress));
  let isBreak = $derived(timer.phase === "shortBreak" || timer.phase === "longBreak");
  let attributedTitle = $derived(timer.attributedTodoTitle ?? "Unassigned");

  let stateHint = $derived.by(() => {
    switch (timer.state) {
      case "idle":
        return "Pick a todo from the list, or start a session without one.";
      case "focusRunning":
        return "Stay with one thing. The break comes when the focus is complete.";
      case "focusPaused":
        return "Paused. Your active focus time is safe.";
      case "breakPending":
        return "Focus complete. Start the break when you are ready.";
      case "breakRunning":
        return "Step away for a moment. The break is counting down.";
      case "breakPaused":
        return "Break paused. Resume when you are ready.";
      default:
        return "";
    }
  });

  let attributionLabel = $derived(
    timer.state === "idle" ? "No todo selected" : isBreak ? "On break from" : "Focusing on",
  );
</script>

<section class="timer-card" class:is-break={isBreak} aria-labelledby="timer-phase">
  <div class="card-top">
    <span class="state-pill" class:is-break={isBreak}>
      {timer.state === "idle" ? "Ready" : isBreak ? "Break" : "Focus"}
    </span>
    <div class="card-meta">
      {#if timer.muted}
        <span class="muted-badge">Muted</span>
      {/if}
      <span class="session-count">
        {timer.completedFocusCount}
        {timer.completedFocusCount === 1 ? "session" : "sessions"}
      </span>
    </div>
  </div>

  <div
    class="timer-dial"
    role="img"
    aria-label={`${phaseLabel(timer)} timer, ${formatTimer(timer.remainingMs)} remaining`}
  >
    <svg class="ring" viewBox="0 0 200 200" aria-hidden="true">
      <circle class="ring-track" cx="100" cy="100" r={radius}></circle>
      <circle
        class="ring-progress"
        class:is-break={isBreak}
        cx="100"
        cy="100"
        r={radius}
        stroke-dasharray={circumference}
        stroke-dashoffset={dashOffset}
      ></circle>
    </svg>
    <div class="dial-content">
      <span class="phase-label" id="timer-phase">{phaseLabel(timer)}</span>
      <span class="remaining-time">{formatTimer(timer.remainingMs)}</span>
      <span class="phase-detail">{formatTimer(timer.totalMs)} {isBreak ? "break" : "focus"}</span>
    </div>
  </div>

  <div class="attribution">
    <span class="attribution-label">{attributionLabel}</span>
    <span class="attribution-title" class:unassigned={!timer.attributedTodoTitle}>
      {attributedTitle}
    </span>
  </div>

  <p class="state-hint">{stateHint}</p>

  {#if timer.persistenceWarning}
    <div class="warning-banner" role="status">
      <strong>Save warning</strong>
      <span>{timer.persistenceWarning}</span>
    </div>
  {/if}

  <div class="controls">
    {#if timer.state === "idle"}
      <button type="button" class="btn primary" onclick={onStartUnassigned}>Start unassigned</button>
    {:else if timer.state === "focusRunning"}
      <button type="button" class="btn primary" onclick={onPause}>Pause</button>
      <button type="button" class="btn danger" onclick={onEnd}>End focus</button>
    {:else if timer.state === "focusPaused"}
      <button type="button" class="btn primary" onclick={onResume}>Resume</button>
      <button type="button" class="btn danger" onclick={onEnd}>End focus</button>
    {:else if timer.state === "breakPending"}
      <button type="button" class="btn primary break" onclick={onStartBreak}>Start break</button>
      <button type="button" class="btn secondary" onclick={onSkipBreak}>Skip break</button>
      <button type="button" class="btn secondary" onclick={onStartUnassigned}>
        Start unassigned
      </button>
    {:else if timer.state === "breakRunning"}
      <button type="button" class="btn primary break" onclick={onPause}>Pause</button>
      <button type="button" class="btn secondary" onclick={onSkipBreak}>Skip break</button>
    {:else if timer.state === "breakPaused"}
      <button type="button" class="btn primary break" onclick={onResume}>Resume</button>
      <button type="button" class="btn secondary" onclick={onSkipBreak}>Skip break</button>
    {/if}
  </div>
</section>

<style>
  .timer-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
    padding: 1.5rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background:
      radial-gradient(circle at 50% 0%, color-mix(in srgb, var(--accent-focus) 9%, transparent), transparent 65%),
      var(--surface);
    box-shadow: var(--shadow-md);
  }

  .timer-card.is-break {
    background:
      radial-gradient(circle at 50% 0%, color-mix(in srgb, var(--accent-break) 10%, transparent), transparent 65%),
      var(--surface);
  }

  .card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    gap: 1rem;
  }

  .state-pill {
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent-focus) 12%, transparent);
    color: var(--accent-focus-strong);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.09em;
    padding: 0.3rem 0.65rem;
    text-transform: uppercase;
  }

  .state-pill.is-break {
    background: color-mix(in srgb, var(--accent-break) 14%, transparent);
    color: var(--accent-break-strong);
  }

  .card-meta {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    color: var(--text-muted);
    font-size: 0.78rem;
    white-space: nowrap;
  }

  .muted-badge {
    border: 1px solid color-mix(in srgb, var(--warning) 40%, var(--border));
    border-radius: 999px;
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    color: var(--warning-text);
    font-weight: 600;
    padding: 0.18rem 0.5rem;
  }

  .session-count {
    font-variant-numeric: tabular-nums;
  }

  .timer-dial {
    position: relative;
    width: min(15rem, 72vw);
    aspect-ratio: 1;
    display: grid;
    place-items: center;
  }

  .ring {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .ring-track {
    fill: none;
    stroke: var(--ring-track);
    stroke-width: 9;
  }

  .ring-progress {
    fill: none;
    stroke: var(--accent-focus);
    stroke-width: 9;
    stroke-linecap: round;
    transform: rotate(-90deg);
    transform-origin: 100px 100px;
    transition: stroke-dashoffset 500ms linear, stroke 250ms ease;
  }

  .ring-progress.is-break {
    stroke: var(--accent-break);
  }

  .dial-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.15rem;
    text-align: center;
    padding: 1rem;
  }

  .phase-label {
    color: var(--text-muted);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
  }

  .remaining-time {
    font-size: clamp(2.4rem, 7vw, 3.4rem);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.04em;
    line-height: 1.05;
  }

  .phase-detail {
    color: var(--text-faint);
    font-size: 0.72rem;
  }

  .attribution {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.15rem;
    text-align: center;
    max-width: 100%;
  }

  .attribution-label {
    color: var(--text-faint);
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .attribution-title {
    font-size: 1rem;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .attribution-title.unassigned {
    color: var(--text-muted);
    font-weight: 500;
  }

  .state-hint {
    margin: 0;
    max-width: 28rem;
    color: var(--text-muted);
    font-size: 0.82rem;
    line-height: 1.5;
    text-align: center;
  }

  .warning-banner {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    width: 100%;
    border: 1px solid color-mix(in srgb, var(--warning) 42%, var(--border));
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--warning) 11%, var(--surface));
    color: var(--warning-text);
    font-size: 0.8rem;
    padding: 0.65rem 0.8rem;
  }

  .warning-banner strong {
    font-size: 0.75rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .controls {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 0.6rem;
    width: 100%;
  }

  .btn {
    min-width: 7rem;
    border: 1px solid transparent;
    border-radius: 999px;
    font: inherit;
    font-size: 0.88rem;
    font-weight: 600;
    padding: 0.62rem 1.15rem;
    cursor: pointer;
    transition: background 150ms ease, border-color 150ms ease, color 150ms ease, transform 150ms ease;
  }

  .btn:active {
    transform: translateY(1px);
  }

  .btn.primary {
    background: var(--accent-focus);
    color: #fff;
    box-shadow: 0 6px 16px color-mix(in srgb, var(--accent-focus) 24%, transparent);
  }

  .btn.primary:hover {
    background: var(--accent-focus-strong);
  }

  .btn.primary.break {
    background: var(--accent-break);
    box-shadow: 0 6px 16px color-mix(in srgb, var(--accent-break) 24%, transparent);
  }

  .btn.primary.break:hover {
    background: var(--accent-break-strong);
  }

  .btn.secondary {
    border-color: var(--border-strong);
    background: var(--surface);
    color: var(--text);
  }

  .btn.secondary:hover {
    background: var(--surface-muted);
  }

  .btn.danger {
    border-color: color-mix(in srgb, var(--danger) 34%, var(--border));
    background: transparent;
    color: var(--danger);
  }

  .btn.danger:hover {
    background: color-mix(in srgb, var(--danger) 9%, var(--surface));
  }
</style>
