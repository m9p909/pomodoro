import type { TimerSnapshot } from "./bindings";

export function formatTimer(ms: number): string {
  const totalSeconds = Math.max(0, Math.ceil(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

export function formatFocus(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "0m";

  const totalMinutes = Math.round(ms / 60_000);
  if (totalMinutes < 1) return "<1m";
  if (totalMinutes < 60) return `${totalMinutes}m`;

  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return minutes === 0 ? `${hours}h` : `${hours}h ${minutes}m`;
}

export function formatDay(day: string): string {
  const parts = day.split("-").map(Number);
  if (parts.length === 3 && parts.every(Number.isFinite)) {
    const date = new Date(parts[0], parts[1] - 1, parts[2]);
    if (!Number.isNaN(date.getTime())) {
      return new Intl.DateTimeFormat(undefined, {
        month: "short",
        day: "numeric",
      }).format(date);
    }
  }
  return day;
}

export function phaseLabel(timer: TimerSnapshot): string {
  if (timer.state === "idle") return "Ready";

  switch (timer.phase) {
    case "focus":
      return "Focus";
    case "shortBreak":
      return "Short break";
    case "longBreak":
      return "Long break";
    default:
      return "Ready";
  }
}
