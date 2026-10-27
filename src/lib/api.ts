import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { TimerAlert, TimerSnapshot } from "./bindings";

export const EVENT_TIMER_SNAPSHOT = "timer-snapshot";
export const EVENT_DATA_CHANGED = "data-changed";
export const EVENT_TIMER_ALERT = "timer-alert";

export function onTimerSnapshot(cb: (snapshot: TimerSnapshot) => void): Promise<UnlistenFn> {
  return listen<TimerSnapshot>(EVENT_TIMER_SNAPSHOT, (event) => cb(event.payload));
}

export function onDataChanged(cb: () => void): Promise<UnlistenFn> {
  return listen(EVENT_DATA_CHANGED, () => cb());
}

export function onTimerAlert(cb: (alert: TimerAlert) => void): Promise<UnlistenFn> {
  return listen<TimerAlert>(EVENT_TIMER_ALERT, (event) => cb(event.payload));
}

export function formatError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return "Something went wrong.";
}
