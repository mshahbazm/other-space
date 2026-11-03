// Small DOM toolkit for a native-feeling macOS window: elements, icons,
// push buttons, dropdown menus, sheets and a HUD-style toast.

export function el(tag: string, className = "", text = ""): HTMLElement {
  const n = document.createElement(tag);
  if (className) n.className = className;
  if (text) n.textContent = text;
  return n;
}

export const $ = <T extends HTMLElement>(sel: string) => document.querySelector(sel) as T;

// Inline icons (stroke = currentColor, so they follow light/dark mode).
const ICONS = {
  plus: '<svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14"/></svg>',
  app: '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="3"/><path d="M3 9h18"/></svg>',
  terminal: '<svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="16" rx="3"/><path d="m7 10 3 2.5L7 15M12.5 15H17"/></svg>',
  more: '<svg viewBox="0 0 24 24"><circle cx="6" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="18" cy="12" r="1.2"/></svg>',
  trash: '<svg viewBox="0 0 24 24"><path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3"/></svg>',
  folder:
    '<svg viewBox="0 0 24 24" style="fill:currentColor;stroke:none;opacity:.85"><path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h4.6l2 2h8.4A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z"/></svg>',
};
export type IconName = keyof typeof ICONS;

export function icon(name: IconName, className = "icon"): HTMLElement {
  const i = el("span", className);
  i.innerHTML = ICONS[name];
  return i;
}

/// A macOS push button, optionally with a leading icon.
export function push(label: string, opts: { icon?: IconName; className?: string; title?: string } = {}) {
  const b = el("button", `push ${opts.className ?? ""}`.trim()) as HTMLButtonElement;
  b.type = "button";
  if (opts.icon) b.append(icon(opts.icon));
  b.append(el("span", "", label));
  if (opts.title) b.title = opts.title;
  return b;
}

/// A borderless icon-only button.
export function iconOnly(name: IconName, title: string) {
  const b = el("button", "icon-only") as HTMLButtonElement;
  b.type = "button";
  b.title = title;
  b.setAttribute("aria-label", title);
  b.append(icon(name));
  return b;
}

export function labeled(label: string, control: HTMLElement): HTMLElement {
  const l = el("label");
  l.append(el("span", "", label), control);
  return l;
}

// ---------- Toast ----------

let toastTimer: number | undefined;

export function toast(msg: string, isError = false) {
  const t = $("#toast");
  t.textContent = msg;
  t.classList.toggle("error", isError);
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => (t.hidden = true), 3200);
}

// ---------- Menus ----------

let openMenu: { menu: HTMLElement; anchor: HTMLElement } | null = null;

export function closeMenu() {
  if (!openMenu) return;
  openMenu.anchor.setAttribute("aria-expanded", "false");
  openMenu.menu.remove();
  openMenu = null;
}

export type MenuItem = { label: string; danger?: boolean; run: () => void } | null;

/// A dropdown menu under `anchor`. `null` items are separators.
export function showMenu(anchor: HTMLElement, items: MenuItem[]) {
  closeMenu();
  const menu = el("div", "menu");
  menu.setAttribute("role", "menu");
  for (const item of items) {
    if (!item) {
      menu.append(el("hr"));
      continue;
    }
    const b = el("button", item.danger ? "danger" : "", item.label) as HTMLButtonElement;
    b.type = "button";
    b.onclick = () => {
      closeMenu();
      item.run();
    };
    menu.append(b);
  }
  document.body.append(menu);
  const r = anchor.getBoundingClientRect();
  const left = Math.min(r.right - menu.offsetWidth, window.innerWidth - menu.offsetWidth - 8);
  menu.style.left = `${Math.max(8, left)}px`;
  menu.style.top = `${r.bottom + 4}px`;
  anchor.setAttribute("aria-expanded", "true");
  openMenu = { menu, anchor };
}

document.addEventListener("mousedown", (e) => {
  const t = e.target as Node;
  if (openMenu && !openMenu.menu.contains(t) && !openMenu.anchor.contains(t)) closeMenu();
});
document.addEventListener("keydown", (e) => e.key === "Escape" && closeMenu());

// ---------- Sheets ----------

/// A macOS-style sheet dropping from the toolbar. `build` fills its body.
/// Resolves with the chosen action's value, or null when cancelled.
export function sheet(opts: {
  title: string;
  body?: string;
  build?: (content: HTMLElement) => void;
  actions: { label: string; value: string; className?: string; isDefault?: boolean }[];
}): Promise<string | null> {
  return new Promise((resolve) => {
    closeMenu();
    const overlay = el("div", "sheet-overlay");
    const s = el("div", "sheet");
    s.setAttribute("role", "dialog");
    s.append(el("h3", "", opts.title));
    if (opts.body) s.append(el("p", "", opts.body));
    const content = el("div");
    s.append(content);
    opts.build?.(content);

    const done = (v: string | null) => {
      overlay.remove();
      document.removeEventListener("keydown", onKey);
      resolve(v);
    };
    const row = el("div", "sheet-actions");
    const cancel = push("Cancel");
    cancel.onclick = () => done(null);
    row.append(cancel);
    let defaultValue: string | null = null;
    for (const a of opts.actions) {
      const b = push(a.label, { className: a.className ?? (a.isDefault ? "primary" : "") });
      b.onclick = () => done(a.value);
      if (a.isDefault) defaultValue = a.value;
      row.append(b);
    }
    s.append(row);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") done(null);
      if (e.key === "Enter" && defaultValue) {
        e.preventDefault();
        done(defaultValue);
      }
    };
    document.addEventListener("keydown", onKey);
    overlay.append(s);
    document.body.append(overlay);
    (s.querySelector("input, select") as HTMLElement | null)?.focus();
  });
}
