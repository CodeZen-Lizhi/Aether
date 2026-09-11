# Desktop Host and Gateway Contract

## Scenario: Dashboard gateway controls

### 1. Scope / Trigger

Desktop port, autostart, and directory configuration belongs in the existing System Settings page. Gateway start, stop, and restart belongs in the dashboard header status menu. Do not add a “客户端设置” menu item or permanent standalone settings page; bundled `desktop.html` is only for stopped/failed recovery.

### 2. Signatures

Reuse `desktop_status/start/stop/restart/set_port/set_autostart/open_data_dir/open_log_dir/logs/quit`. `desktop_set_port({ port: u16 })` safely stops a running gateway, saves the port, restarts, and opens the new origin.

### 3. Contracts

- `desktop-local` matches only the bundled `main` recovery window.
- `desktop-dashboard` matches `dashboard-*` and remote URLs under `http://127.0.0.1:*/*`.
- Rust `authorize` must also match the dynamic dashboard label, credential-free loopback URL, and the current managed gateway port. A capability is not sufficient runtime authorization.
- Web/Docker pages and every other localhost page have no desktop command access.

### 4. Validation & Error Matrix

- Other window label -> `此窗口没有桌面管理权限`.
- Dashboard host, credentials, or port does not match the managed origin -> deny.
- Running port save -> close old dashboard, stop child, persist, start child, open new dashboard.
- Persist/start failure after closing the dashboard -> show the bundled recovery view and preserve the actionable error.

### 5. Good/Base/Bad Cases

- Good: save `8085` in System Settings; the `8084` dashboard closes and `8085` opens.
- Base: save while stopped; only configuration changes and the next start uses it.
- Bad: grant every localhost page a capability and rely on hidden frontend controls for authorization.

### 6. Tests Required

- Vue: header shows state and running actions; System Settings verifies port, autostart, and invalid-field focus.
- Rust: `cargo check -p aether-desktop --locked`; native QA covers origin changes, stopped recovery, and IPC denial for an unmanaged page.
- UI: at 840x620 and normal desktop widths, the header does not overflow and state/focus/danger styling remains legible in both themes.

### 7. Wrong vs Correct

Wrong: the app/tray menu opens a standalone client settings page and the dashboard cannot manage its own lifecycle.

Correct: frequent lifecycle controls stay in the dashboard header, persistent configuration stays in System Settings, and the recovery view appears only when the service is unavailable.

## 1. Scope / Trigger

Apply this contract to `apps/aether-desktop`, `frontend/src/desktop`, and gateway/usage shutdown changes. The host owns one local child and its data directory. The existing management UI remains an authenticated same-origin HTTP application.

The implementation references are `src-tauri/src/{commands,config,gateway,instance,process,secrets,session,windows}.rs`, `src-tauri/src/session.js`, `apps/aether-gateway/src/lifecycle.rs`, and `crates/aether-usage/runtime/src/runtime/shutdown.rs`.

## 2. Signatures

Tauri arguments are JSON objects; status fields use snake_case:

```ts
type GatewayPhase = 'setup' | 'starting' | 'running' | 'stopping' | 'stopped' | 'failed'
interface DesktopStatus {
  phase: GatewayPhase
  configured: boolean
  port: number
  gateway_url: string
  data_dir: string
  log_dir: string
  autostart: boolean
  pid: number | null
  error: string | null
  version: string
}
```

| Command | Arguments | Result |
| --- | --- | --- |
| `desktop_status`, `desktop_start`, `desktop_stop`, `desktop_restart` | None | `DesktopStatus` |
| `desktop_set_port` | `{ port: number }` | `DesktopStatus` |
| `desktop_set_autostart` | `{ enabled: boolean }` | `DesktopStatus` |
| `desktop_logs` | None | At most 100 filtered strings |
| `desktop_open_dashboard`, `desktop_open_data_dir`, `desktop_open_log_dir`, `desktop_quit` | None | `void` |

Gateway CLI: `--app-host <IP>` (`APP_HOST`, default `0.0.0.0`), `--shutdown-timeout-seconds <1..3600>` (`AETHER_GATEWAY_SHUTDOWN_TIMEOUT_SECONDS`, default 20), and opt-in `--exit-on-stdin-close`.

Desktop session opt-in: `--desktop-mode`, with `AETHER_DESKTOP_SESSION_SECRET` supplied only through the child environment. Requires the exact IPv4 loopback listener and host EOF management. `POST /_gateway/desktop/session` accepts `X-Aether-Desktop-Session` and `X-Client-Device-Id`, validates exact Host/Origin, and returns the existing login response plus HttpOnly refresh cookie. It is unavailable by default in Web/Docker mode.

## 3. Contracts

