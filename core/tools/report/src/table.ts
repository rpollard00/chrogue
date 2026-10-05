import { toCsv, type Table } from "./analysis";
import { button, download, h } from "./dom";
import type { Sort } from "./state";

export interface Column<Row> {
  key: string;
  label: string;
  title?: string;
  numeric?: boolean;
  sort?: (row: Row) => number | string;
  cell: (row: Row) => Node | string;
}

export interface SortableTable<Row> {
  el: HTMLTableElement;
  setHead(key: string, node: Node): void;
  update(rows: readonly Row[], sort: Sort, rowClass?: (row: Row) => string): void;
  shown(): number;
}

/** A table draws this number of rows at most. The CSV of a view has each row. */
export const ROW_LIMIT = 500;

const missing = (value: number | string): boolean => typeof value === "number" && Number.isNaN(value);

function compareMissingLast(a: number | string, b: number | string, descending: boolean): number {
  if (missing(a) || missing(b)) return Number(missing(a)) - Number(missing(b));
  const order = typeof a === "number" && typeof b === "number" ? a - b : String(a).localeCompare(String(b));
  return descending ? -order : order;
}

export function sortableTable<Row>(columns: readonly Column<Row>[], onSort: (sort: Sort) => void): SortableTable<Row> {
  let current: Sort = { key: "", descending: false };
  let shown = 0;
  const body = h("tbody");
  const heads = new Map<string, { th: HTMLTableCellElement; extra: HTMLElement }>();
  const row = h("tr");
  for (const column of columns) {
    const extra = h("div", { class: "head-extra" });
    const label = column.sort
      ? button(column.label, () => onSort({ key: column.key, descending: current.key === column.key ? !current.descending : column.numeric === true }), {
          class: "sort",
          title: column.title,
        })
      : h("span", { text: column.label, title: column.title });
    const th = h("th", { class: column.numeric ? "num" : "", attrs: { scope: "col" } }, label, extra);
    heads.set(column.key, { th, extra });
    row.append(th);
  }
  const el = h("table", { class: "data" }, h("thead", {}, row), body);
  return {
    el,
    setHead: (key, node) => heads.get(key)?.extra.replaceChildren(node),
    update(rows, sort, rowClass) {
      current = sort;
      for (const [key, { th }] of heads) {
        if (key === sort.key) th.setAttribute("aria-sort", sort.descending ? "descending" : "ascending");
        else th.removeAttribute("aria-sort");
      }
      const by = columns.find((column) => column.key === sort.key)?.sort;
      const sorted = by ? [...rows].sort((a, b) => compareMissingLast(by(a), by(b), sort.descending)) : rows;
      shown = Math.min(sorted.length, ROW_LIMIT);
      body.replaceChildren(
        ...sorted.slice(0, ROW_LIMIT).map((item) =>
          h("tr", { class: rowClass?.(item) ?? "" }, ...columns.map((column) => h("td", { class: column.numeric ? "num" : "" }, column.cell(item)))),
        ),
      );
    },
    shown: () => shown,
  };
}

export function csvButton(name: string, table: () => Table): HTMLButtonElement {
  return button("Download CSV", () => download(name, toCsv(table()), "text/csv"), { class: "quiet" });
}
