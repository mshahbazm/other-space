# Other

Run multiple Claude Code accounts side by side, for example your own Claude
subscription and one from your employer or a client. Each account keeps its
own login, chats and settings, and you pick the account from the menu bar
(system tray) instead of logging out and back in.

Works on macOS, Windows and Linux.

## Features

- **Default account, untouched.** Your normal Claude setup (`~/.claude`) is
  always listed first and Other never changes it.
- **Extra accounts.** Add "Work", "Client A"… Open Claude Code as that account
  and run `/login` once. Each row shows the email it's signed in as.
- **Claude Code in a terminal.** Start `claude` as any account. Quit it and
  the shell is still on that account.
- **Claude desktop app per account** (macOS, Windows). Open the Claude app as
  any account, each in its own window, at the same time.
- **Menu bar / system tray.** Every account, with the same actions.
- **Folders.** Pick an account for a folder (e.g. `~/work/acme` → Acme). Then
  `claude` started inside it, in VS Code, Cursor or any terminal, uses that
  account. Subfolders inherit; the most specific folder wins.
- **Copy settings** from one account to another: CLAUDE.md, settings, agents,
  commands, skills, hooks, plugins and MCP servers. Logins and chats are never
  copied.
- **Remove** an account and keep its data, or delete it (to the Trash /
  Recycle Bin) and sign it out.

## Platform support

| | macOS | Windows | Linux |
|---|---|---|---|
| Claude Code accounts | ✓ | ✓ | ✓ |
| Claude desktop app per account | ✓ | ✓ | No Claude desktop app for Linux |
| Terminal it opens | Terminal.app | Windows Terminal, else PowerShell | `$TERMINAL`, GNOME Terminal, Konsole, xfce4-terminal, kitty, Alacritty, WezTerm, foot, xterm |
| Folder switching | zsh, bash | PowerShell | zsh, bash |

Windows and Linux support is new and still being tested on real machines.

## How it works

There's no daemon or proxy, and nothing runs in the background. Quit Other
and everything keeps working.

- Each added account is a folder under `~/.multiapp/profiles/<id>/` whose
  `home/.claude` is used as `CLAUDE_CONFIG_DIR`. Claude Code keeps each
  config folder's login separately: in the macOS Keychain under
  `Claude Code-credentials-<first 8 hex of sha256(config dir)>`, and on
  Windows and Linux in a file inside the config folder.
- The Claude desktop app is started with its own `--user-data-dir` (a
  separate login for the app) and `CLAUDE_CONFIG_DIR` for its built-in
  Claude Code.
- Terminals load a small generated activate script (`activate`, or
  `activate.ps1` on Windows) before starting `claude`.
- Folder switching is optional. When turned on, Other adds one clearly marked
  line to your shell's startup file (`~/.zshrc`, `~/.bashrc` /
  `~/.bash_profile`, or your PowerShell profile) that loads a hook from
  `~/.multiapp/`.

This relies on Claude Code's `CLAUDE_CONFIG_DIR` and Electron's
`--user-data-dir`, which aren't documented as multi-account features, so a
future Claude release could change how they behave.

## Install

Download the installer for your system from
[Releases](../../releases). Official builds include a 7-day free trial, then
ask you to buy a license (the app keeps working either way). Building it
yourself with `npm run build:free` is free; see [LICENSING.md](LICENSING.md).

## Develop

Requirements: Node.js 20+, Rust (stable), and on Linux the
[Tauri system packages](https://tauri.app/start/prerequisites/).

```sh
npm install
npm run dev:free      # run the app (no license prompt)
npm run build:free    # installers under src-tauri/target/release/bundle
cd src-tauri && cargo test
```

Releases and code signing: [RELEASING.md](RELEASING.md). Licensing and the
license API: [LICENSING.md](LICENSING.md).

### Tech stack

- [Tauri 2](https://tauri.app): a Rust backend and the system's own WebView
  for the window (about 10 MB installed).
- Rust for everything that touches the system: accounts, launching apps and
  terminals, login cleanup, the shell hooks.
- TypeScript + Vite for the window, with no UI framework.

### Project layout

```
src/                  the window (TypeScript)
  main.ts             entry: tabs, buttons, rendering
  accounts.ts         Accounts page
  folders.ts          Folders page + terminal setup
  license.ts          license bar and key entry
  api.ts              backend calls, shared state
  ui.ts               small UI toolkit: buttons, menus, sheets, toast
  styles.css
src-tauri/src/        the backend (Rust)
  lib.rs              app startup, tray, window behaviour
  commands.rs         everything the window can call
  tray.rs             the menu-bar / tray menu
  accounts.rs         account list, folders, where data lives
  claude.rs           Claude Code: CLI lookup, signed-in account, login
  claude_app.rs       Claude desktop app, one instance per account
  terminal.rs         Claude Code in a terminal as an account
  shell.rs            folder switching hooks (zsh, bash, PowerShell)
  settings.rs         copy settings between accounts
  license.rs          trial and license keys (official builds)
  paths.rs            shared locations
```

## Known limitations

- Every Claude app window looks the same (same icon, same title); the icon
  comes from the Claude app itself.
- The Claude app's login is separate from Claude Code's, so an account signs
  in once in each.
- On Windows, folder switching needs PowerShell to be allowed to run scripts
  (`Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`).

## License

[GPL-3.0](LICENSE). You're free to use, build, change and share Other; if
you distribute a changed version, its source must stay open under the same
license.

Not affiliated with Anthropic.
