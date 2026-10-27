<script lang="ts">
  import type { WeeklyAnalytics } from "../bindings";
  import { formatDay, formatFocus } from "../format";

  let { weeks }: { weeks: WeeklyAnalytics } = $props();
</script>

<section class="weekly-panel" aria-labelledby="weekly-title">
  <div class="panel-heading">
    <div>
      <p class="eyebrow">Rolling 7 days</p>
      <h3 id="weekly-title">Weekly focus</h3>
    </div>
    <span class="week-total">
      <span class="week-total-value">{formatFocus(weeks.totalMs)}</span>
      <span class="week-total-label">total</span>
    </span>
  </div>

  {#if weeks.rows.length === 0}
    <p class="empty-note">No focus time this week.</p>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (scrollable regions must accept focus) -->
    <div class="table-scroll" role="region" aria-label="Weekly focus by todo" tabindex="0">
      <table>
        <caption class="sr-only">Focus time per todo across the last seven days</caption>
        <thead>
          <tr>
            <th scope="col" class="todo-col">Todo</th>
            {#each weeks.days as day (day)}
              <th scope="col" class="day-col">{formatDay(day)}</th>
            {/each}
            <th scope="col" class="total-col">Total</th>
          </tr>
        </thead>
        <tbody>
          {#each weeks.rows as row (row.key)}
            <tr>
              <th scope="row" class="todo-col">{row.title}</th>
              {#each row.cells as cell, index (index)}
                <td class="day-col" class:zero={cell === 0}>{formatFocus(cell)}</td>
              {/each}
              <td class="total-col">{formatFocus(row.totalMs)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr>
            <th scope="row" class="todo-col">Daily total</th>
            {#each weeks.dayTotals as total, index (index)}
              <td class="day-col">{formatFocus(total)}</td>
            {/each}
            <td class="total-col">{formatFocus(weeks.totalMs)}</td>
          </tr>
        </tfoot>
      </table>
    </div>
  {/if}
</section>

<style>
  .weekly-panel {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    padding: 1.5rem;
  }

  .panel-heading {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    justify-content: space-between;
    gap: 0.75rem 1rem;
  }

  .eyebrow {
    margin: 0 0 0.25rem;
    color: var(--text-faint);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }

  h3 {
    margin: 0;
    font-size: 1.05rem;
    letter-spacing: -0.01em;
  }

  .week-total {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    white-space: nowrap;
  }

  .week-total-value {
    font-size: 1.2rem;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .week-total-label {
    color: var(--text-faint);
    font-size: 0.76rem;
  }

  .table-scroll {
    max-width: 100%;
    overflow-x: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    -webkit-overflow-scrolling: touch;
  }

  .table-scroll:focus-visible {
    outline: 2px solid var(--accent-focus);
    outline-offset: 2px;
  }

  table {
    width: 100%;
    min-width: 42rem;
    border-collapse: collapse;
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }

  th,
  td {
    border-bottom: 1px solid var(--border);
    padding: 0.5rem 0.7rem;
    text-align: right;
    white-space: nowrap;
  }

  thead th {
    background: var(--surface-muted);
    color: var(--text-muted);
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  tbody th,
  tfoot th {
    text-align: left;
  }

  tbody th {
    font-weight: 600;
    max-width: 14rem;
    overflow-wrap: anywhere;
    white-space: normal;
  }

  tbody tr:last-child th,
  tbody tr:last-child td {
    border-bottom: 1px solid var(--border-strong);
  }

  tfoot th,
  tfoot td {
    border-bottom: 0;
    background: color-mix(in srgb, var(--surface-muted) 55%, var(--surface));
    color: var(--text-muted);
    font-weight: 700;
  }

  .todo-col {
    text-align: left;
  }

  .total-col {
    color: var(--text);
    font-weight: 700;
  }

  .day-col.zero {
    color: var(--text-faint);
    font-weight: 400;
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

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  @media (max-width: 640px) {
    .weekly-panel {
      padding: 1rem;
    }
  }
</style>