- As of the user's 2026-09-10 instruction, commit and push new work only to the current `codex/tauri-macos` branch. Do not automatically sync `slim-personal`, and do not undo already-synchronized commits. A future cross-branch sync requires a new explicit request.
- Reuse shared gateway and management implementations for model mapping, backup and other business features. Keep desktop-specific behavior in the host, native session/UI entry points, lifecycle and packaging adapters; verify affected shared behavior together with desktop integration without inferring cross-branch publication.
- Only window `main` at the bundled launcher URL can call desktop commands. Every custom command checks both label and URL in `commands::authorize`; capabilities alone do not establish this boundary. Dashboard labels are `dashboard-<uuid>` and receive no remote IPC permission.
- The dashboard and its API requests use `http://127.0.0.1:<port>/`. Preserve Bearer access tokens, HttpOnly refresh cookies, and `X-Client-Device-Id`. `POST /api/auth/refresh` has no request body, including no `{}`.
- Create the dashboard window at `/admin/dashboard`. Desktop initialization and session recovery must not show the Web login page, including after token/Cookie expiry. The Web home guard keeps its original behavior when no native desktop session function exists.
- The settings window starts hidden and unfocused for both fresh and configured profiles. Foreground launch starts the gateway in the background and opens the dashboard when ready; background autostart leaves windows hidden. While initial startup is pending, Dock/menu/secondary-instance activation only records the request to open the dashboard. Hand off this intent atomically when startup completes, without holding the intent lock across gateway or native UI operations. Settings remain available through explicit menu actions and failure recovery.
- The host passes `--app-host 127.0.0.1 --listener-shards 1 --shutdown-timeout-seconds 20 --exit-on-stdin-close --desktop-mode`. It clears the inherited environment, allowlists system/proxy/certificate variables, and supplies production, SQLite, memory runtime, absolute static/log paths, local Cookie/CORS settings, and Keychain secrets. No user-supplied administrator password is required.
- A fresh profile starts and initializes automatically. The gateway maintains an internal data identity; reuse the sole active local administrator in existing desktop data without changing its ID or password hash. Reject ambiguous/multiple or disabled identities in non-empty user storage; do not select an arbitrary account or elevate its role.
- Each managed child gets a fresh high-entropy session capability. Keep it in process memory; never place it in settings, URLs, browser storage, or logs. Only the dashboard main frame at the exact gateway origin receives a closure-backed `window.__AETHER_DESKTOP__.authenticate(deviceId)`. Use Tauri 2.11.5 `initialization_script` plus explicit frame/origin guards; capture fetch before page scripts, fix the destination, use same-origin mode/credentials, reject redirects, and bound the request timeout. No Keychain signing/encryption key enters the page.
- The session endpoint verifies the capability in constant time, enforces exact Host/Origin and device identity, and creates a normal revocable session for the startup-selected data identity. Neither a desktop marker nor loopback source alone grants access. The capability does not authenticate proxy requests. Frontend authentication is deduplicated; failed refresh can re-establish a desktop session, while failure displays an explicit retry without login redirects or automatic retry loops.
- Build the SQLite URL from the encoded path of `Url::from_file_path`; raw path concatenation corrupts paths containing `#`, spaces, or non-ASCII characters.
- Data paths are canonical and private (directory mode 0700, settings mode 0600). `desktop.json` stores configuration, never administrator passwords or encryption/JWT keys. Keychain service is `com.aether.desktop.<first 12 SHA-256 bytes of canonical data path as hex>`, account `gateway-secrets`.
- Keychain payload is `{version: 1, jwt: string, encryption: string}`. Existing database plus missing/unreadable/invalid secrets is a failure; never generate replacement secrets in that case. Copy DB/WAL/SHM and configuration before a version upgrade while the child is stopped.
- Acquire the per-user file lock synchronously before any Tauri setup or credential/data access. Preserve the lock inode. Only the owner binds/removes the activation socket; secondary processes request activation and exit.
- Starting/stopping/settings operations are serialized. Wait for the managed child's structured `gateway_ready` output and local health response, not an arbitrary service on the same port. Never terminate processes found by port or executable name.
- Bind each dashboard to its child generation. Hold the lifecycle operation lock through connection snapshot and window creation; closing the window for stop/restart belongs to the same lock. Use a new window label when rebuilding because Tauri `destroy` is asynchronous. A watcher must recheck `failed` inside that lock before closing a window. Dispatch Reopen/activation off the UI thread so it never waits on a lock whose owner is creating a native window.
- Closing a window hides it. Explicit quit stops the gateway before exiting. Closing the host-owned stdin pipe also initiates gateway shutdown after host failure. At the normal 20-second budget, approximately 18 seconds are available for live connections, then usage persistence up to 19.5 seconds, then remaining background shutdown. The host allows 23 seconds before killing only its own child and reporting the timeout.
- Handle macOS `RunEvent::Reopen` by opening the dashboard only when `has_visible_windows` is false. Otherwise retain the visible window; bringing an open settings window to the foreground must not replace it with the dashboard.
- Track real TCP/HTTP-upgrade lifetimes. Register `UsageRuntime::track_persistence_handoff()` **before** spawning a detached terminal producer, and retain its guard through persistence handoff. An empty queue alone does not prove that usage has drained.
- Packaged builds use only sibling `Contents/MacOS/aether-gateway` and `Contents/Resources/web`. Workspace fallbacks are for unbundled debug execution only. macOS 14 is required for per-data-directory WKWebView persistent storage. External HTTP(S) navigation opens the system browser; downloads accept only the gateway origin or its blob URLs and use a safe unique filename in Downloads.
- Keep `Cargo.lock` and desktop npm lockfile. Build-time macro stripping defaults to `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=none` for the macOS 27 loader; release application optimization/stripping remains enabled. Linux workspace checks exclude `aether-desktop`; validate the native crate on macOS.
- Every `desktop.mjs build` allocates the next numeric version before compiling, synchronizing npm manifest/root lock entry, Tauri config, desktop Cargo package and its root lock entry. Increment the last component, carrying at 99; `dev` and standalone `prepare` do not allocate versions. Failed builds retain their allocated version. Never rename a DMG without changing the app/runtime version.
- Validate all five version files before writes and hold the workspace build lock through resource preparation and packaging. Release the lock on ordinary success/failure; never remove another active build's lock. Apply generated Tauri version/CFBundleVersion overrides after caller signing overrides. Build scripts do not perform Git operations; a release runner may allocate and commit the version under the same lock before invoking the shared prepare/Tauri stages.

