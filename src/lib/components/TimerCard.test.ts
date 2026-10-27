import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TimerSnapshot } from "../bindings";
import TimerCard from "./TimerCard.svelte";

afterEach(cleanup);

const snapshot = (overrides: Partial<TimerSnapshot> = {}): TimerSnapshot => ({
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

describe("TimerCard", () => {
  it("shows the initial focus duration and start control", async () => {
    const onStartUnassigned = vi.fn();
    const { getByText } = render(TimerCard, { timer: snapshot(), onStartUnassigned } as never);

    expect(getByText("25:00")).toBeTruthy();
    await fireEvent.click(getByText("Start unassigned"));
    expect(onStartUnassigned).toHaveBeenCalledOnce();
  });

  it("shows pause and end controls while focus is running", async () => {
    const onPause = vi.fn();
    const onEnd = vi.fn();
    const { getByText } = render(TimerCard, {
      timer: snapshot({
        state: "focusRunning",
        phase: "focus",
        remainingMs: 1_234_000,
        attributedTodoId: "todo-1",
        attributedTodoTitle: "Write tests",
      }),
      onPause,
      onEnd,
    } as never);

    expect(getByText("20:34")).toBeTruthy();
    expect(getByText("Write tests")).toBeTruthy();
    await fireEvent.click(getByText("Pause"));
    await fireEvent.click(getByText("End focus"));
    expect(onPause).toHaveBeenCalledOnce();
    expect(onEnd).toHaveBeenCalledOnce();
  });

  it("shows resume and end controls while focus is paused", async () => {
    const onResume = vi.fn();
    const onEnd = vi.fn();
    const { getByText } = render(TimerCard, {
      timer: snapshot({
        state: "focusPaused",
        phase: "focus",
        attributedTodoId: "todo-1",
        attributedTodoTitle: "Write tests",
      }),
      onResume,
      onEnd,
    } as never);

    await fireEvent.click(getByText("Resume"));
    await fireEvent.click(getByText("End focus"));
    expect(onResume).toHaveBeenCalledOnce();
    expect(onEnd).toHaveBeenCalledOnce();
  });

  it("shows start and skip controls while a break is pending", async () => {
    const onStartBreak = vi.fn();
    const onSkipBreak = vi.fn();
    const { getByText } = render(TimerCard, {
      timer: snapshot({
        state: "breakPending",
        phase: "shortBreak",
        remainingMs: 300_000,
        totalMs: 300_000,
        attributedTodoId: "todo-1",
        attributedTodoTitle: "Write tests",
      }),
      onStartBreak,
      onSkipBreak,
    } as never);

    expect(getByText("5:00")).toBeTruthy();
    await fireEvent.click(getByText("Start break"));
    await fireEvent.click(getByText("Skip break"));
    expect(onStartBreak).toHaveBeenCalledOnce();
    expect(onSkipBreak).toHaveBeenCalledOnce();
  });

  it("shows pause and skip controls while a break is running", async () => {
    const onPause = vi.fn();
    const onSkipBreak = vi.fn();
    const { getByText } = render(TimerCard, {
      timer: snapshot({
        state: "breakRunning",
        phase: "longBreak",
        remainingMs: 900_000,
        totalMs: 900_000,
      }),
      onPause,
      onSkipBreak,
    } as never);

    expect(getByText("15:00")).toBeTruthy();
    await fireEvent.click(getByText("Pause"));
    await fireEvent.click(getByText("Skip break"));
    expect(onPause).toHaveBeenCalledOnce();
    expect(onSkipBreak).toHaveBeenCalledOnce();
  });

  it("shows resume and skip controls while a break is paused", async () => {
    const onResume = vi.fn();
    const onSkipBreak = vi.fn();
    const { getByText } = render(TimerCard, {
      timer: snapshot({
        state: "breakPaused",
        phase: "shortBreak",
        remainingMs: 180_000,
        totalMs: 300_000,
      }),
      onResume,
      onSkipBreak,
    } as never);

    await fireEvent.click(getByText("Resume"));
    await fireEvent.click(getByText("Skip break"));
    expect(onResume).toHaveBeenCalledOnce();
    expect(onSkipBreak).toHaveBeenCalledOnce();
  });

  it("shows a muted badge when sound is muted", () => {
    const { getByText } = render(TimerCard, {
      timer: snapshot({ muted: true }),
    } as never);

    expect(getByText("Muted")).toBeTruthy();
  });

  it("shows the persistence warning banner", () => {
    const { getByText } = render(TimerCard, {
      timer: snapshot({ persistenceWarning: "Focus time could not be saved." }),
    } as never);

    expect(getByText("Save warning")).toBeTruthy();
    expect(getByText("Focus time could not be saved.")).toBeTruthy();
  });
});
