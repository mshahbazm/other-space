# Licensing

Other is open source under the GPL-3.0. The official builds (from Releases)
come with a 7-day free trial and then ask you to buy a license. Buying
supports development, and official builds get updates. Nothing in the app is
ever locked: after the trial it keeps working and shows a small reminder.

## Building it yourself, free

Builds made with the `free` feature skip licensing entirely:

```sh
npm install
npm run build:free   # installers under src-tauri/target/release/bundle
npm run dev:free     # run it during development
```

A plain `npm run tauri build` makes the same build as the official one, with
the trial and license prompt.

## How the app handles licenses

- **Trial:** starts on first launch and lasts 7 days. Afterwards a bar at
  the bottom of the window and items in the menu bar point to "Buy License"
  and "Enter Key…".
- **Activating:** the key is sent to the licensing API together with a
  random per-install device id and the computer's name, so the buyer can see
  and manage their activated computers.
- **Re-checking:** a licensed install re-checks its key about once a week in
  the background. Only a clear "not valid" answer (refund, revoked) removes
  the license; being offline never does.
- **Moving to another computer:** "Manage… → Deactivate This Computer"
  frees the activation.
- **Storage:** `license.json` in the app's data folder (trial start, device
  id, key, activation id, email). The app contains no secrets.

Build-time overrides (for testing against a local API):
`OTHER_LICENSE_API` (default `https://licensing.xvelopers.com`) and
`OTHER_BUY_URL` (default `https://ai.xvelopers.com/other`).

## Licensing API contract

The app expects these endpoints. One API can serve several apps; `product`
says which one (`"other"` for this app). All requests are `POST` with a JSON
body and `Content-Type: application/json`.

### `POST /v1/licenses/activate`

```json
{ "product": "other", "key": "…", "device_id": "uuid", "device_name": "Alex's MacBook" }
```

| Status | Meaning | Body |
|---|---|---|
| 200 | Activated (or this device was already activated: return the same activation) | `{ "activation_id": "…", "email": "buyer@example.com" }` (`email` optional) |
| 404 | Unknown key (or key for another product) | any |
| 403 | Key already active on its maximum number of devices | any |
| 410 | Key refunded or revoked | any |

### `POST /v1/licenses/validate`

```json
{ "product": "other", "key": "…", "activation_id": "…" }
```

| Status | Meaning | Body |
|---|---|---|
| 200 | Checked | `{ "valid": true, "email": "buyer@example.com" }`; `valid: false` removes the license |
| 404 / 410 | Unknown activation, or key revoked: the app removes the license | any |
| other | Temporary problem: the app keeps the license and tries again later | any |

### `POST /v1/licenses/deactivate`

```json
{ "product": "other", "key": "…", "activation_id": "…" }
```

| Status | Meaning |
|---|---|
| 200 / 204 | Deactivated |
| 404 | Already gone (treated as success) |