## 4. Validation & Error Matrix

| Condition | Required behavior |
| --- | --- |
| Foreground launch, fresh or configured profile | Prepare the gateway with settings hidden; the first normal visible page is the dashboard |
| Background autostart | Keep windows hidden unless explicitly activated; report failures through the existing settings recovery |
| Activation during initial startup | Defer until ready without falling back to settings or losing the request |
| Fresh desktop profile | Start and initialize automatically; do not show account/password forms |
| Port outside 1024–65535 or already occupied | Report error; leave the occupying service untouched |
| Port change while child exists | Require stopping the child first |
| Unique existing active local administrator | Reuse its identity and password hash without prompting |
| Multiple or disabled/ambiguous identities in non-empty user storage | Report an explicit startup error; never change permissions or guess an account |
| Desktop session request without capability or with foreign Host/Origin | Reject without creating a session |
| Browser Cookie/token expired or revoked | Re-establish through the native session function; show retry on failure |
| Desktop status IPC failure with no previously decoded status | Keep the dashboard header's “启动网关” recovery action available; only gate it on an in-flight request or a known managed PID |
| Invalid settings file | Keep the failure visible after stop/retry; do not overwrite it with defaults |
| Existing DB without original Keychain record | Fail before starting; explain recovery using the original key or logical import into a new profile |
| Foreign window/origin calls IPC | Return the authorization error |
| Child exits unexpectedly | Mark `failed`, close dashboard, show launcher and logs |
| Shutdown deadline expires | Record timeout, cancel remaining work within budget; never claim complete drain |

## 5. Good / Base / Bad Cases

- Good: a fresh isolated profile initializes automatically and opens the dashboard without login, then proxies JSON/SSE through a disposable local upstream; restart retains encrypted provider credentials and usage.
- Base: existing configured profile starts the bundled gateway on loopback; closing the dashboard keeps it available; explicit quit releases the port.
- Bad: copying only an existing encrypted database into another profile must fail with missing Keychain keys; it must not silently generate new keys or clear the database.

## 6. Tests Required

- Unit: IPC payload decoding and UI state/error recovery; initial foreground/background visibility intent and activation/completion races; port boundaries; private settings and upgrade snapshot; invalid/missing key representation; bounded/redacted logs; safe navigation/download names; simultaneous lock acquisition; session origin/frame/redirect restrictions and secret rotation.
- Gateway: default CLI compatibility, IPv4/IPv6 binding, startup cancellation, signal/EOF shutdown, active HTTP/SSE and upgraded WebSocket drain, stalled HTTP/1 and HTTP/2 timeout, and terminal handoff/queue persistence.
- Integration: `scripts/qa_lifecycle.py <gateway> <web>` creates disposable SQLite and loopback upstream, checks authentication refresh/export/import/JSON/SSE/usage, and verifies EOF, SIGTERM, parent death, and socket release. Test the **bundled release** binaries/resources once; debug results alone do not validate packaging.
- Native: use the actual `.app` outside the checkout for automatic first-run, existing-profile migration, WKWebView pages/session recovery, close/hide, menu restart/quit, autostart, file dialogs/downloads, clipboard, external browser, and real Keychain persistence/failure. Ordinary browser tests do not cover these behaviors. Record blocked native checks explicitly.
- Bundle: verify deep code signature, DMG checksum/content, architecture, minimum system version, and absence of workspace dynamic-library dependencies. Do not label ad-hoc builds notarized.

## 7. Wrong vs Correct

Wrong: poll for an available port and adopt whatever process answers; kill by port at exit; decide persistence is complete because the terminal queue is currently empty.

Correct: own the `Child`, wait for its ready event plus health response, close its stdin to stop, and drain tracked live connections and producer handoffs before waiting for usage idle.

```rust
// Register before spawn so shutdown observes a producer that has not queued yet.
let handoff = usage_runtime.track_persistence_handoff();
tokio::spawn(async move {
    let _handoff = handoff;
    // Persist/enqueue the terminal outcome through the existing usage path.
});
```
