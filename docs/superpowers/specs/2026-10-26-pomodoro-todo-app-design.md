# Pomodoro Todo App — Design

**Date:** 2026-10-26  
**Status:** Approved  
**Platforms:** Linux, macOS, Windows

## 1. Summary

A local-first desktop pomodoro and todo application built with Tauri 2, a Rust core, a thin Svelte 5 frontend, and SQLite. Users maintain a flat todo list with optional pomodoro estimates, run fixed 25/5/15 timer cycles, and review daily and rolling seven-day focus time per todo.

All business rules, timing, validation, persistence, and analytics live in Rust. Svelte only renders state, sends user commands, and plays the completion chime on request.

## 2. Goals

- Provide a fast desktop pomodoro timer that remains accurate while the window is minimized.
- Track active focus time per todo and per unassigned session.
- Save focus time incrementally, so quitting or crashing does not lose the session.
- Offer a simple daily dashboard and a rolling seven-day per-todo history.
- Ship as a self-contained, offline application bundle for each target OS.
- Keep the frontend intentionally thin and the Rust core independently testable.
- License the project under MIT and automate cross-platform CI builds and tagged releases with GitHub Actions.

## 3. Non-goals

- Accounts, cloud sync, or any network communication.
- Team features, sharing, or collaboration.
- Mobile or browser-hosted versions.
- Configurable timer durations.
- Projects, tags, subtasks, recurring todos, or due dates.
- System-tray operation or keeping the timer alive after the window closes.
- Auto-starting phases.
- Auto-updates, code signing, or notarization in the initial release.

## 4. Confirmed product decisions

- **Durations:** fixed 25-minute focus, 5-minute short break, 15-minute long break.
- **Break cycle:** a long break follows every fourth completed focus phase.
- **Manual transitions:** the user explicitly starts every focus phase and break.
- **Skipping:** breaks may be skipped while pending, running, or paused.
- **Estimates:** optional positive integer number of pomodoros; undefined by default.
- **Task selection:** playback starts focus for a todo. Focus may also run unassigned.
- **Task switching:** playing another todo during focus keeps the countdown running and changes attribution from that moment.
- **Focus accounting:** only active focus elapsed time counts. Pauses and breaks never count.
- **Partial sessions:** abandoning a session preserves focus time already earned.
- **Estimate credit:** a todo earns one completed pomodoro toward its estimate only when it owns the entire focus phase.
- **Cycle credit:** any completed focus phase advances the global four-pomodoro break cycle, even when tasks were switched.
- **Persistence:** all data is stored locally in SQLite, with active time checkpointed during a session.
- **Alerts:** completion plays a bundled chime and shows a native notification. Global mute suppresses both.
- **Window lifecycle:** minimizing keeps the timer running; closing ends the session after confirmation.
- **Distribution:** one self-contained platform-native app bundle per OS with embedded frontend assets and migrations.

## 5. User flows

### 5.1 Create a todo and start focusing

1. The user enters a title and may optionally enter an estimate in pomodoros.
2. The todo appears at the end of the active list.
3. Pressing play on the todo creates a focus session, attributes it to that todo, and starts a 25-minute countdown.
4. The timer ring and the todo both show that focus is active.

### 5.2 Complete a focus phase

1. When active focus reaches 25 minutes, Rust completes the session, checkpoints its final time, advances the global cycle count, and rings/notifies unless muted.
2. If the same todo held the entire phase, the todo gains one completed pomodoro of credit.
3. The timer enters a pending-break state showing the break length: 15 minutes after every fourth completed focus phase, otherwise 5 minutes.
4. The user starts the break, skips it, or starts another focus phase. Starting focus implicitly skips the pending break.

### 5.3 Switch tasks mid-focus

1. The user presses play on a different todo while a focus phase is running.
2. The countdown continues without interruption.
3. Rust checkpoints the time accrued so far under the previous todo, then attributes subsequent time to the new todo.
4. Switching attribution permanently forfeits estimate credit for the todo that started the phase, even if the user switches back.
5. The completed phase still advances the global break cycle.

### 5.4 Pause, resume, or abandon

