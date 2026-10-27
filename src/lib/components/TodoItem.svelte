<script lang="ts">
  import type { Todo } from "../bindings";

  type TodoItemProps = {
    todo: Todo;
    isActive?: boolean;
    breakActive?: boolean;
    canMoveUp?: boolean;
    canMoveDown?: boolean;
    onPlay?: (id: string) => void;
    onUpdate?: (
      id: string,
      title: string,
      estimate: number | null,
    ) => boolean | void | Promise<boolean | void>;
    onToggle?: (id: string, completed: boolean) => boolean | void | Promise<boolean | void>;
    onDelete?: (id: string) => void;
    onMoveUp?: () => void;
    onMoveDown?: () => void;
    onDragStart?: (event: DragEvent) => void;
    onDragEnd?: () => void;
    onDragOver?: (event: DragEvent) => void;
    onDrop?: (event: DragEvent) => void;
  };

  let {
    todo,
    isActive = false,
    breakActive = false,
    canMoveUp = false,
    canMoveDown = false,
    onPlay = () => {},
    onUpdate = () => {},
    onToggle = () => {},
    onDelete = () => {},
    onMoveUp,
    onMoveDown,
    onDragStart,
    onDragEnd,
    onDragOver,
    onDrop,
  }: TodoItemProps = $props();

  let editing = $state(false);
  let draftTitle = $state("");
  let draftEstimate = $state("");
  let editError = $state<string | null>(null);

  let completed = $derived(todo.completedAt != null);
  let reorderable = $derived(onMoveUp != null && onMoveDown != null);
  let playDisabled = $derived(completed || breakActive || isActive);

  let progressLabel = $derived(
    todo.estimatePomodoros == null
      ? `${todo.completedPomodoros}`
      : `${todo.completedPomodoros} / ${todo.estimatePomodoros}`,
  );

  function startEdit() {
    draftTitle = todo.title;
    draftEstimate = todo.estimatePomodoros == null ? "" : `${todo.estimatePomodoros}`;
    editError = null;
    editing = true;
  }

  function cancelEdit() {
    editing = false;
    editError = null;
  }

  function parseEstimate(value: string): number | null {
    const trimmed = value.trim();
    if (trimmed === "") return null;
    const estimate = Number(trimmed);
    if (!Number.isInteger(estimate) || estimate <= 0) {
      throw new Error("Estimate must be a positive whole number.");
    }
    return estimate;
  }

  async function saveEdit(event: SubmitEvent) {
    event.preventDefault();
    const title = draftTitle.trim();
    if (!title) {
      editError = "Enter a todo title.";
      return;
    }

    let estimate: number | null;
    try {
      estimate = parseEstimate(draftEstimate);
    } catch (error) {
      editError = error instanceof Error ? error.message : "Enter a valid estimate.";
      return;
    }

    const succeeded = await onUpdate(todo.id, title, estimate);
    if (succeeded === false) return;
    editing = false;
    editError = null;
  }

  async function handleToggle(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const succeeded = await onToggle(todo.id, input.checked);
    if (succeeded === false) {
      input.checked = completed;
    }
  }
</script>

<li
  class="todo-item"
  class:completed
  class:running={isActive && !completed}
  ondragover={(event) => onDragOver?.(event)}
  ondrop={(event) => onDrop?.(event)}
