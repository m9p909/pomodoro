<script lang="ts">
  import type { AnalyticsSnapshot, FocusRow } from "../bindings";
  import { formatDay, formatFocus } from "../format";
  import WeeklyMatrix from "./WeeklyMatrix.svelte";

  let { analytics }: { analytics: AnalyticsSnapshot } = $props();

  let hasData = $derived(
    analytics.today.rows.length > 0 || analytics.week.rows.length > 0,
  );

  function progressLabel(row: FocusRow): string {
    return row.estimatePomodoros == null
      ? `${row.completedPomodoros}`
      : `${row.completedPomodoros} / ${row.estimatePomodoros}`;
  }

  function progressDescription(row: FocusRow): string {
    return row.estimatePomodoros == null
      ? `${row.completedPomodoros} completed pomodoros`
      : `${row.completedPomodoros} of ${row.estimatePomodoros} estimated pomodoros completed`;
  }

  function barPercent(activeMs: number, totalMs: number): number {
    if (totalMs <= 0) return 0;
    return Math.max(0, Math.min(100, Math.round((activeMs / totalMs) * 100)));
  }
</script>

<section class="insights" aria-labelledby="insights-title">
  <div class="insights-heading">
    <p class="eyebrow">Insights</p>
    <h2 id="insights-title">Focus overview</h2>
  </div>

  {#if !hasData}
    <div class="empty-panel">
      <h3>No focus time yet</h3>
      <p>Start a focus session and your daily and weekly totals will appear here.</p>
    </div>
  {:else}
    <section class="today-panel" aria-labelledby="today-title">
      <div class="today-heading">
        <div>
          <p class="eyebrow">Today</p>
          <h3 id="today-title">{formatDay(analytics.today.date)}</h3>
        </div>
        <div class="daily-total">
          <span class="daily-total-value">{formatFocus(analytics.today.totalMs)}</span>
          <span class="daily-total-label">focused today</span>
        </div>
      </div>

      {#if analytics.today.rows.length === 0}
        <p class="empty-note">No focus time today.</p>
      {:else}
        <ul class="today-rows" aria-label="Focus time by todo today">
          {#each analytics.today.rows as row (row.key)}
            {@const percent = barPercent(row.activeMs, analytics.today.totalMs)}
            <li class="today-row">
              <div class="row-heading">
                <span class="row-title">{row.title}</span>
                <span class="row-metrics">
                  <span class="row-duration">{formatFocus(row.activeMs)}</span>
                  <span class="row-progress" aria-label={progressDescription(row)}>
                    {progressLabel(row)}
                  </span>
                </span>
              </div>
              <div
                class="bar-track"
                role="progressbar"
                aria-label={`${row.title} share of today's focus`}
                aria-valuemin="0"
                aria-valuemax="100"
                aria-valuenow={percent}
              >
                <span class="bar-fill" style:width={`${percent}%`}></span>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <WeeklyMatrix weeks={analytics.week} />
  {/if}
</section>

<style>
  .insights {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    max-width: 64rem;
    margin: 0 auto;
  }

  .insights-heading {
    padding: 0 0.2rem;
  }

  .eyebrow {
    margin: 0 0 0.25rem;
    color: var(--text-faint);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    font-size: 1.35rem;
    letter-spacing: -0.02em;
  }

  h3 {
    margin: 0;
    font-size: 1.05rem;
    letter-spacing: -0.01em;
  }

  .today-panel {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    padding: 1.5rem;
  }

  .today-heading {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 0.75rem 1rem;
  }

  .daily-total {
    display: flex;
    align-items: baseline;
    gap: 0.45rem;
    white-space: nowrap;
  }

  .daily-total-value {
    font-size: 1.7rem;
    font-weight: 700;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }

  .daily-total-label {
    color: var(--text-faint);
    font-size: 0.78rem;
  }

  .today-rows {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .today-row {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .row-heading {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.25rem 0.75rem;
  }

  .row-title {
    min-width: 0;
    overflow-wrap: anywhere;
    font-size: 0.88rem;
    font-weight: 600;
  }

  .row-metrics {
    display: flex;
    align-items: baseline;
    gap: 0.65rem;
    white-space: nowrap;
  }

  .row-duration {
    font-size: 0.85rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .row-progress {
    min-width: 2.6rem;
    color: var(--text-faint);
    font-size: 0.76rem;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .bar-track {
    height: 0.5rem;
    overflow: hidden;
    border-radius: 999px;
    background: var(--surface-muted);
  }

  .bar-fill {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, var(--accent-focus), var(--accent-focus-strong));
    transition: width 220ms ease;
  }

  .empty-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.6rem;
    border: 1px dashed var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--surface) 72%, transparent);
    padding: 2.5rem 1.5rem;
    text-align: center;
  }

  .empty-panel p {
    max-width: 28rem;
    margin: 0;
    color: var(--text-muted);
    font-size: 0.86rem;
    line-height: 1.55;
  }

  .empty-note {
    margin: 0;
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font-size: 0.84rem;
    padding: 1.4rem 1rem;
    text-align: center;
  }

  @media (max-width: 640px) {
    .today-panel {
      padding: 1rem;
    }
  }
</style>
