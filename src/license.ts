// The license bar at the bottom of the window (official builds only): trial
// days left, then a gentle reminder. Nothing in the app is ever locked.

import { listen } from "@tauri-apps/api/event";
import { call } from "./api";
import { $, el, labeled, push, sheet, toast } from "./ui";

export type LicenseStatus =
  | { state: "free" }
  | { state: "trial"; days_left: number }
  | { state: "expired" }
  | { state: "licensed"; email: string | null };

let status: LicenseStatus = { state: "free" };

async function enterKey() {
  const input = document.createElement("input");
  input.type = "text";
  input.placeholder = "XXXX-XXXX-XXXX-XXXX";
  input.spellcheck = false;
  const choice = await sheet({
    title: "Enter License Key",
    body: "Paste the key from your purchase email.",
    build: (c) => c.append(labeled("License key", input)),
    actions: [{ label: "Activate", value: "go", isDefault: true }],
  });
  if (choice !== "go" || !input.value.trim()) return;
  const next = await call<LicenseStatus>("activate_license", { key: input.value });
  if (next) {
    status = next;
    render();
    toast("Thank you! Other is licensed on this computer.");
  }
}

async function manage() {
  const email = status.state === "licensed" ? status.email : null;
  const choice = await sheet({
    title: "License",
    body:
      (email ? `Licensed to ${email}. ` : "Other is licensed on this computer. ") +
      "To move the license to another computer, deactivate it here first.",
    actions: [{ label: "Deactivate This Computer", value: "deactivate", className: "destructive" }],
  });
  if (choice !== "deactivate") return;
  const next = await call<LicenseStatus>("deactivate_license");
  if (next) {
    status = next;
    render();
    toast("This computer is deactivated. The key can now be used elsewhere.");
  }
}

function render() {
  const bar = $("#license-bar");
  document.body.classList.toggle("has-license-bar", status.state !== "free");
  bar.hidden = status.state === "free";
  bar.replaceChildren();
  if (status.state === "free") return;

  const text = el("span", "license-text");
  const actions = el("div", "license-actions");
  const buy = push("Buy License", { className: status.state === "expired" ? "primary" : "" });
  buy.onclick = () => call("buy_license");
  const key = push("Enter Key…");
  key.onclick = enterKey;

  switch (status.state) {
    case "trial":
      text.textContent = `Free trial: ${status.days_left} day${status.days_left === 1 ? "" : "s"} left`;
      actions.append(key, buy);
      break;
    case "expired":
      text.textContent = "Your trial has ended. Other keeps working; a license supports it and gets you updates.";
      actions.append(key, buy);
      break;
    case "licensed": {
      text.textContent = status.email ? `Licensed to ${status.email}` : "Licensed";
      const m = push("Manage…");
      m.onclick = manage;
      actions.append(m);
      break;
    }
  }
  bar.append(text, actions);
}

export async function initLicense() {
  status = (await call<LicenseStatus>("license_status")) ?? { state: "free" };
  render();
  // "Enter License Key…" in the menu bar opens this window and the sheet.
  await listen("enter-license-key", () => enterKey());
}

/// Re-read (e.g. when the window regains focus, as the trial counts down).
export async function refreshLicense() {
  const next = await call<LicenseStatus>("license_status");
  if (next && JSON.stringify(next) !== JSON.stringify(status)) {
    status = next;
    render();
  }
}
