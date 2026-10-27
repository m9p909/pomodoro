<script lang="ts">
  import type { Todo } from "../bindings";
  import TodoItem from "./TodoItem.svelte";

  type TodoListProps = {
    todos: Todo[];
    activeTodoId: string | null;
    breakActive: boolean;
    onPlay?: (id: string) => void;
    onCreate?: (title: string, estimate: number | null) => void;
    onUpdate?: (id: string, title: string, estimate: number | null) => void;
    onToggle?: (id: string, completed: boolean) => void;
    onDelete?: (id: string) => void;
    onReorder?: (ids: string[]) => void;
  };

  let {
    todos,
    activeTodoId,
    breakActive,
    onPlay = () => {},
    onCreate = () => {},
    onUpdate = () => {},
    onToggle = () => {},
    onDelete = () => {},
    onReorder = () => {},
  }: TodoListProps = $props();

  let newTitle = $state("");
  let newEstimate = $state("");
  let createError = $state<string | null>(null);
  let draggedId = $state<string | null>(null);

  let activeTodos = $derived(
    todos
      .filter((todo) => todo.completedAt == null)
      .sort((left, right) => left.sortOrder - right.sortOrder),
  );

  let completedTodos = $derived(
    todos
      .filter((todo) => todo.completedAt != null)
      .sort((left, right) => left.sortOrder - right.sortOrder),
  );

  function parseEstimate(value: string): number | null {
    const trimmed = value.trim();
    if (trimmed === "") return null;
    const estimate = Number(trimmed);
    if (!Number.isInteger(estimate) || estimate <= 0) {
      throw new Error("Estimate must be a positive whole number.");
    }
    return estimate;
  }

  function handleCreate(event: SubmitEvent) {
    event.preventDefault();
    const title = newTitle.trim();
    if (!title) {
      createError = "Enter a todo title.";
      return;
    }

    let estimate: number | null;
    try {
      estimate = parseEstimate(newEstimate);
    } catch (error) {
      createError = error instanceof Error ? error.message : "Enter a valid estimate.";
      return;
    }

    onCreate(title, estimate);
    newTitle = "";
    newEstimate = "";
    createError = null;
  }

  function move(id: string, offset: number) {
    const order = activeTodos.map((todo) => todo.id);
    const from = order.indexOf(id);
    const to = from + offset;
    if (from === -1 || to < 0 || to >= order.length) return;
    order.splice(from, 1);
    order.splice(to, 0, id);
    onReorder(order);
  }

  function startDrag(event: DragEvent, id: string) {
    draggedId = id;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", id);
    }
  }

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  }

  function dropOn(event: DragEvent, targetId: string) {
    event.preventDefault();
    const source = draggedId ?? event.dataTransfer?.getData("text/plain") ?? null;
    draggedId = null;
    if (!source || source === targetId) return;

    const order = activeTodos.map((todo) => todo.id);
    const from = order.indexOf(source);
    const to = order.indexOf(targetId);
    if (from === -1 || to === -1) return;
    order.splice(from, 1);
    order.splice(to, 0, source);
    onReorder(order);
  }
</script>

<section class="todo-panel" aria-labelledby="todo-list-title">
  <div class="panel-heading">
    <p class="eyebrow">Todo list</p>
    <span class="active-count">{activeTodos.length} active</span>
  </div>
  <h2 id="todo-list-title">Plan the work</h2>

  <form class="create-form" onsubmit={handleCreate}>
    <div class="field">
      <label for="new-todo-title">New todo title</label>
      <input
        id="new-todo-title"
        bind:value={newTitle}
        placeholder="What needs doing?"
        autocomplete="off"
      />
    </div>
    <div class="field estimate-field">
      <label for="new-todo-estimate">Estimate (pomodoros)</label>
      <input
        id="new-todo-estimate"
        type="number"
        min="1"
        step="1"
        value={newEstimate}
        oninput={(event) => (newEstimate = event.currentTarget.value)}
        placeholder="Optional"
      />
    </div>
    <button type="submit" class="add-button">Add todo</button>
  </form>

  {#if createError}
    <p class="form-error" role="alert">{createError}</p>
  {/if}

  {#if activeTodos.length === 0}
    <p class="empty-state">No active todos yet.</p>
  {:else}
    <ul class="todo-list" aria-label="Active todos">
      {#each activeTodos as todo, index (todo.id)}
        <TodoItem
          {todo}
          isActive={todo.id === activeTodoId}
          {breakActive}
          canMoveUp={index > 0}
          canMoveDown={index < activeTodos.length - 1}
          {onPlay}
          {onUpdate}
          {onToggle}
          {onDelete}
          onMoveUp={() => move(todo.id, -1)}
          onMoveDown={() => move(todo.id, 1)}
          onDragStart={(event) => startDrag(event, todo.id)}
          onDragEnd={() => (draggedId = null)}
          onDragOver={handleDragOver}
          onDrop={(event) => dropOn(event, todo.id)}
        />
      {/each}
    </ul>
  {/if}

  {#if completedTodos.length > 0}
    <details class="completed-section">
      <summary>Completed ({completedTodos.length})</summary>
      <ul class="todo-list completed-list" aria-label="Completed todos">
        {#each completedTodos as todo (todo.id)}
          <TodoItem {todo} {breakActive} {onPlay} {onUpdate} {onToggle} {onDelete} />
        {/each}
      </ul>
    </details>
  {/if}
</section>

<style>
  .todo-panel {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    padding: 1.5rem;
  }

  .panel-heading {
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

  .active-count {
    color: var(--text-faint);
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  h2 {
    margin: 0;
    font-size: 1.2rem;
    letter-spacing: -0.01em;
  }

  .create-form {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 9rem auto;
    align-items: end;
    gap: 0.55rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--surface-muted) 45%, var(--surface));
    padding: 0.75rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
  }

  .field label {
    color: var(--text-muted);
    font-size: 0.72rem;
    font-weight: 600;
  }

  .field input {
    width: 100%;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 0.85rem;
    padding: 0.45rem 0.55rem;
  }

  .add-button {
    border: 1px solid transparent;
    border-radius: 999px;
    background: var(--accent-focus);
    color: #fff;
    font: inherit;
    font-size: 0.82rem;
    font-weight: 600;
    padding: 0.5rem 0.9rem;
    cursor: pointer;
    transition: background 150ms ease;
  }

  .add-button:hover {
    background: var(--accent-focus-strong);
  }

  .form-error {
    margin: -0.35rem 0 0;
    color: var(--danger);
    font-size: 0.78rem;
  }

  .empty-state {
    margin: 0;
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font-size: 0.84rem;
    padding: 1.4rem 1rem;
    text-align: center;
  }

  .todo-list {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .completed-section {
    border-top: 1px solid var(--border);
    padding-top: 0.7rem;
  }

  .completed-section summary {
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 600;
    padding: 0.15rem 0;
    user-select: none;
  }

  .completed-section summary:hover {
    color: var(--text);
  }

  .completed-list {
    margin-top: 0.55rem;
  }

  @media (max-width: 560px) {
    .create-form {
      grid-template-columns: minmax(0, 1fr);
    }

    .add-button {
      justify-self: start;
    }
  }
</style>
