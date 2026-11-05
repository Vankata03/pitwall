# Release packaging notes

The portable Windows package is unsigned. Read the README for SmartScreen and Windows
Firewall guidance. Distribute THIRD-PARTY-NOTICES.md with the binary and include
its contents and the full notice below on the release page.

Tile arrangement is pointer-only. Windows tray operation, notifications, console
attachment, browser activation and start with Windows need verification on Windows.
A Rust cross-check is not a Windows build or a runtime check.

The v1 release gates in docs/spec/v1.md section 18 remain open: required captures,
latency and dropped-packet measurements. Online sessions and spectating are unverified.
OBS and device performance are unverified. Packaging does not close those gates.
The repository remains private and licence-free. Publication requires the owner's
PITWALL register search and repository licence decision.

## Notice

pitwall is unofficial and is not associated in any way with the Formula 1 companies. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trade marks of Formula One Licensing B.V.

This project is not endorsed by or affiliated with EA or its licensors.

Team names, driver names and other third-party names and marks belong to their respective owners and are used only to identify them.
