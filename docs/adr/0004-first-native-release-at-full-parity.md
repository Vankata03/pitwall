# 0004. The first native release ships at full parity

Date: 2026-11-05
Status: Accepted

## Context

The native rewrite replaces every part of the product: 27 tile types, layouts, presets and edit mode, the status page, replay controls, liveries, the throttle/brake overlay and, per ADR 0001, multiple dashboard windows and a stream window. The work can ship as one release or as a smaller native slice first.

## Decision

- Work proceeds in vertical slices that merge to `main` as they pass review, behind the existing web build.
- The first public native release, and the deletion of the web stack, wait until every behaviour in the inventory is preserved or explicitly dropped by an ADR.
- No release ships both products side by side.

## Consequences

- No user loses a tile or feature when upgrading.
- The web build stays the shipped product, and keeps building in CI, until parity. Fixes to the web build during the rewrite are kept to a minimum.
- The parity inventory is the release gate. It lives on the rewrite's wayfinder map, and every item closes as either "ported, with tests" or "dropped, see ADR".
- Native slices on `main` are reachable by developers, for example through a cargo feature or a separate binary target, until the switch.
