import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import type {
  AnalyticsSnapshot,
  Bootstrap,
  DataSnapshot,
  TimerAlert,
  TimerSnapshot,
} from "./lib/bindings";

const state = vi.hoisted(() => {
  type EventHandler = (event: { payload: unknown }) => void;
  type CloseHandler = (event: { preventDefault: () => void }) => void | Promise<void>;

  return {
    listeners: new Map<string, EventHandler>(),
    unlisteners: [] as Array<ReturnType<typeof vi.fn>>,
    closeHandler: null as CloseHandler | null,
    failEvents: new Set<string>(),
    bootstrap: vi.fn(),
    refreshData: vi.fn(),
    startFocus: vi.fn(),
    pauseTimer: vi.fn(),
    resumeTimer: vi.fn(),
    endFocus: vi.fn(),
    startBreak: vi.fn(),
    skipBreak: vi.fn(),
    setMuted: vi.fn(),
    closeApp: vi.fn(),
    confirm: vi.fn(),
    play: vi.fn(() => Promise.resolve()),
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event: string, handler: (event: { payload: unknown }) => void) => {
    if (state.failEvents.has(event)) {
      return Promise.reject(new Error(`${event} subscription failed`));
    }
    state.listeners.set(event, handler);
    const unlisten = vi.fn(() => {
      state.listeners.delete(event);
    });
    state.unlisteners.push(unlisten);
    return Promise.resolve(unlisten);
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: vi.fn((handler: (event: { preventDefault: () => void }) => void) => {
      state.closeHandler = handler;
      const unlisten = vi.fn(() => {
        state.closeHandler = null;
      });
      state.unlisteners.push(unlisten);
      return Promise.resolve(unlisten);
    }),
  }),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: state.confirm,
}));

vi.mock("./lib/bindings", () => ({
  commands: {
    bootstrap: state.bootstrap,
    refreshData: state.refreshData,
    startFocus: state.startFocus,
    pauseTimer: state.pauseTimer,
    resumeTimer: state.resumeTimer,
    endFocus: state.endFocus,
    startBreak: state.startBreak,
    skipBreak: state.skipBreak,
    setMuted: state.setMuted,
    closeApp: state.closeApp,
  },
}));

class AudioMock {
  currentTime = 0;
  play = state.play;
}

const timerSnapshot = (overrides: Partial<TimerSnapshot> = {}): TimerSnapshot => ({
  state: "idle",
  phase: null,
  remainingMs: 1_500_000,
  totalMs: 1_500_000,
  attributedTodoId: null,
  attributedTodoTitle: null,
  completedFocusCount: 0,
  muted: false,
  persistenceWarning: null,
  ...overrides,
});

const analytics: AnalyticsSnapshot = {
  today: { date: "2026-10-26", totalMs: 0, rows: [] },
  week: { days: [], rows: [], dayTotals: [], totalMs: 0 },
};

const bootstrapData = (timer: TimerSnapshot = timerSnapshot()): Bootstrap => ({
  todos: [],
  timer,
  analytics,
});

const dataSnapshot: DataSnapshot = { todos: [], analytics };

