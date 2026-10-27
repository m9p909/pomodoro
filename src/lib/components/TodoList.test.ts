import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import TodoList from "./TodoList.svelte";
import type { Todo } from "../bindings";

afterEach(cleanup);

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

function props(todos: Todo[] = []) {
  return {
    todos,
    activeTodoId: null,
    breakActive: false,
    onPlay: vi.fn(),
    onCreate: vi.fn(),
    onUpdate: vi.fn(),
    onToggle: vi.fn(),
    onDelete: vi.fn(),
    onReorder: vi.fn(),
  };
}

describe("TodoList", () => {
  it("creates a trimmed todo with no estimate by default", async () => {
    const p = props();
    const { getByLabelText, getByRole } = render(TodoList, { props: p });
    await fireEvent.input(getByLabelText("New todo title"), {
      target: { value: "  Write tests  " },
    });
    await fireEvent.click(getByRole("button", { name: "Add todo" }));
    expect(p.onCreate).toHaveBeenCalledWith("Write tests", null);
  });

  it("creates a todo with an estimate", async () => {
    const p = props();
    const { getByLabelText, getByRole } = render(TodoList, { props: p });
    await fireEvent.input(getByLabelText("New todo title"), {
      target: { value: "Plan sprint" },
    });
    await fireEvent.input(getByLabelText("Estimate (pomodoros)"), {
      target: { value: "4" },
    });
    await fireEvent.click(getByRole("button", { name: "Add todo" }));
    expect(p.onCreate).toHaveBeenCalledWith("Plan sprint", 4);
  });

  it("blocks a zero estimate", async () => {
    const p = props();
    const { getByLabelText, getByRole } = render(TodoList, { props: p });
    await fireEvent.input(getByLabelText("New todo title"), {
      target: { value: "Task" },
    });
    await fireEvent.input(getByLabelText("Estimate (pomodoros)"), {
      target: { value: "0" },
    });
    await fireEvent.click(getByRole("button", { name: "Add todo" }));
    expect(p.onCreate).not.toHaveBeenCalled();
  });

  it("blocks a negative estimate", async () => {
    const p = props();
    const { getByLabelText, getByRole } = render(TodoList, { props: p });
    await fireEvent.input(getByLabelText("New todo title"), {
      target: { value: "Task" },
    });
    await fireEvent.input(getByLabelText("Estimate (pomodoros)"), {
      target: { value: "-2" },
    });
    await fireEvent.click(getByRole("button", { name: "Add todo" }));
    expect(p.onCreate).not.toHaveBeenCalled();
  });

  it("blocks a non-integer estimate", async () => {
    const p = props();
    const { getByLabelText, getByRole } = render(TodoList, { props: p });
    await fireEvent.input(getByLabelText("New todo title"), {
      target: { value: "Task" },
    });
    await fireEvent.input(getByLabelText("Estimate (pomodoros)"), {
      target: { value: "1.5" },
    });
    await fireEvent.click(getByRole("button", { name: "Add todo" }));
    expect(p.onCreate).not.toHaveBeenCalled();
  });

  it("shows completed todos in a collapsed section", () => {
    const p = props([todo({ id: "b", title: "Done task", completedAt: 1 })]);
    const { getByText } = render(TodoList, { props: p });
    const summary = getByText("Completed (1)");
    expect(summary).toBeTruthy();
    expect(getByText("Done task")).toBeTruthy();
    expect(summary.closest("details")?.open).toBe(false);
  });

  it("shows an empty state when there are no active todos", () => {
    const p = props([todo({ id: "b", title: "Done task", completedAt: 1 })]);
    const { getByText } = render(TodoList, { props: p });
    expect(getByText("No active todos yet.")).toBeTruthy();
  });

  it("orders the active list by sort order", () => {
    const p = props([
      todo({ id: "b", title: "Second", sortOrder: 1 }),
      todo({ id: "a", title: "First", sortOrder: 0 }),
    ]);
    const { getAllByRole } = render(TodoList, { props: p });
    const titles = getAllByRole("listitem").map((item) => item.textContent);
    expect(titles[0]).toContain("First");
    expect(titles[1]).toContain("Second");
  });

  it("moves an active todo down via the accessible control", async () => {
    const p = props([
      todo({ id: "a", title: "A", sortOrder: 0 }),
      todo({ id: "b", title: "B", sortOrder: 1 }),
    ]);
    const { getByRole } = render(TodoList, { props: p });
    await fireEvent.click(getByRole("button", { name: "Move A down" }));
    expect(p.onReorder).toHaveBeenCalledWith(["b", "a"]);
  });

  it("moves an active todo up via the accessible control", async () => {
    const p = props([
      todo({ id: "a", title: "A", sortOrder: 0 }),
      todo({ id: "b", title: "B", sortOrder: 1 }),
    ]);
    const { getByRole } = render(TodoList, { props: p });
    await fireEvent.click(getByRole("button", { name: "Move B up" }));
    expect(p.onReorder).toHaveBeenCalledWith(["b", "a"]);
  });

  it("reorders active todos by drag and drop", async () => {
    const p = props([
      todo({ id: "a", title: "A", sortOrder: 0 }),
      todo({ id: "b", title: "B", sortOrder: 1 }),
    ]);
    const { getAllByTitle, getByText } = render(TodoList, { props: p });
    await fireEvent.dragStart(getAllByTitle("Drag to reorder")[0]);
    await fireEvent.drop(getByText("B").closest("li")!);
    expect(p.onReorder).toHaveBeenCalledWith(["b", "a"]);
  });

  it("edits a todo inline", async () => {
    const p = props([todo({ id: "a", title: "A" })]);
    const { getByLabelText, getByRole } = render(TodoList, { props: p });
    await fireEvent.click(getByRole("button", { name: "Edit A" }));
    await fireEvent.input(getByLabelText("Edit todo title"), {
      target: { value: "  Updated title  " },
    });
    await fireEvent.input(getByLabelText("Edit estimate (pomodoros)"), {
      target: { value: "5" },
    });
    await fireEvent.click(getByRole("button", { name: "Save" }));
    expect(p.onUpdate).toHaveBeenCalledWith("a", "Updated title", 5);
  });

  it("clears the estimate when the edit estimate is blank", async () => {
    const p = props([todo({ id: "a", title: "A" })]);
    const { getByLabelText, getByRole } = render(TodoList, { props: p });
    await fireEvent.click(getByRole("button", { name: "Edit A" }));
    await fireEvent.input(getByLabelText("Edit estimate (pomodoros)"), {
      target: { value: "" },
    });
    await fireEvent.click(getByRole("button", { name: "Save" }));
    expect(p.onUpdate).toHaveBeenCalledWith("a", "A", null);
  });

  it("blocks an invalid estimate while editing", async () => {
    const p = props([todo({ id: "a", title: "A" })]);
    const { getByLabelText, getByRole } = render(TodoList, { props: p });
    await fireEvent.click(getByRole("button", { name: "Edit A" }));
    await fireEvent.input(getByLabelText("Edit estimate (pomodoros)"), {
      target: { value: "0" },
    });
    await fireEvent.click(getByRole("button", { name: "Save" }));
    expect(p.onUpdate).not.toHaveBeenCalled();
  });

  it("toggles completion from the checkbox", async () => {
    const p = props([todo({ id: "a", title: "A" })]);
    const { getByLabelText } = render(TodoList, { props: p });
    await fireEvent.click(getByLabelText("Mark A complete"));
    expect(p.onToggle).toHaveBeenCalledWith("a", true);
  });

  it("uncompletes a completed todo from the checkbox", async () => {
    const p = props([todo({ id: "a", title: "A", completedAt: 1 })]);
    const { getByLabelText } = render(TodoList, { props: p });
    await fireEvent.click(getByLabelText("Mark A incomplete"));
    expect(p.onToggle).toHaveBeenCalledWith("a", false);
  });

  it("deletes a todo", async () => {
    const p = props([todo({ id: "a", title: "A" })]);
    const { getByRole } = render(TodoList, { props: p });
    await fireEvent.click(getByRole("button", { name: "Delete A" }));
    expect(p.onDelete).toHaveBeenCalledWith("a");
  });

  it("plays an available todo", async () => {
    const p = props([todo({ id: "a", title: "A" })]);
    const { getByRole } = render(TodoList, { props: p });
    await fireEvent.click(getByRole("button", { name: "Play A" }));
    expect(p.onPlay).toHaveBeenCalledWith("a");
  });

  it("disables play for the currently active todo", () => {
    const p = { ...props([todo({ id: "a", title: "A" })]), activeTodoId: "a" };
    const { getByRole } = render(TodoList, { props: p });
    expect(getByRole("button", { name: "Play A" }).hasAttribute("disabled")).toBe(true);
  });

  it("disables play while a break is active", () => {
    const p = { ...props([todo({ id: "a", title: "A" })]), breakActive: true };
    const { getByRole } = render(TodoList, { props: p });
    expect(getByRole("button", { name: "Play A" }).hasAttribute("disabled")).toBe(true);
  });

  it("disables play for a completed todo", () => {
    const p = props([todo({ id: "a", title: "A", completedAt: 1 })]);
    const { getByRole } = render(TodoList, { props: p });
    expect(getByRole("button", { name: "Play A" }).hasAttribute("disabled")).toBe(true);
  });

  it("shows the pomodoro estimate and progress", () => {
    const p = props([todo({ id: "a", title: "A", estimatePomodoros: 5, completedPomodoros: 2 })]);
    const { getByText } = render(TodoList, { props: p });
    expect(getByText("2 / 5")).toBeTruthy();
  });
});
