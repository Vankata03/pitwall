# Pitwall Classic visual prototype review

Scope: a static composition study for “What should the dashboard look like?”. This is not product acceptance, a production implementation, or a pixel-identical reproduction.

Source: `/Users/vankata/.t3/userdata/attachments/b0c0855e-af50-4be8-9751-4fd67379247c-07900a03-fa04-4f9a-82b6-47f2b2fb7c65.jpg` (1985 × 1206 pixels, photographed dashboard with orange surround).
Implementation: `docs/prototypes/dashboard-refined-prototype.html`.
Implementation capture: `docs/prototypes/dashboard-refined-preview.png` (1500 × 949 pixels, CSS viewport width 1500, screenshot density 1).
State: player car, simulated telemetry. The 1500-pixel dashboard scales uniformly to the available width.

## Comparison evidence

Viewed the supplied source and implementation together in a two-column HTML comparison at 1600 CSS pixels. The source includes a photographed surround; the implementation is a direct render. Compared the dashboard regions rather than the surrounding photograph. Also inspected the implementation at 1500 pixels, where gauge labels, wheel readouts, typography and chart axes are readable. The inline 728-pixel render was checked separately for clipping and console errors.

## Findings and iterations

- The rejected first prototype used a flat car symbol and a generic tile grid. The revision uses a generated raster car with realistic material shading and the source's instrument/car/track composition.
- The first revised car was too narrow and overhead. A second generated asset changes the camera angle and gives the front wing and tyres more visual weight.
- Gauge endpoint labels extended beyond their available height. Reduced the gauge radius and moved the centre upward; the revised capture shows the labels within the tile.
- The wider car overlapped the wheel readouts. Reduced its width and adjusted its position; the final capture shows the readouts clear of the car.

## Required fidelity surfaces

- Typography: bold proportional numerals and small uppercase labels match the hierarchy. Arial is a system fallback; the reference's exact display font has not been identified. Exact typeface matching remains follow-up polish.
- Layout: left instrument group and ERS, open central car composition with four wheel readouts, right weather and track, lower lap strip and traces. Borders remain restrained. No clipped dashboard regions in the inspected captures.
- Colour: charcoal surfaces, papaya orange car, cyan energy/throttle, green state, red brake, blue weather/track title. Colours were inferred visually, not sampled from an original design file.
- Image: generated unbranded car raster replaces the flat placeholder. The car is sharp at the intended display size and has no visible background box. The schematic track is explicitly labelled; it is not a validated circuit outline.
- Content: simulated labels and values are disclosed. Original sponsor artwork, driver identity and Splunk branding are intentionally replaced. These mock readouts are not a telemetry contract.

## Verification

- HTML rendering at 1500 and 728 CSS pixels returned no console messages.
- A rendered script clicked the restricted-rival control, checked its state and hidden wheel values, then clicked the player-car control and checked restoration. No assertion errors appeared.
- No live game, collector, packet parsing, saved layouts, touch-device execution or performance testing took place.
- Small-screen use currently scales the desktop composition; a readable phone layout remains undecided.

## Follow-up polish and open decisions

- Owner approval of this visual direction remains pending; the Wayfinder ticket stays open.
- Identify and select the final display typeface.
- Replace the schematic circuit with a learned track outline during implementation.
- Complete the v1 tile catalogue, edit mode and layout behaviour in their own decisions.

Final result: passed for the bounded desktop composition study. This does not resolve the Wayfinder decision.

final result: passed

## Temperature presentation iteration

Owner direction: combine throttle and brake; use one speed gauge with km/h by default and mph selectable in settings; rename DRS to SM; explore different temperature presentations.

Changes:
- One distance-based input chart overlays throttle and brake with separate colours and a legend.
- One speed gauge remains. A prototype setting switches that gauge and the speed trace between km/h and mph; km/h is the default. The setting is not persisted.
- SM replaces the displayed DRS label. No telemetry interpretation for SM has been decided here.
- Engine temperature uses a numeric readout and a simulated 30-second trend.
- Wheel temperatures offer three treatments through `?variant=A`, `B`, or `C`: concentric rings, range ribbons, and small histories. Exact brake, surface and inner values remain visible in every variant.
- Temperature ranges, colours and histories are illustrative. They are not validated safe limits.