1. Pause stops focus-time accrual and freezes the countdown. Resume continues from the same active elapsed time.
2. Pausing does not affect estimate credit if the same todo remains attributed.
3. Ending focus early records an abandoned session, keeps earned time in analytics, and grants no estimate or cycle credit.
4. Breaks support pause, resume, and skip.

### 5.5 Close or crash

1. Closing the window during a focus or break phase asks for confirmation.
2. On confirmation, Rust flushes earned focus time, marks the session abandoned, and exits.
3. If the process crashes, the next launch finds the active session, marks it abandoned at its last checkpoint, and starts idle.
4. A pending break does not survive restart; the completed-focus count does.

### 5.6 Review focus time

1. The daily dashboard shows today's total focus time and a per-todo breakdown, including unassigned time.
2. The weekly view shows the previous six local days plus today as a todo-by-day matrix with per-item and per-day totals.
3. Completed and deleted todos remain in history.

## 6. Architecture

### 6.1 Stack

- **Shell:** Tauri 2.
- **Backend:** Rust.
- **Frontend:** Svelte 5, TypeScript, Vite, static build.
- **Database:** SQLite through `rusqlite`, with embedded, versioned migrations.
- **Alerts:** a Rust-triggered completion event, a bundled chime played by the frontend, and the Tauri notification plugin.
- **IPC:** typed Tauri commands and events with generated TypeScript bindings.

### 6.2 Rust modules

- `domain` — todo, session, phase, settings, and analytics types; validation and invariants.
- `store` — SQLite connection management, migrations, transactions, and repositories.
- `timer` — the authoritative timer state machine, monotonic clock, sleep detection, checkpoints, and event emission.
- `todos` — create, edit, reorder, complete, soft-delete, and list operations.
- `analytics` — daily and rolling seven-day aggregation in Rust/SQL, including local-day boundaries.
- `settings` — persisted mute and global cycle count.
- `commands` — thin Tauri adapters that validate requests, call core services, and return typed results.

### 6.3 Frontend boundary

Svelte owns no business behavior. It:

- Renders todo lists, timer snapshots, and analytics responses.
- Sends commands such as play, pause, resume, stop, skip, reorder, and save.
- Formats durations and draws the countdown ring from values supplied by Rust.
- Plays the embedded chime only when Rust marks an alert as audible.

The frontend must not calculate timers, complete phases, count cycles, decide estimate credit, aggregate focus time, or implement task ordering rules.

### 6.4 Storage location

The database lives in the OS application-data directory for the app bundle ID. Frontend assets, migrations, and the chime are embedded in the bundle. There is no app-bundled server, language runtime, or network dependency. Tauri uses the OS-provided WebView: WKWebView on macOS, WebView2 on Windows, and WebKitGTK on Linux. Linux packages declare the WebKitGTK dependency.

### 6.5 Concurrency and lifecycle

- One Rust timer task is authoritative; it emits snapshots at least once per second.
- A single-instance guard prevents two app instances from writing the same database.
- SQLite uses WAL mode and foreign keys. Multi-row changes use transactions.
- On startup, active sessions are recovered as abandoned before the UI becomes interactive.

## 7. Timer state machine

### 7.1 States

- `idle`
- `focus_running`
- `focus_paused`
- `break_pending` — a completed focus phase is waiting for the user to start or skip a break.
- `break_running`
- `break_paused`

### 7.2 Transitions

| From | Action | To | Effects |
|---|---|---|---|
| `idle` | Start focus for todo or unassigned | `focus_running` | Create active session; start 25-minute countdown |
| `focus_running` | Pause | `focus_paused` | Stop accrual; flush checkpoint |
| `focus_paused` | Resume | `focus_running` | Continue same active elapsed time |
| `focus_paused` | Play the attributed todo | `focus_running` | Resume without changing credit |
| `focus_running` / `focus_paused` | Play a different todo | `focus_running` | Flush checkpoint; change attribution; clear credit; resume if paused |
| `focus_running` / `focus_paused` | End focus | `idle` | Mark session abandoned; flush time; no credit |
| `focus_running` | 25 minutes active elapsed | `break_pending` | Complete session; apply credit rules; increment cycle; alert |
| `break_pending` | Start break | `break_running` | Start 5- or 15-minute break |
| `break_pending` | Skip break | `idle` | Discard pending break; cycle unchanged |
| `break_pending` | Start focus | `focus_running` | Skip pending break; start new focus session |
| `break_running` | Pause | `break_paused` | Freeze break countdown |
| `break_paused` | Resume | `break_running` | Continue break |
| `break_running` / `break_paused` | Skip break | `idle` | End break; cycle unchanged |
| `break_running` | Break duration elapsed | `idle` | Alert; wait for next manual focus |

