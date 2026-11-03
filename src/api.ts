// Backend calls and the shared state the pages render from.

import { invoke } from "@tauri-apps/api/core";
import { toast } from "./ui";

export type Account = {
  id: string;
  name: string;
  is_default: boolean;
  data_dir: string;
  email: string | null;
  organization: string | null;
};

export type Folder = {
  id: string;
  name: string;
  path: string;
  /// Serialized as `profile_id` by the backend for data compatibility.
  profile_id: string;
};

export type ShellInfo = {
  /** "zsh", "bash" or "PowerShell"; null when the user's shell isn't supported. */
  shell: string | null;
  /** The startup file the hook line goes into, e.g. "~/.zshrc". */
  file: string | null;
  installed: boolean;
};

export type PlatformInfo = {
  os: "macos" | "windows" | "linux";
  claude_installed: boolean;
  claude_app_installed: boolean;
  shell: ShellInfo;
};

export const state = {
  accounts: [] as Account[],
  platform: {
    os: "macos",
    claude_installed: true,
    claude_app_installed: false,
    shell: { shell: null, file: null, installed: false },
  } as PlatformInfo,
  /// Re-render everything; set by main.ts.
  refresh: () => {},
};

/// Call a backend command. Errors are shown as a toast and return undefined.
export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T | undefined> {
  try {
    return (await invoke<T>(cmd, args)) ?? (null as T);
  } catch (e) {
    toast(String(e), true);
    return undefined;
  }
}

/// What the OS calls its bin.
export const trashName = () => (state.platform.os === "windows" ? "Recycle Bin" : "Trash");

/// A <select> of accounts, optionally leaving one out.
export function accountSelect(selected: string, exclude?: string): HTMLSelectElement {
  const sel = document.createElement("select");
  for (const a of state.accounts.filter((a) => a.id !== exclude)) {
    const opt = document.createElement("option");
    opt.value = a.id;
    opt.textContent = a.name;
    opt.selected = a.id === selected;
    sel.append(opt);
  }
  return sel;
}