Verification:
- Inspected all three temperature variants at 1500 CSS pixels; no console messages.
- Inspected the default at 728 CSS pixels. It retains the scaled desktop composition, with the switcher below the charts.
- Rendered assertions checked one speed gauge, the km/h default, mph conversion state and trace label, A/B/C cycling and wraparound, hidden restricted temperature canvases, and the presence of one combined pedal chart. No assertion errors appeared.
- Updated `docs/prototypes/dashboard-refined-preview.png` to the latest default render (1500 × 1044).
- Owner selection of a temperature treatment remains pending. No Wayfinder resolution or production-readiness claim.

final result: passed

## Tyre wear correction

Owner identified tyre wear as a primary readout missing from the temperature study.

- Every wheel now leads with its exact wear percentage, followed by a remaining-life bar and an explicitly labelled remaining percentage. Example: 18% wear means 82% remaining.
- The same wear block remains present across all three temperature treatments; temperature detail sits below it.
- Values are simulated: FL 18%, FR 21%, RL 24%, RR 27%. No warning thresholds or strategy recommendations are implied.
- Restricted telemetry hides both wear percentages and remaining-life indicators.
- Rendered assertions verified four wear meters in each variant, correct wear/remaining complements, and hidden restricted wear. No assertion or console errors appeared.
- Desktop inspection found label crowding in the first iteration. Explicit header line heights and reduced chart height removed the overlap in the revised capture.
- Updated the stored preview. Owner acceptance remains pending.

final result: passed

## Source-informed B refinement

Scope: continue the selected B direction using the findings in `docs/research/splunk-dashboard-visual-source.md`. This remains a static prototype, not an exact recreation or an accepted product design.

Changes:
- Added Oxanium from Google Fonts as an openly licensed display face, with the original OFL notice under `docs/prototypes/assets/oxanium/`. The font is embedded in the HTML for reliable standalone rendering. Source: https://github.com/google/fonts/tree/main/ofl/oxanium . No Formula1 font payload was copied.
- Applied display typography to principal values and headings, with tabular numerals and a readable secondary face.
- Enlarged the generated car from a 370-pixel to a 440-pixel image box and widened the central workspace from 610 to 660 pixels. Retained generated, unbranded artwork.
- Rebuilt gauges with a 270-degree sweep, continuous colour transitions, major/minor tick hierarchy, larger values and a separate teal discipline palette. This is a local reconstruction, not copied upstream rendering code.
- Replaced B's stretched temperature canvas with DOM rows and 16-segment ribbons. Right-side rows mirror the left. Exact values and leading wear percentages remain readable beside each wheel.
- Rebalanced ERS typography, chunkier battery cells, terminal, captions and coloured stats.
- Rebuilt lap-state geometry at its actual display width. Corrected the remaining G-force, engine, track and chart backing/display size mismatches.
- Smoothed the schematic track. It remains illustrative, not a validated Monza outline.

Evidence and checks:
- Updated `docs/prototypes/dashboard-refined-preview.png`: 1500 × 1074 pixels, screenshot density 1, player car, B temperatures, km/h.
- Viewed the latest implementation and supplied source photograph together in a 1600-pixel two-column comparison. The photograph retains its orange surround and perspective; compared dashboard composition rather than asserting pixel equivalence.
- Inspected the implementation at 1500 pixels to review numerals, gauge ticks, temperature labels, wear values, car boundaries and chart axes. Also rendered at 728 pixels; the narrow view scales the desktop composition and is not a separate readable phone layout.
- Rendered assertions passed for embedded font loading, B default, visible canvas aspect ratios, four wear meters in A/B/C, unit switching and restricted wear/temperature visibility.
- The assertion render logged `PASS: embedded font, B default, canvas aspect ratios, wear in A/B/C, units and restricted data`. The plain 728-pixel render returned no console messages.
- `git diff --check` passed. No live-game, device, frame-rate, persistence or collector validation performed.

Remaining boundaries:
- Oxanium is a licensed substitute, not the original Formula1 face.
- Car artwork and circuit differ from the photograph. The track remains schematic.
- A/C alternatives remain available as prototype history; B is selected by default.
- Temperature ranges and histories remain simulated and illustrative. No operating thresholds were approved.
- The Wayfinder visual decision remains open for owner feedback. No commit, push or tracker resolution.

final result: passed

## Four structural layout explorations

Owner request: explore more visually distinct alternatives by restructuring and reordering the dashboard.

Artifact: `docs/prototypes/dashboard-layout-explorations.html`, with `?variant=A|B|C|D`. Previous reference-based prototype remains unchanged by this iteration.

