# 0005. The native release supports Windows only

Date: 2026-11-05
Status: Accepted

## Context

Rewrite map #64 specifies one portable Windows application. The existing product also has macOS menu-bar and terminal behavior, platform-specific data paths and cross-platform build and release support. The owner confirmed that the native rewrite drops those paths, so the parity gate needs an explicit removal decision.

## Decision

- The native product supports Windows only. Drop macOS and Linux support, including their build and release targets, platform branches, data-path behavior and support documentation.
- Keep the existing web product building and shipped during internal native slices, as required by ADR 0004. Complete the platform removal when switching to the native product.
- Keep existing Windows recordings, layouts, configuration, track maps, livery packs and overlay settings compatible under ADR 0003.

## Consequences

Users of macOS and Linux do not receive a native replacement. A cross-platform native product is outside this rewrite's scope. DEC-02 in the parity inventory remains unchecked until the removal has implementation and verification evidence.