const alert = (overrides: Partial<TimerAlert> = {}): TimerAlert => ({
  kind: "focusComplete",
  audible: true,
  title: "Focus complete",
  body: "Take a break.",
  ...overrides,
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

async function waitForCloseHandler() {
  await waitFor(() => expect(state.closeHandler).not.toBeNull());
  return state.closeHandler!;
}

describe("App", () => {
  beforeEach(() => {
    state.listeners.clear();
    state.unlisteners.length = 0;
    state.closeHandler = null;
    state.failEvents.clear();
    vi.clearAllMocks();

    state.bootstrap.mockResolvedValue(bootstrapData());
    state.refreshData.mockResolvedValue(dataSnapshot);
    state.startFocus.mockResolvedValue(timerSnapshot());
    state.pauseTimer.mockResolvedValue(timerSnapshot());
    state.resumeTimer.mockResolvedValue(timerSnapshot());
    state.endFocus.mockResolvedValue(timerSnapshot());
    state.startBreak.mockResolvedValue(timerSnapshot());
    state.skipBreak.mockResolvedValue(timerSnapshot());
    state.setMuted.mockResolvedValue(timerSnapshot());
    state.closeApp.mockResolvedValue(null);
    state.confirm.mockResolvedValue(false);
    vi.stubGlobal("Audio", AudioMock);
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it("does not play a chime when the alert is not audible", async () => {
    state.bootstrap.mockResolvedValue(
      bootstrapData(timerSnapshot({ muted: true })),
    );
    const { getByText } = render(App);
    await waitFor(() => expect(getByText("Start unassigned")).toBeTruthy());

    state.listeners.get("timer-alert")!({
      payload: alert({ audible: false }),
    });

    expect(state.play).not.toHaveBeenCalled();
  });

  it("plays a chime when the alert is audible", async () => {
    const { getByText } = render(App);
    await waitFor(() => expect(getByText("Start unassigned")).toBeTruthy());

    state.listeners.get("timer-alert")!({ payload: alert({ audible: true }) });

    await waitFor(() => expect(state.play).toHaveBeenCalledOnce());
  });

  it("prevents close and keeps the app open when confirmation is declined", async () => {
    state.bootstrap.mockResolvedValue(
      bootstrapData(timerSnapshot({ state: "focusRunning", phase: "focus" })),
    );
    const { getByText } = render(App);
    await waitFor(() => expect(getByText("Pause")).toBeTruthy());
    const handler = await waitForCloseHandler();
    const event = { preventDefault: vi.fn() };

    await handler(event);

    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(state.confirm).toHaveBeenCalledOnce();
    expect(state.closeApp).not.toHaveBeenCalled();
  });

  it("calls closeApp after the close is approved", async () => {
    state.bootstrap.mockResolvedValue(
      bootstrapData(timerSnapshot({ state: "focusRunning", phase: "focus" })),
    );
    state.confirm.mockResolvedValue(true);
    const { getByText } = render(App);
    await waitFor(() => expect(getByText("Pause")).toBeTruthy());
    const handler = await waitForCloseHandler();
    const event = { preventDefault: vi.fn() };

    await handler(event);

    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(state.confirm).toHaveBeenCalledOnce();
    expect(state.closeApp).toHaveBeenCalledOnce();
  });

  it("prompts before closing while startup is still in progress", async () => {
    const pending = deferred<Bootstrap>();
    state.bootstrap.mockReturnValueOnce(pending.promise);
    const { getByText } = render(App);
    const handler = await waitForCloseHandler();
    const event = { preventDefault: vi.fn() };

    await handler(event);

    expect(event.preventDefault).toHaveBeenCalledOnce();
    expect(state.confirm).toHaveBeenCalledOnce();
    expect(state.closeApp).not.toHaveBeenCalled();

    pending.resolve(bootstrapData());
    await waitFor(() => expect(getByText("Start unassigned")).toBeTruthy());
  });

  it("keeps a newer timer snapshot that arrives while bootstrap is resolving", async () => {
    const pending = deferred<Bootstrap>();
    state.bootstrap.mockReturnValueOnce(pending.promise);
    const { getAllByText, getByText, queryByText } = render(App);
    await waitFor(() => expect(state.listeners.has("timer-snapshot")).toBe(true));

    state.listeners.get("timer-snapshot")!({
      payload: timerSnapshot({
        state: "focusRunning",
        phase: "focus",
        remainingMs: 1_234_000,
      }),
    });
    pending.resolve(bootstrapData(timerSnapshot()));

    await waitFor(() => expect(getByText("Pause")).toBeTruthy());
    expect(getAllByText("20:34")).toHaveLength(2);
    expect(queryByText("Start unassigned")).toBeNull();
  });

  it("keeps a degraded subscription banner after bootstrap succeeds", async () => {
    state.failEvents.add("timer-alert");
    const { getByText } = render(App);

    await waitFor(() => expect(getByText("Live updates degraded")).toBeTruthy());
    await waitFor(() => expect(getByText("Start unassigned")).toBeTruthy());

    expect(getByText("Live updates degraded")).toBeTruthy();
    expect(getByText("timer-alert subscription failed")).toBeTruthy();
  });

  it("keeps listener failures after a command error is dismissed", async () => {
    state.failEvents.add("timer-alert");
    state.setMuted.mockRejectedValueOnce(new Error("Could not save mute state."));
    const { getByText, queryByText } = render(App);
    await waitFor(() => expect(getByText("Start unassigned")).toBeTruthy());

    await fireEvent.click(getByText("Mute"));
    await waitFor(() => expect(getByText("Could not save mute state.")).toBeTruthy());
    await fireEvent.click(getByText("Dismiss"));

    expect(queryByText("Could not save mute state.")).toBeNull();
    expect(getByText("Live updates degraded")).toBeTruthy();
  });

  it("cleans up every listener on unmount", async () => {
    const { getByText, unmount } = render(App);
    await waitFor(() => expect(getByText("Start unassigned")).toBeTruthy());
    await waitFor(() => expect(state.unlisteners).toHaveLength(4));

    unmount();

    for (const unlisten of state.unlisteners) {
      expect(unlisten).toHaveBeenCalledOnce();
    }
  });
});
