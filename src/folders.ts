// The Folders page: folders mapped to accounts, and the one-time terminal
// shell setup that makes `claude` follow those mappings.

import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { accountSelect, call, type Folder, state } from "./api";
import { $, el, icon, iconOnly, labeled, sheet, toast } from "./ui";

export async function addFolder() {
  const picked = await openDialog({ directory: true, title: "Choose a folder for an account" });
  if (typeof picked !== "string") return;
  const name = picked.split("/").filter(Boolean).pop() ?? picked;
  const sel = accountSelect(state.accounts.find((a) => !a.is_default)?.id ?? "default");
  const choice = await sheet({
    title: `Which account should “${name}” use?`,
    body: "Claude Code started anywhere inside this folder (and its subfolders) will use it.",
    build: (c) => c.append(labeled("Account", sel)),
    actions: [{ label: "Add", value: "add", isDefault: true }],
  });
  if (choice !== "add") return;
  if ((await call("add_folder", { path: picked, accountId: sel.value })) !== undefined) {
    state.refresh();
    const { shell, installed } = state.platform.shell;
    if (shell && !installed) setUpShell();
  }
}

async function setUpShell() {
  const { shell, file } = state.platform.shell;
  const windowsNote =
    state.platform.os === "windows"
      ? " PowerShell must be allowed to run scripts (Set-ExecutionPolicy -Scope CurrentUser RemoteSigned)."
      : "";
  const choice = await sheet({
    title: "Let your terminal switch accounts",
    body:
      `Other adds one line to ${file ?? `your ${shell} profile`} so ${shell} switches to a folder's account ` +
      `when you enter it. Nothing else in your shell changes.${windowsNote}`,
    actions: [{ label: "Set Up", value: "go", isDefault: true }],
  });
  if (choice !== "go") return;
  const info = await call<typeof state.platform.shell>("install_shell_integration");
  if (info !== undefined) {
    state.platform.shell = info;
    toast("Done — open a new terminal window to use it");
    renderShellStatus();
  }
}

export function renderFolder(f: Folder): HTMLElement {
  const row = el("div", "row");
  row.append(icon("folder", "folder-icon"));

  const text = el("div", "row-text");
  text.append(el("span", "row-title", f.name), el("span", "row-sub", f.path.replace(/^\/Users\/[^/]+/, "~")));
  row.append(text);

  const actions = el("div", "row-actions");
  const sel = accountSelect(f.profile_id);
  sel.title = "Account used in this folder";
  sel.onchange = async () => {
    if ((await call("set_folder_account", { id: f.id, accountId: sel.value })) !== undefined)
      toast(`${f.name} now uses “${sel.selectedOptions[0].textContent}”`);
  };
  const code = iconOnly("terminal", "Open a terminal here and start claude");
  code.onclick = () => call("open_folder", { id: f.id });
  const del = iconOnly("trash", "Remove this folder (the folder itself isn't touched)");
  del.onclick = async () => {
    if ((await call("remove_folder", { id: f.id })) !== undefined) state.refresh();
  };
  actions.append(sel, code, del);
  row.append(actions);
  return row;
}

export function renderShellStatus() {
  const box = $("#shell-integration");
  box.replaceChildren();
  const { shell, installed } = state.platform.shell;
  if (!shell) {
    box.append("Automatic switching works with zsh, bash and PowerShell.");
  } else if (installed) {
    box.append(el("span", "status-on", "● "), `Terminal switching is on (${shell}).`);
  } else {
    const link = el("button", "link", "Set up your terminal…") as HTMLButtonElement;
    link.type = "button";
    link.onclick = setUpShell;
    box.append("One-time step: ", link);
  }
}