| Variant | Composition | Primary visual hierarchy |
| --- | --- | --- |
| A — Night stage | Open car stage, large speed on the left, four-corner condition group on the right | Car, speed, tyre wear, lap data, input trace |
| B — Race desk | Timing rail, large circuit workspace, compact car, four equal condition tiles | Race position, circuit, condition, input trace |
| C — Driver cockpit | Large horizontal speed/gear/lap band, full-width input trace, lower inspection strip | Driving values, pedal inputs, condition |
| D — Technical sheet | Light engineering sheet with a condition table and companion car illustration | Cross-wheel wear/temperature comparison, session values, input trace |

Preserved across all four: exact per-wheel wear; selected B temperature ribbons (integrated into the D table); one speed value with km/h default and mph setting; SM; one combined throttle/brake trace. All data remains simulated. Layouts intentionally explore different secondary information density, rather than implementing the full v1 tile catalogue.

Visual checks:
- Inspected all four at 1440 CSS pixels, screenshot density 1. Font loaded and rendered values, image proportions, thermal ribbons, table alignment and chart axes were visible.
- Stored captures: `docs/prototypes/layout-A-preview.png`, `layout-B-preview.png`, `layout-C-preview.png`, `layout-D-preview.png`.
- This is a new direction exploration rather than a fidelity comparison against the earlier source photograph.
- Typography uses the previously vendored Oxanium font. The generated car asset is reused; no new artwork or external code was imported.
- A and C use open section boundaries, B uses compact enclosed work areas, D uses a light document/table structure. Differences affect composition and priority, not only palette.

Interaction checks:
- Rendered assertions passed for four wear meters and one combined pedal chart in every variant; no desktop board overflow; restricted values hidden in every variant; previous/next wraparound; units retained across layout changes.
- The assertion render logged `PASS: four layouts, wear, combined pedals, overflow, restricted data, cycling and units`.
- Variant keys update the URL; arrow keys switch except when an input/select/textarea/editable element has focus.
- No persistence, backend, live telemetry, device or frame-rate validation.

Owner selection or a combination remains pending. No tracker closure, commit or push.

final result: passed

## Two art directions after layout rejection

Owner requested both a dense motorsport instrument panel and a polished broadcast graphic after rejecting the previous layout set.

Artifact: `docs/prototypes/dashboard-two-directions.html`, `?variant=A|B`. B is the initial preview for comparison, not an owner-selected winner.

- A — Motorsport instruments: 270-degree speed/RPM dial; small discipline gauge; framed weather, circuit, lap and ERS cells; front-facing car; wheel-local wear and B thermal ribbons; combined pedal trace; current stint.
- B — Broadcast graphics: dark programme masthead and live-value block on a light picture area; race-order rail; a new wide three-quarter car render; persistent lower tyre-condition ribbon; combined trace and supplementary readings.
- Shared: exact wear on all four tyres; selected B thermal-ribbon treatment; one configurable speed value with km/h default; SM; combined throttle/brake. All values and temperature ranges remain illustrative.

New generated asset: `docs/prototypes/assets/papaya-broadcast.png`, generated with the built-in image tool. Prompt: photorealistic studio CGI of a complete unbranded papaya-orange and black open-wheel racing car, elevated front three-quarter camera, front wing toward lower right and rear wing upper left, landscape footprint around 2:1, transparent alpha background, realistic carbon fibre/rubber/suspension, small teal accents, no logos, text, numbers, floor or scenery. Displayed within a 650 × 355 image box without stretching. Original front-facing generated asset retained for A.

Visual evidence:
- Inspected both directions at 1440 CSS pixels, density 1. The source of visual truth is the owner's requested contrast in art direction; this is not a pixel-fidelity recreation of the original photograph.
- In A's first capture, high gauge endpoint labels were clipped. Shifted the dial centre upward and inspected the revised rendering; endpoint labels now fit.
- Inspected the new B raster in the full composition for crop, sharpness and image margins. Programme title, car, live-value block and lower tyre strip occupy separate regions.
- Captures: `docs/prototypes/direction-A-preview.png` and `direction-B-preview.png`.

