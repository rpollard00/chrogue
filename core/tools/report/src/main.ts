import "./style.css";
import { parseBalanceText } from "./analysis";
import { mountReport } from "./app";
import { button, h } from "./dom";

const app = document.querySelector<HTMLElement>("#app");
const notice = document.querySelector<HTMLElement>("#notice");
const picker = h("input", { attrs: { type: "file", name: "file", accept: ".json,application/json", hidden: "" } });
let unmount = (): void => {};

function showError(message: string): void {
  if (!notice) return;
  notice.replaceChildren(h("span", { text: message }), button("Close", () => (notice.hidden = true), { class: "quiet" }));
  notice.hidden = false;
}

function open(json: string, name: string): void {
  if (!app) return;
  const result = parseBalanceText(json);
  if (result.kind === "error") {
    showError(`The report cannot read ${name}. ${result.message}`);
    return;
  }
  if (notice) notice.hidden = true;
  unmount();
  unmount = mountReport(app, result.dataset, () => picker.click());
}

async function openFile(file: File | undefined): Promise<void> {
  if (file) open(await file.text(), file.name);
}

function showEmpty(): void {
  app?.replaceChildren(
    h(
      "section",
      { class: "empty" },
      h("h1", { text: "Balance report" }),
      h("p", { text: "This page has no data. Drop a balance.json file on the page, or select one." }),
      button("Select a file", () => picker.click()),
    ),
  );
}

picker.addEventListener("change", () => {
  void openFile(picker.files?.[0]);
  picker.value = "";
});
document.body.append(picker);
document.addEventListener("dragover", (event) => {
  event.preventDefault();
  document.body.classList.add("dragging");
});
document.addEventListener("dragleave", () => document.body.classList.remove("dragging"));
document.addEventListener("drop", (event) => {
  event.preventDefault();
  document.body.classList.remove("dragging");
  void openFile(event.dataTransfer?.files[0]);
});

const inlined = document.querySelector("#balance-data")?.textContent?.trim() ?? "";
showEmpty();
if (inlined) open(inlined, "the data of this page");
