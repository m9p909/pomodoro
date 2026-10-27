import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App.svelte";
import type {
  AnalyticsSnapshot,
  Bootstrap,
  DataSnapshot,
  TimerAlert,
  TimerSnapshot,
  Todo,
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
    createTodo: vi.fn(),
    updateTodo: vi.fn(),
    setTodoCompleted: vi.fn(),
    deleteTodo: vi.fn(),
    reorderTodos: vi.fn(),
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
    createTodo: state.createTodo,
    updateTodo: state.updateTodo,
    setTodoCompleted: state.setTodoCompleted,
    deleteTodo: state.deleteTodo,
    reorderTodos: state.reorderTodos,
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

const todo = (overrides: Partial<Todo> = {}): Todo => ({
  id: "a",
  title: "Write tests",
  estimatePomodoros: 3,
  sortOrder: 0,
  completedAt: null,
  createdAt: 0,
  updatedAt: 0,
  completedPomodoros: 0,
  ...overrides,
});

const analytics: AnalyticsSnapshot = {
  today: { date: "2026-10-26", totalMs: 0, rows: [] },
  week: { days: [], rows: [], dayTotals: [], totalMs: 0 },
};

const bootstrapData = (timer: TimerSnapshot = timerSnapshot(), todos: Todo[] = []): Bootstrap => ({
  todos,
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
    state.createTodo.mockResolvedValue(todo());
    state.updateTodo.mockResolvedValue(todo());
    state.setTodoCompleted.mockResolvedValue(todo());
    state.deleteTodo.mockResolvedValue(timerSnapshot());
    state.reorderTodos.mockResolvedValue([]);
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

  it("asks for confirmation before deleting a todo", async () => {
    state.bootstrap.mockResolvedValue(
      bootstrapData(timerSnapshot(), [todo({ id: "a", title: "A" })]),
    );
    const { getByRole } = render(App);
    await waitFor(() => expect(getByRole("button", { name: "Delete A" })).toBeTruthy());

    await fireEvent.click(getByRole("button", { name: "Delete A" }));

    await waitFor(() => expect(state.confirm).toHaveBeenCalledOnce());
    expect(state.deleteTodo).not.toHaveBeenCalled();
  });

  it("deletes a todo and refreshes after confirmation", async () => {
    state.bootstrap.mockResolvedValue(
      bootstrapData(timerSnapshot(), [todo({ id: "a", title: "A" })]),
    );
    state.confirm.mockResolvedValue(true);
    const { getByRole } = render(App);
    await waitFor(() => expect(getByRole("button", { name: "Delete A" })).toBeTruthy());

    await fireEvent.click(getByRole("button", { name: "Delete A" }));

    await waitFor(() => expect(state.deleteTodo).toHaveBeenCalledWith("a"));
    await waitFor(() => expect(state.refreshData).toHaveBeenCalled());
  });

  it("creates a todo and refreshes", async () => {
    const { getByLabelText, getByRole } = render(App);
    await waitFor(() => expect(getByLabelText("New todo title")).toBeTruthy());

    await fireEvent.input(getByLabelText("New todo title"), {
      target: { value: "  New task  " },
    });
    await fireEvent.click(getByRole("button", { name: "Add todo" }));

    await waitFor(() => expect(state.createTodo).toHaveBeenCalledWith("New task", null));
    await waitFor(() => expect(state.refreshData).toHaveBeenCalled());
  });

  it("reorders active todos and appends completed ids for the command", async () => {
    state.bootstrap.mockResolvedValue(
      bootstrapData(timerSnapshot(), [
        todo({ id: "a", title: "A", sortOrder: 0 }),
        todo({ id: "b", title: "B", sortOrder: 1 }),
        todo({ id: "c", title: "Done", sortOrder: 2, completedAt: 1 }),
      ]),
    );
    const { getByRole } = render(App);
    await waitFor(() => expect(getByRole("button", { name: "Move A down" })).toBeTruthy());

    await fireEvent.click(getByRole("button", { name: "Move A down" }));

    await waitFor(() => expect(state.reorderTodos).toHaveBeenCalledWith(["b", "a", "c"]));
    expect(state.refreshData).toHaveBeenCalled();
  });

  it("updates a todo and refreshes", async () => {
    state.bootstrap.mockResolvedValue(
      bootstrapData(timerSnapshot(), [todo({ id: "a", title: "A" })]),
    );
    const { getByLabelText, getByRole } = render(App);
    await waitFor(() => expect(getByRole("button", { name: "Edit A" })).toBeTruthy());

    await fireEvent.click(getByRole("button", { name: "Edit A" }));
    await fireEvent.input(getByLabelText("Edit todo title"), {
      target: { value: "  Updated title  " },
    });
    await fireEvent.input(getByLabelText("Edit estimate (pomodoros)"), {
      target: { value: "5" },
    });
    await fireEvent.click(getByRole("button", { name: "Save" }));

    await waitFor(() => expect(state.updateTodo).toHaveBeenCalledWith("a", "Updated title", 5));
    expect(state.refreshData).toHaveBeenCalled();
  });

  it("toggles a todo completion and refreshes", async () => {
    state.bootstrap.mockResolvedValue(
      bootstrapData(timerSnapshot(), [todo({ id: "a", title: "A" })]),
    );
    const { getByLabelText } = render(App);
    await waitFor(() => expect(getByLabelText("Mark A complete")).toBeTruthy());

    await fireEvent.click(getByLabelText("Mark A complete"));

    await waitFor(() => expect(state.setTodoCompleted).toHaveBeenCalledWith("a", true));
    expect(state.refreshData).toHaveBeenCalled();
  });

  it("clears a command error after a successful todo command", async () => {
    state.setMuted.mockRejectedValueOnce(new Error("Could not save mute state."));
    const { getByLabelText, getByRole, getByText, queryByText } = render(App);
    await waitFor(() => expect(getByText("Start unassigned")).toBeTruthy());

    await fireEvent.click(getByText("Mute"));
    await waitFor(() => expect(getByText("Could not save mute state.")).toBeTruthy());

    await fireEvent.input(getByLabelText("New todo title"), {
      target: { value: "Clean up" },
    });
    await fireEvent.click(getByRole("button", { name: "Add todo" }));

    await waitFor(() => expect(state.createTodo).toHaveBeenCalledWith("Clean up", null));
    await waitFor(() => expect(queryByText("Could not save mute state.")).toBeNull());
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