Verification:
- Initial rendered assertions passed for both directions, four wear meters, one combined trace, undistorted canvases, desktop overflow, restricted states and controls.
- Final rendered assertions confirmed both image assets decode, required data remains present, mph selects 178 and km/h restores 286, restricted readings hide, and neither board overflows at its 1440-pixel design width.
- Final console output: `PASS: both car assets load, required readings, units, restricted states, no overflow`.
- No backend, live telemetry, touch-device execution, persistence or performance validation. Small previews scale the desktop compositions.
- Previous prototype artifacts remain available. No commit, push or tracker closure. Owner choice remains pending.

final result: passed

## Owner direction preference

Owner feedback: “A feels better.”

- Preferred base: A — Motorsport instruments.
- Updated `dashboard-two-directions.html` to default to A. B remains available for comparison.
- This selects an art direction for further refinement; it does not accept the complete visual design or resolve the Wayfinder ticket.
- Existing requirements remain: prominent tyre wear, B thermal ribbons, one configurable speed readout with km/h default, SM, combined throttle/brake.

## Selected A refinement

Owner direction: A feels better; proceed with refinement of that base.

Changes in `docs/prototypes/dashboard-two-directions.html`:
- Kept A as the default and enlarged the central car from 420 to 470 pixels wide. Widened the central workspace to 700 pixels and adjusted side instruments to fit the same 1440-pixel board.
- Tightened header and instrument group alignment, softened borders, increased wheel readout sizes and retained the selected B thermal-ribbon treatment.
- Added explicit remaining-life captions beneath wear bars. Wear percentages remain the leading wheel values.
- Matched wheel labels to the front-facing illustration: front axle readouts below rear axle readouts; the driver's left is on the right of the picture. Wing labels follow the same orientation.
- Removed duplicate gear from the right timing tile; gear remains below the speed dial.
- Current-stint strip now shows exactly nine completed laps, with no invented future stint length.
- Adjusted actual canvas dimensions to their display boxes; no stretched dials or circuit canvas.

Visual verification:
- Inspected the refined A at 1440 CSS pixels. Increased car bounds leave all four wear/temperature readouts and wing labels separate from the artwork.
- The first capture exposed an overriding neutral ribbon rule. Restored the lit-segment rule and inspected the updated capture, where all temperature ribbons show their intended colours.
- Updated `docs/prototypes/direction-A-preview.png` (1440 × 1096 pixels, screenshot density 1, player car, km/h).
- Rendered the scaled inline view at 728 CSS pixels; no console messages. This remains a desktop composition rather than a device-tested mobile design.

Rendered assertions passed for A default, coloured versus unlit ribbon segments, four wear meters, front-view wheel mapping, nine completed stint laps, mph/km/h switching, hidden restricted values, canvas aspect ratios and no board overflow. The same required-data and restricted-state checks also passed in B.

Console result: `PASS: A default, coloured ribbons, wear, front-view wheel mapping, nine completed laps, units, restricted states, canvas proportions and no overflow`.

`git diff --check` passed. Data, artwork and ranges remain illustrative. No live telemetry, collector, persistence or performance validation. Overall visual acceptance and the Wayfinder ticket remain open; no commit, push or tracker closure.

final result: passed

## Accepted visual direction and final tyre-condition correction

Owner accepted the current A direction for now and required tyres to start at 100 and decrease.

- Visible tyre percentages now represent remaining condition, not accumulated wear. Fresh tyres display 100%; fully worn tyres display 0%. The illustrative raw wear values 18/21/24/27 therefore display 82/79/76/73.
- The label is TYRE with CONDITION REMAINING beneath A's bars. This avoids calling an 82% remaining value 82% wear.
- Bars, meter values and accessibility text use the same remaining-condition semantics. Unknown/restricted telemetry remains unavailable; it must not appear as a fresh 100% tyre.
- Corrected both A and retained B. Historical rejected prototypes remain unchanged as primary sources.
- Rendered assertions passed for values, endpoints, meter consistency and hidden restricted states. No console errors appeared.
- Updated A's stored screenshot. Overall A direction is accepted as the planning reference, with B thermal ribbons, one configurable speed display (km/h default), SM, prominent tyre condition and combined pedals.
- This acceptance settles the visual-language decision, not the full v1 spec, live telemetry correctness, temperature limits, mobile layouts or performance.

final result: passed

## Independent display treatments within accepted A

Owner requested alternative designs for individual sections before continuing collector planning.

Artifact: `docs/prototypes/dashboard-section-studies.html`. The accepted frozen A checkpoint remains unchanged. Five selectors vary the displays independently; the URL stores the selections. The initial view shows the second treatments; Reset to A restores the accepted reference.

