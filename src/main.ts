// Entry point: wires the toolbar tabs and buttons, and renders both pages.

import { type Account, call, type Folder, type PlatformInfo, state } from "./api";
import { addAccount, renderAccount } from "./accounts";
import { addFolder, renderFolder, renderShellStatus } from "./folders";
import { initLicense, refreshLicense } from "./license";
import { $, icon } from "./ui";

let lastRendered = "";

/// Re-render from the backend. With `onlyIfChanged`, skip when nothing
/// changed — re-rendering on window focus would otherwise replace the button
/// under the cursor mid-click and swallow the click.
async function refresh(onlyIfChanged = false) {
  const accounts = (await call<Account[]>("list_accounts")) ?? [];
  const folders = (await call<Folder[]>("list_folders")) ?? [];
  const snapshot = JSON.stringify([accounts, folders]);
  if (onlyIfChanged && snapshot === lastRendered) return;
  lastRendered = snapshot;
  state.accounts = accounts;
  $("#accounts").replaceChildren(...accounts.map(renderAccount));
  $("#folders").replaceChildren(...folders.map(renderFolder));
  $("#no-folders").hidden = folders.length > 0;
  renderShellStatus();
}

function initTabs() {
  const tabs = [...document.querySelectorAll<HTMLButtonElement>(".segment")];
  for (const tab of tabs) {
    tab.onclick = () => {
      for (const t of tabs) t.classList.toggle("active", t === tab);
      $("#page-accounts").hidden = tab.dataset.page !== "accounts";
      $("#page-folders").hidden = tab.dataset.page !== "folders";
    };
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  state.refresh = () => refresh();
  initTabs();
  const platform = await call<PlatformInfo>("platform_info");
  if (platform) state.platform = platform;
  document.body.classList.add(`os-${state.platform.os}`);
  $("#no-claude").hidden = state.platform.claude_installed;
  for (const id of ["#add-account", "#add-folder"]) $(id).prepend(icon("plus"));
  $("#add-account").onclick = addAccount;
  $("#add-folder").onclick = addFolder;
  // Logins happen inside Claude Code — pick up "signed in as" changes.
  window.addEventListener("focus", () => {
    refresh(true);
    refreshLicense();
  });
  refresh();
  initLicense();
});
