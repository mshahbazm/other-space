# Releasing

Installers are built by GitHub Actions (`.github/workflows/release.yml`) and
attached to a draft GitHub Release. Built apps are never committed to the
repository.

## Making a release

1. Set the new version in all three places:
   - `package.json` → `"version"`
   - `src-tauri/Cargo.toml` → `version`
   - `src-tauri/tauri.conf.json` → `"version"`
2. Commit, then tag and push the tag:
   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. The workflow builds on macOS, Windows and Linux (about 15–20 minutes) and
   creates a **draft** release with the installers:
   - macOS: `.dmg` (one universal build for Apple Silicon and Intel)
   - Windows: `.msi` and `-setup.exe`
   - Linux: `.AppImage`, `.deb`, `.rpm`
4. Check the draft on GitHub (Releases), add notes, and publish it.

Without signing secrets the workflow still works; the builds are just unsigned.

## Signing

Signing keys are never stored in the repository. They're added as encrypted
**repository secrets** (GitHub → Settings → Secrets and variables → Actions),
which only the release workflow can read. They aren't visible to anyone
browsing the repo, and pull requests from forks can't access them.

Whoever owns the certificates adds the secrets once; after that anyone with
permission to push a tag can make a signed release, and nobody else needs a
copy of the keys.

### macOS (Developer ID + notarization)

Needs an Apple Developer Program membership (99 USD/year).

1. In the Apple Developer portal create a **Developer ID Application**
   certificate, install it in Keychain Access, and export it (with its
   private key) as a `.p12` file with a password.
2. Create an **app-specific password** for the Apple ID at appleid.apple.com.
3. Add these secrets:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | the `.p12` file, base64-encoded: `base64 -i cert.p12 \| pbcopy` |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` export password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Company Name (TEAMID)` |
| `APPLE_ID` | the Apple ID email |
| `APPLE_PASSWORD` | the app-specific password |
| `APPLE_TEAM_ID` | the 10-character Team ID |

Delete the local `.p12` once it's uploaded.

### Windows (Azure Trusted Signing)

Since 2023, Windows code-signing keys must live in hardware or a cloud
service, so a certificate file can't simply be uploaded. Azure Trusted
Signing is the cloud option Tauri supports directly (about 10 USD/month;
check Microsoft's current eligibility rules for individuals and companies).

1. Create a Trusted Signing account and a certificate profile in Azure.
2. Create an app registration (service principal) and give it the
   *Trusted Signing Certificate Profile Signer* role on the account.
3. Add these secrets:

| Secret | Value |
|---|---|
| `AZURE_CLIENT_ID` | app registration's client ID |
| `AZURE_CLIENT_SECRET` | app registration's client secret |
| `AZURE_TENANT_ID` | Azure tenant ID |
| `AZURE_TRUSTED_SIGNING_ENDPOINT` | e.g. `https://eus.codesigning.azure.net` |
| `AZURE_TRUSTED_SIGNING_ACCOUNT` | Trusted Signing account name |
| `AZURE_TRUSTED_SIGNING_PROFILE` | certificate profile name |

Unsigned Windows builds work but show a SmartScreen warning.

### Linux

Linux packages are usually distributed unsigned; nothing to set up.

## Building and signing locally instead

Signing can also be done on your own machine, with keys that never leave it:

```sh
npm ci
# macOS: with the Developer ID certificate in your login keychain
APPLE_SIGNING_IDENTITY="Developer ID Application: …" \
APPLE_ID=… APPLE_PASSWORD=… APPLE_TEAM_ID=… \
npm run tauri build -- --target universal-apple-darwin
```

On Windows, put `{"bundle":{"windows":{"signCommand":"…"}}}` in
`src-tauri/tauri.windows.conf.json` (ignored by git) with the command for
your signing service, then run `npm run tauri build`.

Upload the files from `src-tauri/target/**/release/bundle/` to the GitHub
Release.

## Before the first public release

- Test on a real Windows machine: the Claude app launcher path
  (`%LOCALAPPDATA%\AnthropicClaude\claude.exe`) and passing it
  `--user-data-dir` haven't been verified there yet.
- Test on Linux: opening a terminal (GNOME Terminal, Konsole, etc.) and the
  bash/zsh folder switching.
