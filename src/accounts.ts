// The Accounts page: one row per account, plus add / copy settings / remove.

import { type Account, accountSelect, call, state, trashName } from "./api";
import { el, iconOnly, labeled, type MenuItem, push, sheet, showMenu, toast } from "./ui";

const AVATAR_COLORS = [
  ["#5e9bff", "#2f6fe4"],
  ["#bf7bff", "#8e44e0"],
  ["#ff9f5a", "#f06a23"],
  ["#4fd18b", "#1fa463"],
  ["#ff6f91", "#e23d6a"],
  ["#4ecdd8", "#1c9fb0"],
];

function avatar(a: Account, index: number): HTMLElement {
  const v = el("div", "avatar", a.name.trim().charAt(0).toUpperCase());
  const [c1, c2] = a.is_default ? ["#a8a8ad", "#7c7c82"] : AVATAR_COLORS[(index - 1) % AVATAR_COLORS.length];
  v.style.setProperty("--c1", c1);
  v.style.setProperty("--c2", c2);
  return v;
}

export async function addAccount() {
  const input = document.createElement("input");
  input.type = "text";
  input.placeholder = "Work, Client A…";
  const choice = await sheet({
    title: "Add Account",
    body: "Give it a name. Then open Claude Code as this account and run /login to sign in.",
    build: (c) => c.append(labeled("Name", input)),
    actions: [{ label: "Add", value: "add", isDefault: true }],
  });
  if (choice !== "add" || !input.value.trim()) return;
  if ((await call("create_account", { name: input.value })) !== undefined) state.refresh();
}

async function copySettings(a: Account) {
  const src = accountSelect("default", a.id);
  const choice = await sheet({
    title: `Copy Settings to “${a.name}”`,
    body: "Copies CLAUDE.md, settings, agents, commands, skills, hooks, plugins and MCP servers. Logins and chats are never copied.",
    build: (c) => c.append(labeled("From", src)),
    actions: [{ label: "Copy", value: "copy", isDefault: true }],
  });
  if (choice !== "copy") return;
  const msg = await call<string>("copy_settings", { fromId: src.value, toId: a.id });
  if (msg !== undefined) toast(msg);
  state.refresh();
}

async function removeAccount(a: Account) {
  const choice = await sheet({
    title: `Remove “${a.name}”?`,
    body:
      "Remove takes it off the list but keeps its login, chats and settings, so adding it again brings them back. " +
      `Delete also moves its data to the ${trashName()} and signs it out.`,
    actions: [
      { label: "Delete", value: "delete", className: "destructive" },
      { label: "Remove", value: "keep", isDefault: true },
    ],
  });
  if (!choice) return;
  const msg = await call<string>("delete_account", { id: a.id, deleteData: choice === "delete" });
  if (msg !== undefined) toast(msg);
  state.refresh();
}

export function renderAccount(a: Account, index: number): HTMLElement {
  const row = el("div", "row");
  row.title = a.email ? a.data_dir : "Click Claude Code, then run /login to sign in";
  row.append(avatar(a, index));

  const text = el("div", "row-text");
  text.append(el("span", "row-title", a.name));
  // Personal plans report an org named "<email>'s Organization" — just noise.
  const org = a.organization && a.organization !== `${a.email}'s Organization` ? a.organization : null;
  text.append(
    a.email
      ? el("span", "row-sub", org ? `${a.email} · ${org}` : a.email)
      : el("span", "row-sub warn", "Not signed in"),
  );
  row.append(text);

  const actions = el("div", "row-actions");
  if (state.platform.claude_app_installed) {
    const app = push("Claude", { icon: "app", title: `Open the Claude app as “${a.name}”` });
    app.onclick = () => call("open_claude_app", { id: a.id });
    actions.append(app);
  }
  const code = push("Claude Code", { icon: "terminal", title: `Open a terminal and start claude as “${a.name}”` });
  code.onclick = () => call("open_claude_code", { id: a.id });
  actions.append(code);

  if (a.is_default) {
    actions.append(el("span", "icon-only")); // keeps the buttons aligned
  } else {
    const more = iconOnly("more", "More");
    const items: MenuItem[] = [];
    if (state.accounts.length > 1) items.push({ label: "Copy Settings From…", run: () => copySettings(a) }, null);
    items.push({ label: "Remove…", danger: true, run: () => removeAccount(a) });
    more.onclick = () => showMenu(more, items);
    actions.append(more);
  }
  row.append(actions);
  return row;
}