A running or paused break cannot be replaced by focus directly; the user skips it first. Todo playback is disabled during breaks. Play on the currently attributed todo is a no-op while running and resumes while paused. Deleting the actively attributed todo is equivalent to ending focus.

### 7.3 Break cycle

- The persisted `completed_focus_count` increments when a focus phase reaches 25 active minutes.
- After incrementing, the next break is long when `completed_focus_count % 4 == 0`; otherwise it is short.
- The sequence of breaks is short, short, short, long, short, short, short, long.
- Abandoned focus phases, skipped breaks, and paused time do not change the count.
- The pending break itself is in-memory and does not survive restart.

### 7.4 Task attribution and credit

- A focus session begins with a nullable `credit_todo_id` equal to its starting todo, or `NULL` when started unassigned.
- Any change of attribution to another todo, unassigned, or back to the original sets `credit_todo_id` to `NULL` permanently for that session.
- Pausing, resuming, editing the todo, or completing the todo does not change attribution.
- When the phase completes, the session is credited to `credit_todo_id` if it is non-null. This guarantees that only a todo that owned the entire phase earns estimate progress.
- Global cycle credit is independent of task credit.

### 7.5 Time accounting

- Time is measured with a monotonic clock and credited only while a focus phase is running.
- Each tick adds the active delta to the current session, todo, and local-day bucket.
- The current bucket is flushed to SQLite every 5 seconds and immediately on pause, task switch, stop, phase completion, close, or sleep detection.
- A watchdog treats a tick gap greater than 5 seconds as system sleep: Rust saves only the time up to the last tick, pauses the timer, and credits no time for the gap.
- The system clock may change without affecting the countdown. Local calendar dates are used only for analytics bucketing.

## 8. Data model

All timestamps are stored as UTC epoch milliseconds. Calendar days are local `YYYY-MM-DD` values computed in Rust.

### 8.1 `todos`

- `id` — UUID text primary key.
- `title` — trimmed, required, non-empty text.
- `estimate_pomodoros` — nullable positive integer.
- `sort_order` — integer for manual ordering.
- `created_at`, `updated_at` — timestamps.
- `completed_at` — nullable completion timestamp.
- `deleted_at` — nullable soft-delete timestamp.

Deleting a todo is a soft delete. Deleted todos are excluded from the active list but preserved for analytics and estimate history. Completing, editing, or reordering a todo does not interrupt a running focus session. Deleting the actively attributed todo asks for confirmation; confirming abandons the current focus phase, saves earned time, then soft-deletes the todo. Todo titles need not be unique.

### 8.2 `focus_sessions`

- `id` — UUID primary key.
- `started_at` — session start timestamp.
- `ended_at` — nullable end timestamp.
- `last_checkpoint_at` — timestamp of the last successful time flush.
- `outcome` — `active`, `completed`, or `abandoned`.
- `credit_todo_id` — nullable todo reference; set to `NULL` permanently on any attribution switch.

Active sessions found at startup are marked `abandoned`, with `ended_at` set to `last_checkpoint_at`.

### 8.3 `focus_time`

- `session_id` — foreign key to `focus_sessions`.
- `todo_id` — nullable todo reference; `NULL` represents unassigned time.
- `local_day` — local calendar date.
- `active_ms` — accumulated active focus milliseconds for this session/todo/day.
- `updated_at` — timestamp of the latest checkpoint.

The natural key is session, todo-or-unassigned, and local day, enforced with a sentinel value or partial unique index so unassigned time has exactly one bucket per session/day. Rust upserts this table as time accrues. A session crossing midnight creates a new day bucket. A task switch creates a new todo bucket under the same session.