>
  {#if reorderable}
    <span
      class="drag-handle"
      draggable="true"
      aria-hidden="true"
      title="Drag to reorder"
      ondragstart={(event) => onDragStart?.(event)}
      ondragend={() => onDragEnd?.()}
    >
      ⠿
    </span>
  {/if}

  <input
    class="todo-checkbox"
    type="checkbox"
    checked={completed}
    aria-label={`Mark ${todo.title} ${completed ? "incomplete" : "complete"}`}
    onchange={handleToggle}
  />

  {#if editing}
    <form class="edit-form" onsubmit={saveEdit}>
      <input
        class="edit-title"
        aria-label="Edit todo title"
        bind:value={draftTitle}
        autocomplete="off"
      />
      <input
        class="edit-estimate"
        type="number"
        min="1"
        step="1"
        aria-label="Edit estimate (pomodoros)"
        value={draftEstimate}
        oninput={(event) => (draftEstimate = event.currentTarget.value)}
        placeholder="Estimate"
      />
      <button type="submit" class="action primary">Save</button>
      <button type="button" class="action" onclick={cancelEdit}>Cancel</button>
    </form>
  {:else}
    <div class="todo-main">
      <span class="todo-title">{todo.title}</span>
      <span class="todo-progress">
        <span aria-hidden="true">{progressLabel}</span>
        <span class="sr-only">
          {todo.completedPomodoros} completed pomodoros{todo.estimatePomodoros == null
            ? ""
            : ` of ${todo.estimatePomodoros} estimated`}
        </span>
      </span>
    </div>

    <div class="todo-actions">
      {#if reorderable}
        <button
          type="button"
          class="action move"
          aria-label={`Move ${todo.title} up`}
          disabled={!canMoveUp}
          onclick={onMoveUp}
        >
          ↑
        </button>
        <button
          type="button"
          class="action move"
          aria-label={`Move ${todo.title} down`}
          disabled={!canMoveDown}
          onclick={onMoveDown}
        >
          ↓
        </button>
      {/if}
      <button
        type="button"
        class="action play"
        aria-label={`Play ${todo.title}`}
        disabled={playDisabled}
        onclick={() => onPlay(todo.id)}
      >
        Play
      </button>
      <button type="button" class="action" aria-label={`Edit ${todo.title}`} onclick={startEdit}>
        Edit
      </button>
      <button
        type="button"
        class="action danger"
        aria-label={`Delete ${todo.title}`}
        onclick={() => onDelete(todo.id)}
      >
        Delete
      </button>
    </div>
  {/if}

  {#if editError}
    <p class="edit-error" role="alert">{editError}</p>
  {/if}
</li>

<style>
  .todo-item {
    display: grid;
    grid-template-columns: auto auto minmax(0, 1fr);
    align-items: center;
    gap: 0.55rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    padding: 0.65rem 0.7rem;
    transition: border-color 150ms ease, background 150ms ease;
  }

  .todo-item:hover {
    border-color: var(--border-strong);
  }

  .todo-item.completed {
    background: color-mix(in srgb, var(--surface-muted) 55%, var(--surface));
  }

  .todo-item.running {
    border-color: color-mix(in srgb, var(--accent-focus) 45%, var(--border));
    background: color-mix(in srgb, var(--accent-focus) 5%, var(--surface));
  }

  .drag-handle {
    color: var(--text-faint);
    cursor: grab;
    font-size: 0.9rem;
    line-height: 1;
    padding: 0.15rem;
    user-select: none;
  }

  .drag-handle:active {
    cursor: grabbing;
  }

  .todo-checkbox {
    width: 1rem;
    height: 1rem;
    accent-color: var(--accent-focus);
    cursor: pointer;
  }

  .todo-main {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.75rem;
    min-width: 0;
  }

  .todo-title {
    overflow-wrap: anywhere;
    font-size: 0.9rem;
    font-weight: 600;
    line-height: 1.35;
  }

  .completed .todo-title {
    color: var(--text-muted);
    text-decoration: line-through;
    text-decoration-color: var(--text-faint);
  }

  .todo-progress {
    flex: none;
    color: var(--text-faint);
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .todo-actions {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 0.35rem;
  }

  .action {
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text-muted);
    font: inherit;
    font-size: 0.74rem;
    font-weight: 600;
    padding: 0.28rem 0.65rem;
    cursor: pointer;
    transition: border-color 150ms ease, color 150ms ease, background 150ms ease;
  }

  .action:hover:not(:disabled) {
    border-color: var(--border-strong);
    color: var(--text);
  }

  .action:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .action.move {
    min-width: 1.9rem;
    padding-inline: 0.4rem;
    font-size: 0.85rem;
  }

  .action.play {
    border-color: color-mix(in srgb, var(--accent-focus) 38%, var(--border));
    color: var(--accent-focus-strong);
  }

  .action.play:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent-focus) 9%, var(--surface));
    color: var(--accent-focus-strong);
  }

  .action.danger {
    color: var(--danger);
  }

  .action.danger:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--danger) 34%, var(--border));
    background: color-mix(in srgb, var(--danger) 7%, var(--surface));
    color: var(--danger);
  }

  .action.primary {
    border-color: transparent;
    background: var(--accent-focus);
    color: #fff;
  }

  .action.primary:hover:not(:disabled) {
    background: var(--accent-focus-strong);
    color: #fff;
  }

  .edit-form {
    grid-column: 2 / -1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.4rem;
  }

  .edit-title {
    flex: 1 1 9rem;
    min-width: 0;
  }

  .edit-estimate {
    flex: 0 0 6.5rem;
  }

  .edit-title,
  .edit-estimate {
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.84rem;
    padding: 0.4rem 0.55rem;
  }

  .edit-error {
    grid-column: 1 / -1;
    margin: 0.15rem 0 0;
    color: var(--danger);
    font-size: 0.76rem;
  }
</style>
