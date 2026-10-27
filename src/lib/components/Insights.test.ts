import { cleanup, render, within } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import Insights from "./Insights.svelte";
import type { AnalyticsSnapshot } from "../bindings";

afterEach(cleanup);

const populated: AnalyticsSnapshot = {
  today: {
    date: "2026-10-26",
    totalMs: 1_200_000,
    rows: [
      { key: "a", todoId: "a", title: "Write tests", activeMs: 600_000, completedPomodoros: 2, estimatePomodoros: 4 },
      { key: "b", todoId: "b", title: "Review", activeMs: 300_000, completedPomodoros: 1, estimatePomodoros: null },
      { key: "unassigned", todoId: null, title: "Unassigned", activeMs: 300_000, completedPomodoros: 0, estimatePomodoros: null },
    ],
  },
  week: {
    days: ["2026-10-20", "2026-10-21", "2026-10-22", "2026-10-23", "2026-10-24", "2026-10-25", "2026-10-26"],
    rows: [
      { key: "a", todoId: "a", title: "Write tests", cells: [0, 0, 0, 0, 0, 600_000, 600_000], totalMs: 1_200_000, completedPomodoros: 2, estimatePomodoros: 4 },
      { key: "b", todoId: "b", title: "Review", cells: [0, 0, 0, 0, 0, 0, 300_000], totalMs: 300_000, completedPomodoros: 1, estimatePomodoros: null },
      { key: "unassigned", todoId: null, title: "Unassigned", cells: [0, 0, 0, 0, 0, 0, 300_000], totalMs: 300_000, completedPomodoros: 0, estimatePomodoros: null },
    ],
    dayTotals: [0, 0, 0, 0, 0, 600_000, 1_200_000],
    totalMs: 1_800_000,
  },
};

describe("Insights", () => {
  it("renders the daily total, per-item progress, and weekly matrix", () => {
    const { getAllByText } = render(Insights, { props: { analytics: populated } });
    expect(getAllByText("20m").length).toBeGreaterThan(0);
    expect(getAllByText("2 / 4").length).toBeGreaterThan(0);
    expect(getAllByText("Review").length).toBeGreaterThan(0);
    expect(getAllByText("Unassigned").length).toBeGreaterThan(0);
    expect(getAllByText("5m").length).toBeGreaterThan(0);
    expect(getAllByText("10m").length).toBeGreaterThan(0);
  });

  it("shows an empty state with no focus time", () => {
    const empty: AnalyticsSnapshot = {
      today: { date: "2026-10-26", totalMs: 0, rows: [] },
      week: { days: populated.week.days, rows: [], dayTotals: [0, 0, 0, 0, 0, 0, 0], totalMs: 0 },
    };
    const { getByText } = render(Insights, { props: { analytics: empty } });
    expect(getByText("No focus time yet")).toBeTruthy();
  });

  it("renders seven day columns with row and daily totals", () => {
    const { getByRole, getByText } = render(Insights, { props: { analytics: populated } });
    const todaySection = getByText("Today").closest("section")!;
    expect(within(todaySection).getByText("20m")).toBeTruthy();
    expect(
      within(todaySection)
        .getAllByRole("progressbar")
        .map((bar) => bar.getAttribute("aria-valuenow")),
    ).toEqual(["50", "25", "25"]);

    const table = getByRole("table");

    expect(within(table).getAllByRole("columnheader")).toHaveLength(9);
    expect(within(table).getAllByRole("rowheader")).toHaveLength(4);

    const itemRow = within(table).getByRole("rowheader", { name: "Write tests" }).closest("tr")!;
    const itemCells = within(itemRow).getAllByRole("cell");
    expect(itemCells.at(-1)?.textContent?.trim()).toBe("20m");

    const totalRow = within(table).getByRole("rowheader", { name: "Daily total" }).closest("tr")!;
    const totalCells = within(totalRow).getAllByRole("cell");
    expect(totalCells.map((cell) => cell.textContent?.trim())).toEqual([
      "0m",
      "0m",
      "0m",
      "0m",
      "0m",
      "10m",
      "20m",
      "30m",
    ]);
  });

  it("renders a todo row that Rust returns after deletion", () => {
    const withDeleted: AnalyticsSnapshot = {
      today: {
        ...populated.today,
        rows: [
          {
            key: "gone",
            todoId: "gone",
            title: "Deleted report",
            activeMs: 60_000,
            completedPomodoros: 1,
            estimatePomodoros: 2,
          },
        ],
      },
      week: {
        ...populated.week,
        rows: [
          {
            key: "gone",
            todoId: "gone",
            title: "Deleted report",
            cells: [0, 0, 0, 0, 0, 0, 60_000],
            totalMs: 60_000,
            completedPomodoros: 1,
            estimatePomodoros: 2,
          },
        ],
      },
    };
    const { getAllByText } = render(Insights, { props: { analytics: withDeleted } });
    expect(getAllByText("Deleted report")).toHaveLength(2);
  });
});