| Section | Reference treatment | Second treatment | Third treatment |
| --- | --- | --- | --- |
| Speed/RPM | Arc instrument | Digital speed with shift strip | Needle dial |
| Tyre temperatures | Selected B ribbons | Three vertical thermal columns | Compact numeric matrix |
| ERS | Battery segments | Circular reserve dial | Energy ledger |
| Timing | Separate timing/delta tiles | Sector card | Lap clock |
| Pedals | Overlaid traces | Mirrored traces around a common zero line | Instantaneous meters plus combined trace |

All treatments retain tyre remaining-condition semantics (100% fresh), four corner readouts, SM, configurable speed units and one combined pedal chart. The thermal ranges, sectors, histories and energy readings are simulated. No operating limits are accepted here.

Visual review:
- Inspected the reference, all-second and all-third selections at 1440 CSS pixels, density 1. Captures are `section-reference-preview.png`, `section-second-preview.png` and `section-third-preview.png` under `docs/prototypes/`.
- Fixed the needle dial's clipped upper tick label and the energy ledger's overflowing deployed row, then inspected the updated third-treatment capture.
- Corrected a font-ready callback that restored the reference markup after the custom displays rendered.
- Inspected the 728-pixel inline version. Controls remain outside the scaled dashboard and wrap independently. No mobile-device execution.

Rendered checks passed for all 15 treatments selected individually, unchanged neighbouring selections, four condition meters, one combined pedal chart, restricted-data hiding, mph switching while retaining selections, Reset to A, canvas proportions, tile overflow and board overflow. Also inspected one mixed selection. The full set of possible combinations was not exhaustively tested.

Console result: `PASS: 15 individual treatments, independent selections, condition meters, combined inputs, restricted data, units, reset and overflow`.

`git diff --check` passed. New display choices remain undecided; no tracker resolution, commit or push for this study. No collector, live telemetry, persistence or performance validation.

final result: passed

## Owner-selected component combination and timing comparison

The owner's screenshot selects this combination: shift strip, thermal columns, battery segments, sector card, and meters plus trace. These are now the defaults in `dashboard-section-studies.html`; Reset combo restores them. This supersedes the earlier B-ribbon temperature preference while preserving the accepted A dashboard composition.

Timing behaviour added:
- Completed sectors show their exact time and signed delta against the selected reference lap.
- Best lap and Previous lap are selectable from the timing tile and the external demo toolbar; both controls remain synchronized. Best lap is the default, continuing the previous personal-best reference.
- Sector references come from the same selected complete reference lap, rather than independently best sector times.
- The finished lap displays its total time and delta. The result stays visible when the next lap starts, alongside the new running lap timer.
- Comparison to best uses the best available before the completed lap. A newly fastest lap therefore retains its improvement instead of comparing against itself as zero.
- Pending/in-progress sectors do not fabricate completed-sector deltas. The unfinished lap result remains pending.
- Demo buttons expose running lap 18, lap 18 finish and lap 19 start. These are simulated presentation states, not an implemented telemetry lifecycle. The reference/phase controls are disabled when the sector-card study is not selected.

Simulated arithmetic:
- Finished sectors: 27.102, 28.491, 28.788 seconds; total 1:24.381.
- Prior best lap: sectors 27.150, 28.600, 28.815; total 1:24.565. Sector deltas −0.048, −0.109, −0.027 sum to lap delta −0.184.
- Previous lap: sectors 27.220, 28.430, 29.004; total 1:24.654. Sector deltas −0.118, +0.061, −0.216 sum to lap delta −0.273. The slower middle sector uses a positive sign and loss colour.

Verification:
- Rendered all six combinations of running/finished/next-lap and best/previous references. Checked sector counts, integer-millisecond arithmetic, sign/colour, retained result on next lap, frozen reference, synchronized selectors, phase buttons, retained component choices during unit switching and timing-tile overflow. All checks passed.
- Plain renders at 1440 and 728 CSS pixels returned no console messages.
- Captures: `timing-best-preview.png`, `timing-previous-preview.png`, `timing-next-lap-preview.png` under `docs/prototypes/`.
- The user's image is the visual reference for the selected controls. Timing additions were inspected within that same composition.
- No live-game, recorded-session, invalid-lap, missing-reference, flashback or session-reset behaviour has been implemented or verified. Those reference-lifecycle rules still need specification.

final result: passed