### 8.4 `settings`

A singleton settings record stores:

- `muted` — global sound and notification mute.
- `completed_focus_count` — persisted count for the break cycle.

### 8.5 Indexes and integrity

- Foreign keys are enabled.
- Indexes cover active todo ordering, `focus_time(local_day)`, `focus_time(todo_id, local_day)`, and credited completed sessions.
- Migrations run transactionally before the app becomes interactive.

## 9. Analytics

### 9.1 Definitions

- **Focus time:** sum of `focus_time.active_ms` for focus phases only. Breaks, pauses, and system-sleep gaps never contribute.
- **Daily window:** the current local calendar day.
- **Weekly window:** today plus the previous six local days, ending today.
- **Estimate progress:** count of completed focus sessions whose `credit_todo_id` equals the todo.

### 9.2 Daily dashboard

Rust returns the total active focus time for today and rows for every todo with time today, plus an unassigned row when applicable. Rows include the todo title, time, and estimate progress. The UI renders a proportional bar and duration.

### 9.3 Weekly view

Rust returns a matrix for the seven-day window:

- One row per todo that has any focus time in the window, plus unassigned.
- One column per local day.
- A per-todo total and per-day total.
- Deleted todos remain visible under their last stored title.

Todos with zero focus time in the window are omitted. If no time exists, the UI shows an empty state.

### 9.4 Estimate display

- With an estimate: `completed / estimate` pomodoros.
- Without an estimate: completed pomodoros only, or no progress label until the first completion.
- Estimates can be edited or cleared. Progress derives from credited sessions and is not stored redundantly.

## 10. UI

### 10.1 Window structure

A single window with two tabs: **Focus** and **Insights**. A compact timer status remains visible in the header during Insights.

### 10.2 Focus tab

- **Timer card:** circular countdown ring, phase label, remaining time, attributed todo title or "Unassigned", and controls appropriate to the current state.
- **Controls:** Start, Pause, Resume, End focus, Start break, Skip break. Destructive ending or closing requires confirmation.
- **Todo list:** inline add form, optional estimate field, play control, completion checkbox, edit and delete actions, and drag ordering.
- **Completed todos:** collapsed section below the active list.
- **Unassigned focus:** available from the timer card.
- **Mute:** persistent header toggle.

### 10.3 Insights tab

- **Today:** total focus time and a per-todo breakdown with proportional bars, including unassigned time.
- **Last 7 days:** todo-by-day matrix with row and column totals.
- Empty states explain that focus time appears after a session starts.

### 10.4 Visual and interaction rules

- Neutral desktop styling that follows the system light/dark preference.
- Distinct focus and break accent colors.
- The ring and phase label are the primary timer status indicators.
- Controls are keyboard accessible; no action depends on hover.
- The UI reflects Rust snapshots and does not optimistically invent timer or analytics state.

## 11. Reliability and error handling

- All input is validated in Rust. Invalid titles or estimates return typed command errors and are not persisted.
- A failed checkpoint keeps unsaved time in memory, retries, and shows a persistent warning without stopping the timer.
- Closing with unsaved time prompts the user before exiting.
- Notification or chime failures never stop the timer or lose data. The in-app completion state always appears.
- If notification permission is denied, the timer still completes normally and the chime continues unless muted.
- A failure to open or migrate the database shows a fatal error screen with the database path; timer actions are unavailable.
- The single-instance guard focuses the existing window instead of starting a second timer process.
- Startup recovery finalizes any active session as abandoned at its last checkpoint.

## 12. Packaging, licensing, and release automation

### 12.1 App bundles

- Tauri builds one self-contained app bundle per platform:
  - Linux x86_64: AppImage (primary) and DEB.
  - macOS universal: `.app` bundle in a DMG, supporting Intel and Apple Silicon.
  - Windows x86_64: MSI.
- The Svelte bundle, migrations, and chime are embedded.
- SQLite data is stored in the platform app-data directory, not alongside the executable.
- There is no app-bundled server, language runtime, network access, or telemetry.
- Tauri relies on the OS-provided WebView (WKWebView, WebView2, or WebKitGTK). Linux packages declare WebKitGTK as a dependency.
- Initial builds are unsigned; signing and notarization are release follow-ups. macOS and Windows may show standard warnings for unsigned apps.

