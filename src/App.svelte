<script lang="ts">
  import { onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow, type CloseRequestedEvent } from "@tauri-apps/api/window";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import {
    commands,
    type AnalyticsSnapshot,
    type TimerSnapshot,
    type Todo,
  } from "./lib/bindings";
  import {
    formatError,
    onDataChanged,
    onTimerAlert,
    onTimerSnapshot,
  } from "./lib/api";
  import Header from "./lib/components/Header.svelte";
  import TimerCard from "./lib/components/TimerCard.svelte";
  import TodoList from "./lib/components/TodoList.svelte";

  type Tab = "focus" | "insights";

  let todos = $state<Todo[]>([]);
  let timer = $state<TimerSnapshot | null>(null);
  let analytics = $state<AnalyticsSnapshot | null>(null);
  let tab = $state<Tab>("focus");
  let fatalError = $state<string | null>(null);
  let commandError = $state<string | null>(null);
  let listenerErrors = $state<string[]>([]);
  let loading = $state(true);

  let chime: HTMLAudioElement | null = null;
  let timerEventReceived = false;
  let dataEventReceived = false;
  let refreshRevision = 0;

  let activeTodoId = $derived(
    timer && (timer.state === "focusRunning" || timer.state === "focusPaused")
      ? (timer.attributedTodoId ?? null)
      : null,
  );
  let breakActive = $derived(
    timer?.phase === "shortBreak" || timer?.phase === "longBreak",
  );

  function showCommandError(error: unknown) {
    commandError = formatError(error);
  }

  function addListenerError(error: unknown) {
    const message = formatError(error);
    if (!listenerErrors.includes(message)) {
      listenerErrors = [...listenerErrors, message];
    }
  }

  async function bootstrap() {
    loading = true;
    try {
      const data = await commands.bootstrap();
      if (!timerEventReceived) {
        timer = data.timer;
      }
      if (!dataEventReceived) {
        todos = data.todos;
        analytics = data.analytics;
      }
      fatalError = null;
    } catch (error) {
      fatalError = formatError(error);
    } finally {
      loading = false;
    }
  }

  async function refresh() {
    const revision = ++refreshRevision;
    try {
      const data = await commands.refreshData();
      if (revision !== refreshRevision) return;
      todos = data.todos;
      analytics = data.analytics;
    } catch (error) {
      if (revision !== refreshRevision) return;
      showCommandError(error);
    }
  }

  async function runTimerCommand(command: () => Promise<TimerSnapshot>) {
    try {
      timer = await command();
      commandError = null;
    } catch (error) {
      showCommandError(error);
    }
  }

  async function createTodo(title: string, estimate: number | null) {
    try {
      await commands.createTodo(title, estimate);
      commandError = null;
      await refresh();
    } catch (error) {
      showCommandError(error);
    }
  }

  async function updateTodo(id: string, title: string, estimate: number | null) {
    try {
      await commands.updateTodo(id, title, estimate);
      commandError = null;
      await refresh();
    } catch (error) {
      showCommandError(error);
    }
  }

  async function toggleTodo(id: string, completed: boolean) {
    try {
      await commands.setTodoCompleted(id, completed);
      commandError = null;
      await refresh();
    } catch (error) {
      showCommandError(error);
    }
  }

  async function deleteTodo(id: string) {
    const target = todos.find((todo) => todo.id === id);
    try {
      const confirmed = await confirm(
        target
          ? `Delete "${target.title}"? This cannot be undone.`
          : "Delete this todo? This cannot be undone.",
        {
          title: "Delete todo?",
          kind: "warning",
          okLabel: "Delete",
          cancelLabel: "Cancel",
        },
      );
      if (!confirmed) return;
      await commands.deleteTodo(id);
      commandError = null;
      await refresh();
    } catch (error) {
      showCommandError(error);
    }
  }

  async function reorderTodos(ids: string[]) {
    const completedIds = todos
      .filter((todo) => todo.completedAt != null)
      .sort((left, right) => left.sortOrder - right.sortOrder)
      .map((todo) => todo.id);
    try {
      await commands.reorderTodos([...ids, ...completedIds]);
      commandError = null;
      await refresh();
    } catch (error) {
      showCommandError(error);
    }
  }

  function startFocus(todoId: string | null) {
    void runTimerCommand(() => commands.startFocus(todoId));
  }

  function pauseTimer() {
    void runTimerCommand(() => commands.pauseTimer());
  }

  function resumeTimer() {
    void runTimerCommand(() => commands.resumeTimer());
  }

  function endFocus() {
    void runTimerCommand(() => commands.endFocus());
  }

  function startBreak() {
    void runTimerCommand(() => commands.startBreak());
  }

  function skipBreak() {
    void runTimerCommand(() => commands.skipBreak());
  }

  function toggleMute(muted: boolean) {
    void runTimerCommand(() => commands.setMuted(muted));
  }

  function playChime() {
    try {
      chime ??= new Audio("/chime.wav");
      chime.currentTime = 0;
      void chime.play().catch((error) => {
        console.warn("Could not play the completion chime.", error);
      });
    } catch (error) {
      console.warn("Could not start the completion chime.", error);
    }
  }

  async function handleCloseRequested(event: CloseRequestedEvent) {
    const active =
      timer?.state === "focusRunning" ||
      timer?.state === "focusPaused" ||
      timer?.state === "breakRunning" ||
      timer?.state === "breakPaused";
    const hasWarning = Boolean(timer?.persistenceWarning);
    if (timer && !active && !hasWarning) return;

    event.preventDefault();

    try {
      const message = !timer
        ? fatalError
          ? "Pomodoro could not start. Close the app anyway?"
          : "Pomodoro is still starting. Close anyway?"
        : hasWarning
          ? "Some focus time has not been saved yet. Closing now may lose that progress."
          : "A focus or break session is still in progress. Close Pomodoro and save your progress?";
      const confirmed = await confirm(message, {
        title: "Close Pomodoro?",
        kind: "warning",
        okLabel: "Close",
        cancelLabel: "Keep working",
      });
      if (confirmed) await commands.closeApp();
    } catch (error) {
      showCommandError(error);
    }
  }

  onMount(() => {
    let disposed = false;
    const unlisteners: UnlistenFn[] = [];

    void (async () => {
      const registrations = await Promise.allSettled([
        onTimerSnapshot((snapshot) => {
          timerEventReceived = true;
          timer = snapshot;
        }),
        onDataChanged(() => {
          dataEventReceived = true;
          void refresh();
        }),
        onTimerAlert((alert) => {
          if (alert.audible) playChime();
        }),
      ]);

      for (const registration of registrations) {
        if (registration.status === "fulfilled") {
          if (disposed) {
            registration.value();
          } else {
            unlisteners.push(registration.value);
          }
        } else {
          addListenerError(registration.reason);
        }
      }

      if (!disposed) await bootstrap();
    })();

    void (async () => {
      try {
        const unlisten = await getCurrentWindow().onCloseRequested(handleCloseRequested);
        if (disposed) {
          unlisten();
        } else {
          unlisteners.push(unlisten);
        }
      } catch (error) {
        addListenerError(error);
      }
    })();

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  });
</script>

