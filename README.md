# pitwall

pitwall is a telemetry dashboard for the F1® 25 game, including the 2026 Season Pack.
The collector receives UDP telemetry, records sessions and serves a dashboard to screens
on your LAN. It is free, private and has no repository licence. No installer is required.

## Run

Use the portable unsigned `pitwall.exe` for Windows x64 or `pitwall` for macOS arm64.
Keep `THIRD-PARTY-NOTICES.md` with the binary. Run it from a folder you control.
Both platforms show a tray or menu-bar icon. The dashboard opens on first successful
startup only. Launching pitwall again opens the existing dashboard and exits.

The menu shows Waiting for game or the measured receive rate, and offers Open dashboard,
Show QR, Settings, Open log folder and Quit. Quit closes the current recording. Settings
opens `/status` on the host; when startup fails, it opens `config.toml` for editing instead.
A port conflict leaves the app running with an error line and a notification naming the
port. Correct the config and the app retries. If another app owns the HTTP port, the
status page cannot be served until the conflict is fixed. CLI overrides still take precedence.
Start with Windows is opt-in, off by default, and applies to the current Windows user.

Windows SmartScreen may warn because the binary is unsigned. Use More info → Run anyway
only for a build you trust. Allow Windows Firewall access on Private networks for other
screens and incoming game telemetry. Public-network blocking can prevent access; keep
this on a trusted LAN. On macOS, Gatekeeper may require Open Anyway in System Settings →
Privacy & Security. Do not bypass these warnings for an unknown binary.

On the host, open `http://127.0.0.1:8777`. Other screens use the LAN URL or QR shown on
`/status`. Candidate URLs include each interface; the default-route interface is preferred
for QR. `http://pitwall.local:8777` is best effort: multicast filtering, host-name conflicts
or OS resolver support can prevent it. Use the IP URL if it fails. There is no login or TLS.
Host controls require a loopback URL; viewing and layout editing are open to the LAN.

## Game and screen setup

Start pitwall before the game. While running, the game holds its telemetry port exclusively on Windows, so pitwall cannot bind it if the game started first.

Enable UDP Telemetry in the game's settings. Use UDP Format 2025 or 2026, port 20777,
and a send rate of 60 Hz. Send to `127.0.0.1` if the game and collector share a PC, or
to the collector's LAN IP otherwise. The macOS collector needs a game on another machine
for live data, or a recording for replay. UDP forwarding is optional and off by default;
broadcast telemetry is an alternative when more than one app needs the game stream.

On iPad, disable Auto-Lock for a long session. Wake Lock is unavailable over plain HTTP.
Stream view uses `?layout=<name>&stream=1`; set OBS to a transparent browser source
with a custom 60 fps frame rate. A missing layout renders a blank source.

## Settings and files

Files live under `%APPDATA%\pitwall` on Windows or
`~/Library/Application Support/pitwall` on macOS: `config.toml`, `layouts.json`,
`recordings/`, `tracks/`, `liveries/` and `logs/collector.log`.
Logs are appended across launches; remove or archive old logs while pitwall is stopped.
The `instance` file is locked while running; `instance.url` holds the current local URL.
Unlocked files after exit are harmless.
The `browser-opened` marker records the first-run browser action.

Run `pitwall --console` for terminal diagnostics, or use `--help`. Supported overrides:
`--udp-port PORT`, `--http-port PORT`, `--bind ADDRESS`, `--config PATH`.
An explicitly selected config must exist. UDP and forwarding settings apply live.
HTTP address changes restart the listener and disconnect screens until they use the new
URL; the status page warns before saving. CLI overrides apply again on every reload.

```toml
[udp]
port = 20777
[http]
port = 8777
bind = "0.0.0.0"
[forward]
enabled = false
targets = []
[recording]
enabled = true
size_cap_gb = 20
[app]
start_with_windows = false
```

## Build and package

Use Node 24+, pnpm 11.24.0 and Rust 1.98.0. Install with
`pnpm install --frozen-lockfile`. Run `pnpm test`, `pnpm typecheck`, then `pnpm build`
before `cargo test --workspace`. Debug builds use build ID `dev`.

For release builds, set `PITWALL_BUILD_ID` to the source commit SHA in the environment
for **both** `pnpm build` and `cargo build --release --locked --bin pitwall`.
The release build rejects `dev`, an empty ID or a dashboard with a different ID.
The Portable release packages workflow builds Windows x64 and macOS arm64 on native
runners and uploads portable artifacts with notices, release notes and `BUILD-ID.txt`.
It does not publish a release or change repository visibility.

Regenerate dependency notices after dependency changes with
`python3 tools/notices/generate.py`. The generator covers both release targets and
installed dashboard production dependencies, including licence texts. Omitted crate
licences are cached from pinned upstream revisions; `--fetch-missing` retrieves new ones.
Do not ship real livery art. Local packs stay outside the repository and binary.

## Notice

pitwall is unofficial and is not associated in any way with the Formula 1 companies. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trade marks of Formula One Licensing B.V.

This project is not endorsed by or affiliated with EA or its licensors.

Team names, driver names and other third-party names and marks belong to their respective owners and are used only to identify them.