### 12.2 GitHub Actions

Two workflows are included in `.github/workflows/`:

- **CI** runs on pull requests and pushes to the default branch:
  - Rust formatting, Clippy with warnings denied, unit tests, and SQLite integration tests.
  - Svelte type checks, component tests, and a production frontend build.
  - The Rust and frontend checks run across Ubuntu, macOS, and Windows runners.
- **Release** runs when a tag matching `v*` is pushed:
  - Runs the same formatting, lint, test, and frontend build checks as CI before any artifact is published.
  - Builds Linux AppImage/DEB, macOS universal DMG, and Windows x64 MSI on native GitHub-hosted runners.
  - Publishes the GitHub Release immediately after all builds and checks pass, with all artifacts attached, using the repository `GITHUB_TOKEN` with `contents: write`.
  - Uses the Tauri GitHub Action for building and uploading release assets.
  - The tag version must match the application version declared in the Tauri configuration.
- Release artifacts are unsigned and no signing secrets are required.

### 12.3 License

- The repository includes a root `LICENSE` file containing the MIT License with `Copyright (c) 2026 Pomodoro contributors`.
- Rust and frontend package metadata declare the MIT license.
- The project is developed as publicly available open source; no proprietary assets are included.

## 13. Testing strategy

### 13.1 Rust unit tests

Use an injectable clock and in-memory/test SQLite:

- Every state transition, including invalid actions.
- Pause/resume and abandonment.
- Short/long break sequencing across multiple cycles.
- Break skipping without cycle changes.
- Task switches, split attribution, and permanent credit forfeiture.
- Estimate credit only for whole-phase ownership.
- Global cycle credit after task switches.
- Mute behavior.
- Sleep-gap detection.
- Checkpoint intervals and final flush behavior.
- Midnight and DST day bucketing.

### 13.2 Integration tests

Use temporary SQLite files:

- Todo CRUD, ordering, completion, and soft deletion.
- Session persistence and crash recovery.
- Daily and seven-day aggregation, including unassigned and deleted todos.
- Estimate progress derivation.
- Migration execution and schema integrity.

### 13.3 Frontend tests

Minimal component tests with mocked IPC:

- Timer snapshot rendering.
- Todo list interactions dispatch commands.
- Daily and weekly views render provided data.
- Mute and completion states.

### 13.4 Manual checks per OS

- Chime and native notification, including mute and denied permissions.
- Minimized timer accuracy.
- Close confirmation and startup recovery.
- System sleep during focus.
- Installed-bundle database location and offline operation.

## 14. Acceptance criteria

- Todos can be created, edited, reordered, completed, and deleted, and persist locally.
- Estimates are undefined by default, optional, positive integers in pomodoros, and displayed as progress.
- The timer has a visible circular countdown and supports pause and resume.
- Starting focus from a todo attributes time to it; switching todos continues the countdown and splits analytics.
- The break sequence is short, short, short, long, based on completed focus phases, with manual starts and skippable breaks.
- Completion produces a chime and native notification; global mute suppresses both and persists.
- Active focus time is saved at least every 5 seconds and on every finalization event.
- Focus time excludes pauses, breaks, and system sleep.
- Estimate progress increments only when one todo owned an entire completed focus phase.
- The daily dashboard shows today's per-item focus time.
- The weekly view shows per-item, per-day focus time for the last seven local days.
- Deleted and completed todos remain in history.
- The app is a self-contained Tauri bundle per target OS with a thin Svelte frontend and all business logic in Rust.
- The app works offline with no app-bundled server or language runtime, relying only on the OS-provided WebView.
- The repository contains the MIT License with the agreed copyright line and package metadata declaring MIT.
- GitHub Actions CI validates the Rust core and Svelte frontend on Linux, macOS, and Windows.
- Pushing a `v*` tag builds Linux x86_64, Windows x86_64, and macOS universal artifacts and publishes them immediately as a GitHub Release.
