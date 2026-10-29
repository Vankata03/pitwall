# Splunk F1 dashboard visual source investigation

- Researched: 2026-10-29.
- Scope: public source inspection and visual comparison; no implementation, copied upstream code, tracker writes, installations, commits or pushes.
- Local reference: supplied photo `b0c0855e-af50-4be8-9751-4fd67379247c-07900a03-fa04-4f9a-82b6-47f2b2fb7c65.jpg`.
- Local implementation inspected: [dashboard-refined-prototype.html](../prototypes/dashboard-refined-prototype.html).
- Preserved requirements: temperature B; prominent tyre wear; one speed gauge, km/h by default with mph setting; combined throttle/brake graph; SM label.
- Planning context: [pitwall map #1](https://github.com/Vankata03/pitwall/issues/1). Its clean-room commitment explicitly concerns pits-n-giggles. This investigation does not broaden that commitment or copy Splunk implementation into pitwall.

## Result and confidence

| Finding | Classification | Evidence / boundary |
| --- | --- | --- |
| Robert Castley built a custom F1 dashboard inside Dashboard Studio using JavaScript and Canvas, including arc gauges, tyre temperatures, ERS and lap timing. | Confirmed first-party description | [Splunk event](https://usergroups.splunk.com/events/details/splunk-netherlands-splunk-user-group-presents-canvas-code-amp-claude-how-to-build-purpose-built-splunk-visualizations-from-scratch/). Recording linked there; recording not inspected. |
| Public component repository is `rcastley/splunk-custom-visualizations`. | Confirmed | [Repository](https://github.com/rcastley/splunk-custom-visualizations), [demo entry](https://rcastley.github.io/splunk-custom-visualizations/), [legacy harness](https://rcastley.github.io/splunk-custom-visualizations/harness/legacy/). Demo execution was not verified. |
| The screenshot belongs to this exact app revision. | Unconfirmed | Matching subject, author context and ERS vocabulary establish a strong relation, not exact application provenance. No public dashboard definition matching the photograph was recovered. |
| The simulator's car illustration matches the photographed car's visual design. | Strong visual match | Same elevated front view, number 1, rear wing, Dell/Deloitte/Cisco/Rubrik/DeWalt/Pirelli placements, Google-coloured marks and ONEflight front wing. [Asset](https://github.com/splunk/f1-simulator/blob/2181ce1f2e2822ec69cf33c7e7304b0c55dba859/static/assets/images/illustration.png). No pixel identity or screenshot loading path established. |
| Public font assets include `Formula1` normal and bold. | Confirmed source fact; likely photo relation | [shared/fonts.css](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/shared/fonts.css) embeds two WOFF2 data URLs. ERS explicitly selects this family. Exact photo font and internal font metadata remain unverified. |
| The generic public gauge is the exact photographed gauge. | Not established; material differences | Public source draws a needle, spaced arc segments and system fonts. Photo appears needle-free, with a near-continuous arc and distinctive display typography. |

## Inspected revisions and navigation

| Repository / revision | Inspection |
| --- | --- |
| `rcastley/splunk-custom-visualizations` [`a11f27c5d0e3d24839bf66eb29ba97be3c9d3693`](https://github.com/rcastley/splunk-custom-visualizations/commit/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693), 2026-09-14 | Current main tree, README, license, shared font CSS, legacy manifest/harness and bounded component sources. |
| Same repository [`7c28e60e523272cff3b8da4f08f06aec38fbefc8`](https://github.com/rcastley/splunk-custom-visualizations/tree/7c28e60e523272cff3b8da4f08f06aec38fbefc8), 2026-04-27 | Pre-migration tree and generic gauge source; F1 examples live under `examples/`. Generic gauge already has system fonts and needle. |
| Same repository [`12cc0cd10419366154d7c68b75b9c8eef6b3eb9c`](https://github.com/rcastley/splunk-custom-visualizations/commit/12cc0cd10419366154d7c68b75b9c8eef6b3eb9c), 2026-09-09 | Commit metadata identifies Studio-first migration, legacy move and new native examples. |
| Same repository [`d835a789a607bb245b6cf37ffc0d6228f339eb38`](https://github.com/rcastley/splunk-custom-visualizations/commit/d835a789a607bb245b6cf37ffc0d6228f339eb38), 2026-04-02 | F1-example introduction located through component history. Not an exhaustive audit of every historical blob. |
| `splunk/f1-simulator` [`2181ce1f2e2822ec69cf33c7e7304b0c55dba859`](https://github.com/splunk/f1-simulator/commit/2181ce1f2e2822ec69cf33c7e7304b0c55dba859), 2026-10-12 | Current tree, dashboard documentation, Observability JSON, license and car illustration. |
| Same repository [`c03548aa2f0d87adae7c5f52a4883a59cd2237bd`](https://github.com/splunk/f1-simulator/tree/c03548aa2f0d87adae7c5f52a4883a59cd2237bd) | Older tree checked for dashboard packages; found documentation/assets and Observability JSON, not the photographed Studio composition. |

## Actual component paths

All current component links below are pinned to the inspected commit.

| Component | Source / assets | Match boundary |
| --- | --- | --- |
| F1 ERS | [`legacy/examples/f1_ers/.../src/visualization_source.js`](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_ers/appserver/static/visualizations/f1_ers/src/visualization_source.js); [formatter and harness directory](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_ers/appserver/static/visualizations/f1_ers) | Strong relation: percentage, segmented battery, MEDIUM badge, MGU-K/MGU-H/total/deployed row and captions match photo vocabulary and arrangement. Exact segment count differs visually. |
| F1 track information | [`legacy/examples/f1_track_info/.../src/visualization_source.js`](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_track_info/appserver/static/visualizations/f1_track_info/src/visualization_source.js); [track_splines.json](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_track_info/appserver/static/visualizations/f1_track_info/track_splines.json) | Confirmed F1 component; source renders sector-coloured silhouette, sector markers and track metadata. Photo has a road-like grey circuit with a car marker/name. Exact map component not recovered. |
| Generic gauge | [`legacy/examples/gauge/.../src/visualization_source.js`](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/gauge/appserver/static/visualizations/gauge/src/visualization_source.js) | Useful rendering reference; not proof of the photo's speed/RPM, temperature or discipline implementation. |
| Generic line chart | [`legacy/examples/line_trend_chart/.../src/visualization_source.js`](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/line_trend_chart/appserver/static/visualizations/line_trend_chart/src/visualization_source.js) | Supports multiple series, straight/smooth lines, area fill, grid and hover. No verified link to photographed throttle/brake charts. |
| Generic radar / ticker | [radar source](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/radar_chart/appserver/static/visualizations/radar_chart/src/visualization_source.js), [ticker source](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/live_ticker/appserver/static/visualizations/live_ticker/src/visualization_source.js) | Inspected as candidates; exact photographed G-force and race-control source not established. |
| Native Studio gauge | [`studio-examples/gauge/visualizations/gauge/src/render.ts`](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/studio-examples/gauge/visualizations/gauge/src/render.ts) | Located in current tree; implementation not inspected. Do not substitute its settings for legacy/photo settings. |
| Font asset registry | [shared/fonts.css](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/shared/fonts.css), [legacy manifest](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/harness/legacy/manifest.json) | Manifest loads shared CSS; Motorsport category contains only `f1_track_info` and `f1_ers`. |

**Not located in inspected public trees:** dedicated tyre-temperature component, full hero-car component, wing-damage component, lap-state component, exact G-force display, or matching Studio dashboard JSON. No evidence those were deleted from this repository; do not describe them as recovered archived components.

## Rendering details established from source

### Typography

- `f1_ers` uses `"'Formula1', sans-serif"` for labels and `"'Formula1', monospace"` for readouts; the latter remains Formula1 when loaded, not proof of a separate monospaced face. [ERS constants](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_ers/appserver/static/visualizations/f1_ers/src/visualization_source.js#L33-L34).
- Shared CSS embeds Formula1 normal/bold and Press Start 2P 400. Only Press Start 2P has an adjacent Google Fonts / SIL OFL comment; that comment does not establish Formula1 permission. [Font declarations](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/shared/fonts.css).
- The generic legacy gauge selects `sans-serif` and `monospace`, not Formula1. [Gauge constants](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/gauge/appserver/static/visualizations/gauge/src/visualization_source.js#L84-L85).
- The legacy harness uses Host Grotesk / JetBrains Mono for its own chrome and resets OpenType stylistic features within visualizations to avoid changing Formula1 glyphs. These are not evidence of dashboard fonts. [Harness](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/harness/legacy/index.html#L21-L48).
- Internal WOFF2 name/license metadata was not decoded: `fontTools` is unavailable; nothing installed.

### Generic arc gauge

- Sweep: 135°–405°, 270° total. Forty segments, 1.2° gaps, butt caps; colour interpolates between configured stops. [Arc implementation](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/gauge/appserver/static/visualizations/gauge/src/visualization_source.js#L126-L163).
- Filled-segment alpha progresses from 0.85 to 1; unfilled arc is white at 0.06 alpha. Optional glow affects the final three lit segments, blur 8/12/16. [Same implementation](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/gauge/appserver/static/visualizations/gauge/src/visualization_source.js#L126-L163).
- Major ticks: 1px, white 0.3 alpha, length 8% of outer radius; halfway minor ticks use 4% and white 0.12 alpha. Numeric tick labels clamp to 9–13px. Major interval count depends on max value. [Ticks](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/gauge/appserver/static/visualizations/gauge/src/visualization_source.js#L165-L222), [count selection](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/gauge/appserver/static/visualizations/gauge/src/visualization_source.js#L719-L729).
- Arc width clamps to 8–20px at 12% radius. Readout size clamps to 12–72px at 40% radius; bold white with optional value-coloured glow. Unit is smaller and white 0.45 alpha. A two-pixel needle and centre dot are drawn. [Geometry and text](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/gauge/appserver/static/visualizations/gauge/src/visualization_source.js#L640-L714).
- Photo boundary: do not blindly copy these constants. Photo shows five numeric anchors on speed/RPM and many intermediate ticks, unlike this generic gauge's high-range count selection. Its near-continuous colour band and oversized numerals need separate visual reconstruction.

### ERS

- Horizontal battery: 20 rectangular segments, 2px gaps, fill count rounded from charge; lit alpha 0.7→1, optional glow on final three. Source includes outline and terminal; mode badge has tinted fill and outline. [Battery](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_ers/appserver/static/visualizations/f1_ers/src/visualization_source.js#L87-L139), [horizontal layout](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_ers/appserver/static/visualizations/f1_ers/src/visualization_source.js#L329-L397).
- Medium `#00d2be`; Hotlap `#ff8700`; Overtake `#e10600`; None `#555555`. At charge ≤15%, source substitutes a red interpolation. These are source choices, not validated pitwall operating thresholds. [Constants and colour logic](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_ers/appserver/static/visualizations/f1_ers/src/visualization_source.js#L24-L34), [batteryColor](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_ers/appserver/static/visualizations/f1_ers/src/visualization_source.js#L79-L85).
- Percentage, bolt, battery and mode badge share a centreline; captions sit below. Four bottom columns use teal/blue/orange/red values and small bars below a divider. [Layout](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_ers/appserver/static/visualizations/f1_ers/src/visualization_source.js#L329-L397), [stats](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/legacy/examples/f1_ers/appserver/static/visualizations/f1_ers/src/visualization_source.js#L484-L535).

### Thermal bars: photo evidence only

- Four readout groups sit beside the corresponding wheels. Left/right groups mirror their alignment.
- Each group has BRK, SFC, INR and PSI rows: muted label, bright exact value, short blocks beneath. BRK may run green to yellow; tyre temperatures are predominantly green; PSI can be orange or green.
- No dedicated implementation or calibration recovered. Segment counts, threshold colours, ranges and warning semantics cannot be inferred reliably from one photographed state.
- Preserve selected B ribbons and wear prominence. Use the photo to improve alignment and visual density, not to invent operating ranges.

## Car artwork and composition

- [Car illustration](https://github.com/splunk/f1-simulator/blob/2181ce1f2e2822ec69cf33c7e7304b0c55dba859/static/assets/images/illustration.png) is a 1024×1024 PNG inspected visually. Its square canvas includes substantial empty space around the car; the opaque subject's bounds matter more than the file's dimensions.
- Strong visual correspondence does not prove the dashboard loads that file. The game-year/livery naming and source creator of the artwork were not established.
- Photo composition: quiet charcoal canvas; small-radius outlined groups; car dominates the middle and extends from rear-wing damage to front-wing damage. Wheel readouts sit close to physical wheel positions. Track/weather/timing form the right stack. ERS anchors the lower left. Charts form the bottom row.
- Photo has two speed units and separate throttle/brake charts. Those are intentionally superseded by the user's single-speed and combined-input decisions. Do not restore them for fidelity.
- [Simulator dashboard documentation](https://github.com/splunk/f1-simulator/blob/2181ce1f2e2822ec69cf33c7e7304b0c55dba859/content/f1-2025/dashboards.md) points Enterprise/Cloud users to `splunk/datadrivers-app`. Both public GitHub/API retrieval attempts returned HTTP 404 on 2026-10-29; private vs removed vs renamed is unknown.
- [Public Observability JSON](https://github.com/splunk/f1-simulator/blob/2181ce1f2e2822ec69cf33c7e7304b0c55dba859/observability/dashboard_group_F1_2025.json) contains `chartExports` and Observability chart configuration. It is not the recovered Studio composition; do not present it as the photo's dashboard JSON.

## License and reuse boundaries

| Material | Established | Not established / action boundary |
| --- | --- | --- |
| Component repository source | Root [Apache-2.0 license](https://github.com/rcastley/splunk-custom-visualizations/blob/a11f27c5d0e3d24839bf66eb29ba97be3c9d3693/LICENSE); README names Apache 2.0. | Any later code reuse needs normal license/notice review. No source code copied into pitwall during this task. |
| Simulator repository source | Root [Apache-2.0 license](https://github.com/splunk/f1-simulator/blob/2181ce1f2e2822ec69cf33c7e7304b0c55dba859/LICENSE). | Repository license alone does not establish the contributor's authority over embedded third-party artwork. |
| Formula1 font payloads | Embedded WOFF2; declared family and weights verified. | No asset-specific redistribution grant recovered. Do not vendor these payloads based on repository Apache alone. |
| Car/livery/sponsors/logos | Public first-party image and close visual match. | No artwork-specific license/provenance recovered. Do not redistribute as a shipped pitwall asset on this evidence. |
| Press Start 2P | CSS explicitly identifies Google Fonts and SIL OFL 1.1. | Not the observed F1 display face; irrelevant as a visual replacement. |

This is an inventory of inspected notices, not legal clearance. No conclusion about private use, streaming or redistribution is inferred from public availability.

## Ranked corrections for the local mock

These are proposed changes, not work performed. Compare against the supplied photo at the same dashboard bounds; ignore the photograph's orange surround and perspective distortion.

| Priority | Concrete gap | Proposed correction / acceptance cue |
| --- | --- | --- |
| 1 | `body`, `.dashboard` and Canvas `text()` force Arial; the photo's wide, squared numerals dominate its identity. | Establish a licensed display-face choice before spacing polish. Match numeral width, terminals and heavy weight; apply consistently to values and compact uppercase headings. Keep a readable secondary face. Formula1 is evidence of the upstream family, not permission to ship it. |
| 2 | `.car` is limited to 370×405 inside a 610px centre; local generated car is narrower and more distant than the photographed car. | Size by opaque artwork bounds; increase visible front-wing width and tyre mass, raise rear wing toward damage bar, bring front wing down toward damage labels. Preserve room for prominent wear and B ribbons. Match physical visual proportions without copying the branded image. |
| 3 | Main gauge uses a 252° sweep, stepped green/yellow/orange thirds and fixed orange value glow; discipline incorrectly shares that palette. | Reconstruct photo's near-270° dial, thin continuous-looking band, tick hierarchy and larger numeral-to-unit ratio. Give discipline its teal→light→orange treatment. Use localized, restrained value/edge glow. Do not add generic-source needle just because it exists upstream. |
| 4 | B thermal Canvas draws at 116×147 but CSS displays 116×120, compressing text and ribbons vertically. B uses 40 tiny blocks and a marker, versus short, readable photo rows. | First align backing/design/CSS aspect ratios. Keep B; enlarge row labels/values, reduce segment density, mirror right-side alignment and position each readout beside its wheel. Keep wear as the leading value; retain explicit illustrative-range disclosure until semantics are specified. |
| 5 | ERS `.energy-meta` uses 8px labels and 13px values; battery has 25 thin segments and no terminal/captions. | Rebalance percentage, battery and badge; use chunkier segments, visible battery terminal, captions and four readable coloured stats. Public source supplies the layout mechanism, not proof of exact photo dimensions. |
| 6 | `.lap-state` draws on 389px then scales a 576px coordinate system horizontally; text becomes unnaturally narrow. | Lay out at actual tile width with unscaled typography. Preserve distance strip, sector dividers and composition strip; reserve space for readable legend/value text. |
| 7 | Track is a hand-authored angular polygon; photo uses smooth road edges, deeper grey roadway and a small car/name marker. | Improve curve continuity, road edge hierarchy and marker orientation; retain schematic disclosure until an accepted game-derived outline is available. Do not import upstream spline data as a substitute for the map's recording-derived plan. |
| 8 | Labels/axes are small and crowded; chart grid dominates some mini plots. Photo makes titles and principal values much heavier than secondary scales. | Tune text hierarchy after font selection; retain the combined throttle/brake plot, distance axis and distinct cyan/red traces. Increase useful plot/text space without restoring duplicate speed or separate pedal tiles. |

## Verification, limitations and handoff

- Read project/global instructions, glossary, research skill and bounded vault context. No registered pitwall vault project found; no vault changes.
- Read GitHub map without changing it. Initial sandbox network requests failed; read-only GitHub requests succeeded with approved network access.
- Inspected pinned public sources through `gh api`; used first-party web pages for provenance. The web reader could not retrieve some pinned blob pages; corresponding blobs were read through GitHub API.
- Inspected supplied photo, upstream illustration and existing local preview. **Existing preview shows variant A and is stale relative to current source's B default.** Current-state recommendations derive from HTML/CSS/Canvas source, not a fresh browser render. No browser runtime or visual-regression checks run.
- No performance measurements, live telemetry claims, exact app identity, full historical-repository audit or rights clearance.
- Only this research note was created. Existing untracked `design-qa.md` and `docs/prototypes/` were preserved.
- Next action: address typography and visible car proportions first, then gauges and B readout geometry. Obtain the exact Studio export and missing component sources from the author if exact reconstruction remains necessary; no message sent.