<div class="app-shell">
  <Header tab={tab} {timer} onTab={(next) => (tab = next)} onToggleMute={toggleMute} />

  <main class="app-main">
    {#if listenerErrors.length > 0}
      <div class="degraded-banner" role="status">
        <strong>Live updates degraded</strong>
        <span>{listenerErrors.join(" ")}</span>
      </div>
    {/if}

    {#if commandError}
      <div class="toast" role="alert">
        <span>{commandError}</span>
        <button type="button" class="toast-dismiss" onclick={() => (commandError = null)}>
          Dismiss
        </button>
      </div>
    {/if}

    {#if loading}
      <div class="center-state">
        <span class="spinner" aria-hidden="true"></span>
        <p>Loading your day…</p>
      </div>
    {:else if fatalError}
      <div class="center-state fatal-state">
        <p class="eyebrow">Startup problem</p>
        <h1>Pomodoro could not start</h1>
        <p class="fatal-message">{fatalError}</p>
        <button type="button" class="retry-button" onclick={() => void bootstrap()}>
          Try again
        </button>
      </div>
    {:else if !timer}
      <div class="center-state fatal-state">
        <p class="eyebrow">Startup problem</p>
        <h1>Pomodoro could not start</h1>
        <p class="fatal-message">The app data could not be loaded.</p>
        <button type="button" class="retry-button" onclick={() => void bootstrap()}>
          Try again
        </button>
      </div>
    {:else if tab === "focus"}
      <div class="focus-layout">
        <TimerCard
          {timer}
          onStartUnassigned={() => startFocus(null)}
          onPause={pauseTimer}
          onResume={resumeTimer}
          onEnd={endFocus}
          onStartBreak={startBreak}
          onSkipBreak={skipBreak}
        />

        <TodoList
          {todos}
          {activeTodoId}
          {breakActive}
          onPlay={startFocus}
          onCreate={createTodo}
          onUpdate={updateTodo}
          onToggle={toggleTodo}
          onDelete={deleteTodo}
          onReorder={reorderTodos}
        />
      </div>
    {:else}
      <section class="panel placeholder-panel insights-placeholder" aria-labelledby="insights-placeholder-title">
        <div class="placeholder-heading">
          <p class="eyebrow">Insights</p>
          <span class="placeholder-tag">Placeholder · Task 11</span>
        </div>
        <h2 id="insights-placeholder-title">See where your focus went</h2>
        <p>
          Daily totals and the rolling seven-day matrix will render here from Rust analytics.
        </p>
        <div class="placeholder-outline insights-outline" aria-hidden="true">
          <span class="placeholder-line wide"></span>
          <span class="placeholder-line"></span>
          <span class="placeholder-line"></span>
          <span class="placeholder-line short"></span>
        </div>
        <p class="placeholder-footnote">
          {analytics ? "Analytics are loaded and ready to render." : "No analytics loaded yet."}
        </p>
      </section>
    {/if}
  </main>
</div>

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    background:
      radial-gradient(circle at 15% -10%, color-mix(in srgb, var(--accent-focus) 8%, transparent), transparent 42%),
      linear-gradient(180deg, var(--bg), var(--bg-deep));
  }

  .app-main {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: clamp(1rem, 2.5vw, 2rem);
  }

  .focus-layout {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
    align-items: stretch;
    gap: 1.25rem;
    max-width: 64rem;
    margin: 0 auto;
  }

  .panel {
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    padding: 1.5rem;
  }

  .placeholder-panel {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    border-style: dashed;
    background: color-mix(in srgb, var(--surface) 72%, transparent);
    box-shadow: none;
  }

  .placeholder-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .eyebrow {
    margin: 0;
    color: var(--text-faint);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  .placeholder-tag {
    border: 1px dashed var(--border-strong);
    border-radius: 999px;
    color: var(--text-faint);
    font-size: 0.68rem;
    font-weight: 600;
    padding: 0.2rem 0.55rem;
    white-space: nowrap;
  }

  .placeholder-panel h2 {
    margin: 0;
    font-size: 1.2rem;
    letter-spacing: -0.01em;
  }

  .placeholder-panel p {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.86rem;
    line-height: 1.55;
  }

  .placeholder-outline {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    margin-top: 0.5rem;
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
    padding: 1rem;
  }

  .placeholder-line {
    height: 0.55rem;
    width: 72%;
    border-radius: 999px;
    background: var(--surface-muted);
  }

  .placeholder-line.wide {
    width: 92%;
  }

  .placeholder-line.short {
    width: 48%;
  }

  .insights-outline {
    margin-top: 0.75rem;
  }

  .placeholder-footnote {
    margin-top: auto;
    color: var(--text-faint) !important;
    font-size: 0.76rem !important;
  }

  .insights-placeholder {
    max-width: 64rem;
    margin: 0 auto;
  }

  .degraded-banner {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0.35rem 0.6rem;
    max-width: 64rem;
    margin: 0 auto 1rem;
    border: 1px solid color-mix(in srgb, var(--warning) 42%, var(--border));
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--warning) 11%, var(--surface));
    color: var(--warning-text);
    font-size: 0.82rem;
    line-height: 1.45;
    padding: 0.7rem 0.9rem;
  }

  .degraded-banner strong {
    font-size: 0.75rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .center-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.8rem;
    min-height: 60vh;
    color: var(--text-muted);
    text-align: center;
  }

  .spinner {
    width: 1.6rem;
    height: 1.6rem;
    border: 3px solid var(--surface-muted);
    border-top-color: var(--accent-focus);
    border-radius: 999px;
    animation: spin 750ms linear infinite;
  }

  .fatal-state {
    max-width: 30rem;
    margin: 0 auto;
  }

  .fatal-state h1 {
    margin: 0;
    color: var(--text);
    font-size: 1.5rem;
    letter-spacing: -0.02em;
  }

  .fatal-message {
    margin: 0;
    line-height: 1.6;
  }

  .retry-button {
    border: 0;
    border-radius: 999px;
    background: var(--accent-focus);
    color: #fff;
    font: inherit;
    font-size: 0.88rem;
    font-weight: 600;
    padding: 0.6rem 1.15rem;
    cursor: pointer;
  }

  .retry-button:hover {
    background: var(--accent-focus-strong);
  }

  .toast {
    position: fixed;
    top: 4.6rem;
    right: 1rem;
    z-index: 30;
    display: flex;
    align-items: flex-start;
    gap: 0.9rem;
    max-width: min(28rem, calc(100vw - 2rem));
    border: 1px solid color-mix(in srgb, var(--danger) 35%, var(--border));
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--danger) 8%, var(--surface));
    color: var(--text);
    box-shadow: var(--shadow-md);
    font-size: 0.85rem;
    line-height: 1.45;
    padding: 0.75rem 0.85rem;
  }

  .toast-dismiss {
    flex: none;
    border: 0;
    background: transparent;
    color: var(--danger);
    font: inherit;
    font-size: 0.78rem;
    font-weight: 700;
    cursor: pointer;
    padding: 0;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width: 820px) {
    .focus-layout {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
