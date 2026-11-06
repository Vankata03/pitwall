# Native rewrite review fixes

This pass addresses the standards and spec reviews of the native rewrite based on
`05f9107`. It keeps gpui for the main app and Direct2D for the overlay. It does not
remove the web build, close issues, push branches, or access the game process.

## Implementation

The UDP receiver now publishes a fast frame after each applied datagram. Complex frame
tiles also receive a shared model at display cadence, independently of the slow model.
Slow snapshots publish after input or readout-state changes at a 200 ms cadence.
Silence publishes staleness transitions once; it does not continually advance feed
generations. Paused replay keeps its publications unchanged, including when UDP cannot
bind. Seek and replay loads publish immediately. A pending slow snapshot flushes once
after pause or end. The fast source switch holds a short mutex around writer selection;
model preparation, disk work, and slow snapshot publication stay outside it.

The dashboard prepares typed tile views and compares displayed values per tile. A
retained gpui entity caches each tile's paint. A feed update with the same view does not
invalidate that tile. Size, density, inherited style, and interactive changes can still
require new paint. This verifies the invalidation decision in native tests; GPU paint
cost and interaction with real monitors still need measurement.

Malformed overlay settings use the core's shared settings type and preservation policy.
All new quarantines end in `.bad`, including numbered copies. Status recursively discovers
new files, including `windows.json.bad` and per-window choices, and recognizes older
`.bad.N` names. Directory links are not followed. Newly discovered quarantines are logged;
status removes them when the files disappear. Existing layout backups survive migration:
new copies use the first free `.bak`, `.2.bak`, and subsequent names. Preservation uses
exclusive file creation, so a competing writer cannot replace an earlier backup.

Obsolete HTTP flags tolerate missing values and leave following active flags available
to the parser. The unrequested `--port` alias is removed. The five reviewed behavior
changes are reverted: P1 displays its bound-car interval; restricted tower gaps retain
an em dash and restricted styling; missing pit-stop times keep live or stale styling;
the extra Compare header/button is removed; and only `--udp-port` selects that port.

Shared code now owns overlay settings, file writes and preservation, compound IDs and
families, weather names, number formats, sector arithmetic, lap references, tile names
and types, and binding-key parsing. The workspace documentation lists all five crates.
The existing ticked migration, layout, editing, preset, binding, device and stream parity
rows now cite implementation files and named native tests. No new release gate is ticked.

## Review judgments retained

- Car status deliberately keeps `Inter` and its original unknown-code behavior. Stint
  keeps `Intermediate`, a soft-family `S` for compound 19, and the name `Super soft`.
  The tower keeps `SS`. These are different web presentation contracts, not conflicting
  packet interpretations. Shared compound mapping makes the distinctions explicit.
- Weather keeps `Unknown sky (N)` in its tile and `Unknown` in feed text.
- Feed lap headlines format floating-point seconds; event timestamps use whole `mm:ss`.
  Lap and sector readouts format integer milliseconds. These contexts retain their web
  rounding and zero rules instead of being forced through one formatter.
- `car_names::car_label` and `binding::car_label` remain separate adapters over the
  existing shared `label_from` rule. No naming logic is duplicated.
- `Participant.show_online_names`, `CarTarget`, and catalog `Binding` retain their public
  names. Their documented meanings and saved keys remain stable. Renaming them would
  create source churn without fixing a behavioral defect.
- Tower settings keep distinct UI declarations and storage parsers: the UI needs labels
  and a numeric choice list; the parser accepts saved JSON and clamps radius. Existing
  native tests cover their defaults, stored numeric values and round trips. Reference
  choices share the core enum and stored keys.
- Timing tower and Gaps models remain in core while the other tile view models remain
  beside their gpui renderers. All stay testable without opening a window. Moving them
  solely for symmetry would broaden this review fix.

## Verification and remaining gates

Verification uses a fresh `TEMP`/`TMP` directory under the review artifacts folder.
A repeated workspace run found three legacy collector tests reading directories left by
an earlier process with the same PID. Isolating the test temp folder avoids that stale
fixture data without deleting it or modifying another checkout.

The initial workspace attempt also lacked dashboard build assets. Running
`pnpm install --frozen-lockfile` and `pnpm build` supplied those assets. During regression
work, tests caught the previous P1 expectation and a trace-label match error; both were
corrected before final verification. A later run exposed a real recorder race: startup
repair could remove a sidecar temporary file while Pin was updating it. Cleanup now
uses the same sidecar mutex as Pin and recompression updates. The existing host-command
regression failed before that fix; the host and recorder suites passed afterward.

The explicit real-data command was:

```powershell
cargo test --release -p pitwall-core -p pitwall-native --test real_data_folder --locked -- --ignored --nocapture
```

Both tests passed using copies of the owner's `%APPDATA%/pitwall` folder: 94 files,
31 recordings, and 7,727,543 recorded datagrams. All captures reported telemetry format
2026. The native host replayed the latest Bahrain qualifying recording (438 seconds);
its copy had no changed or added non-log files. These results do not establish real
format-2025 capture coverage.

The final code checkpoint is `83e474d`; the documentation commit follows it.
The release build uses `PITWALL_BUILD_ID=83e474d` for both pnpm and Cargo.

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 764 passed, 0 failed, 8 ignored; fresh `TEMP`/`TMP` |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `pnpm build` with the release build ID | Passed |
| `cargo build --release --workspace --bins --locked` | Passed; built `pitwall-native.exe`, `pitwall-overlay-replay.exe`, `pitwall.exe`, `pitrec.exe`, and `pitrec-resend.exe` |
| Explicit real-data tests above | Both passed; the native host check is also repeated after the recorder race fix |
| `git diff --check` | Passed |
| Merge latest `native/rewrite` | Already up to date at `05f910766e4770d4f15e5ea374bcff56ac884bb2`; ancestor check passed |

The parity citation check found all 41 named tests added to the 16 reviewed ticked rows.
The Unslop pass found no phrase violations. Its soft cadence and readability flags refer
to technical documentation and the structured checklist; no prose change was warranted.
Preservation checks compare the final prose with its pre-editorial drafts.

Live-game behavior, borderless/fullscreen overlay checks, OBS capture, monitor/DPI and
accessibility checks, visual comparison with the web build, and CPU/GPU/memory release
budgets remain unproven by this pass. Existing manual gates remain open. Real-data tests
operate on temporary copies and do not modify the original data folder.
