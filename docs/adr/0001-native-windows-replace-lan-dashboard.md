# 0001. Native windows replace the LAN dashboard and stream view

Date: 2026-11-05
Status: Accepted

## Context

pitwall is being rewritten as one native Windows application in Rust, with no HTTP server, no wire protocol and no browser. Today the collector serves the dashboard over plain HTTP to any device on the LAN (iPad, phone, laptop) and to OBS as a browser source through stream view (`/?layout=...&stream=1`). Removing the web layer removes all of that, so the owner had to decide what, if anything, survives.

## Decision

- **Dropped:** viewing pitwall from other devices on the LAN, the HTTP server, the QR code and candidate URLs, `pitwall.local` over mDNS, the LAN trust rules, and stream view as a browser source.
- **Kept, natively:** pitwall can open several native dashboard windows, each showing one layout, on any monitor of the host. A second monitor replaces the iPad as the second screen.
- **Kept, natively:** a chrome-less stream window shows one layout with no controls, for OBS Window Capture. It uses a solid or chroma-key background. True transparency in OBS is not promised, because OBS window capture does not reliably preserve alpha.
- UDP forwarding of raw packets to configured `host:port` targets is unaffected. It is telemetry sharing, not dashboard serving.

## Consequences

- No network listener other than the UDP telemetry port. The Windows Firewall prompt guidance shrinks to the UDP port.
- The display menu becomes per-window rather than per-device. Each window keeps its own layout, focus car and units.
- Screen classes (wide, tablet, phone) survive as window-size arrangements rather than device types.
- People who used an iPad or phone as the second screen lose that path. A native LAN client is not planned; it would need a new network protocol and its own decision.
- The spec sections on finding the dashboard, LAN trust, the wire protocol, HTTP routes and stream view are rewritten or removed when the web stack is deleted.
