# Which grid and rendering libraries fit 60 Hz tiles?

Research for the pitwall wayfinder ticket of the same name. Sources were retrieved on 2026-10-29. npm versions and dates come from `registry.npmjs.org`. Bundle sizes come from the Bundlephobia API (minified and gzipped size of the full package entry, before tree-shaking). Weekly downloads come from `api.npmjs.org` for 2026-10-21 to 2026-10-27.

Vocabulary follows `GLOSSARY.md`: **tile**, **layout**, **preset**, **focus car**, **collector**.

## Summary

- **Grid:** use **react-grid-layout v2** (MIT, TypeScript, hooks, responsive breakpoints, plain-JSON `{i,x,y,w,h}` layouts). Pin **2.2.4**, or 2.3.x once the open drag regression [#2303](https://github.com/react-grid-layout/react-grid-layout/issues/2303) is fixed. **gridstack.js** is the credible fallback. dnd-kit is not a grid-layout library: it has no resizing.
- **Rendering:** keep 60 Hz data **out of React state**. Write incoming telemetry into a mutable store outside React. Run **one shared `requestAnimationFrame` loop** that calls each mounted tile's imperative `draw()`. Use **Canvas 2D** for traces, the track map and needle gauges, and **DOM/SVG with transform or text-node writes** for small numeric readouts. React renders only the layout, settings and slow-changing data, through `useSyncExternalStore` on throttled, immutable snapshots. Avoid WebGL: browsers allow about 16 live contexts per page, and the data volumes here don't need it.
- **Charts:** use **uPlot** for live traces against lap distance. It is about 22 KB gzipped, MIT, and its author's benchmark shows the lowest CPU and memory when streaming at 60 fps. Write tile-specific drawing (track map, gauges, car image overlay) as small custom Canvas 2D code. Don't use ECharts (about 368 KB gzipped full), Chart.js or visx (SVG reconciled by React) on the 60 Hz path.
- **iPad Safari:** rAF drops to **30 fps in Low Power Mode** (intentional per WebKit) and does not run at 120 Hz on ProMotion screens. Canvas memory has a per-page cap. Fullscreen on iPad shows an overlay button that can't be hidden. Screen Wake Lock works from iPadOS 18.4, including Home Screen web apps. Pointer Events, ResizeObserver and OffscreenCanvas (2D since 16.4, WebGL since 17) are available.

## 1. Grid libraries

### Comparison

| | react-grid-layout | gridstack.js | dnd-kit (`@dnd-kit/react`, `@dnd-kit/core`) | dockview | react-mosaic-component |
|---|---|---|---|---|---|
| What it is | Draggable and resizable grid layout for React [1] | Framework-agnostic dashboard grid with official React, Vue and Angular wrappers [5] | Drag-and-drop toolkit: sortable lists and grids, sensors [9] | IDE-style docking: tabs, groups, floating panels, popouts [11] | Tiling window manager (binary splits) |
| Latest release | 2.3.0, 2026-10-05; 2.0.0 (TS rewrite) on 2025-12-09 [2][npm] | 14.0.0, 2026-09-21; roughly monthly 13.x releases since July 2026 [npm] | `@dnd-kit/react` 0.5.1, 2026-10-15 (still 0.x); `@dnd-kit/core` 6.3.1, 2024-12-05 [npm] | 8.8.0, 2026-10-28 [npm] | 7.2.1, 2026-09-28 [npm] |
| React 19 | Peer `react >= 16.3.0`. v2 needs React 18+ [1]. v1 already removed `ReactDOM`/`findDOMNode` use via `nodeRef` [3]. React 19 is in use in the wild ([#2268](https://github.com/react-grid-layout/react-grid-layout/issues/2268)) | Wrapper is plain React with `createPortal` [6]. The wrapper's own dev deps target React 18.3 [7]; no explicit React 19 statement | `@dnd-kit/react` peer `^18 \|\| ^19` [npm] | React adapter; peer not declared on the core package | Peer `react 16 - 19` [npm] |
| Resize | Yes. Handles `s w e n sw nw se ne`; grid default `['se']` [1] | Yes | **No.** The README lists no resizing [9] | Split panes, not free resize | Split ratios only |
| Responsive breakpoints | `Responsive` component with `breakpoints` and `cols` maps and one layout per breakpoint. Defaults: lg 1200/12, md 996/10, sm 768/6, xs 480/4, xxs 0/2 [1] | `columnOpts.breakpoints`, e.g. `[{w:768, c:1}]` [5] | n/a | n/a | n/a |
| Serializable layout | Array of `LayoutItem {i, x, y, w, h, minW?, maxW?, minH?, maxH?, static?}`; `i` matches the child key [1] | `grid.save()` / `grid.load()` JSON, with `component` and `props` per widget [5][6] | n/a | Not confirmed on the landing page [11] | Tree JSON |
| Touch / iPad | Uses react-draggable (touch and mouse events). 2.3.0 added touch drop-from-outside for iOS Safari and `dragConfig.allowMobileScroll`, because react-draggable calls `preventDefault` on `touchstart` and suppressed child `onClick` on touch devices [4] | "gridstack v6+ now support mobile out of the box": native touch and mouse, `alwaysShowResizeHandle: 'mobile'` default [5] | Pointer, mouse, touch and keyboard sensors [9] | "Touch and mobile ready" [11] | |
| React fit | Idiomatic: React owns children and layout state; `onLayoutChange` gives back the array | GridStack owns the DOM and which widgets exist; React content is portalled into `.grid-stack-item-content`. The docs advise against `items.map(<GridStackItem>)` as the source of truth [6] | You build the grid, resize and compaction yourself | Different paradigm (docking, not a dashboard grid) | Different paradigm |
| Size (min / gzip) | 78 KB / 25 KB | 92 KB / 25 KB | 100 KB / 33 KB | 384 KB / 89 KB | |
| Weekly downloads | 4.6 M | 0.66 M | `core` 33 M; `react` 2.1 M | 0.71 M | 0.17 M |
| Licence | MIT [10] | MIT [5] | MIT | MIT [npm] | Apache-2.0 [npm] |

### Notes

- **react-grid-layout v2 details** [1][2]: TypeScript rewrite with hooks `useContainerWidth`, `useGridLayout` and `useResponsiveLayout`. The `width` prop is now required (use `useContainerWidth`). `onDragStart` fires only after a 3 px move. Compaction is pluggable (`verticalCompactor`, `horizontalCompactor`, `noCompactor`). Items are positioned with CSS transforms by default (`transformStrategy`). 2.1.0 added O(n log n) compactors (author-reported 6x faster at 50 items). 2.3.0 added `GridItem` memoization and edge auto-scroll.
- **Known regression:** in 2.3.0, a dragged or resized item no longer follows the pointer between grid steps, because the memoized GridItem skips the live re-render ([#2303](https://github.com/react-grid-layout/react-grid-layout/issues/2303), open; fix PR [#2304](https://github.com/react-grid-layout/react-grid-layout/pull/2304)). 2.2.0 was flagged "Do not use" in its release notes [2]. **Pin an exact version and check the release notes before upgrading.**
- **react-draggable 4.6.0** broke RGL 1.5.3 resize and drag on React 19 ([#2268](https://github.com/react-grid-layout/react-grid-layout/issues/2268), closed 2026-07-29 with releases 1.5.4 and 2.2.4). RGL depends on `react-draggable ^4.4.6` (latest 4.7.2), so a lockfile matters.
- **Edit mode on iPad:** pitwall's grid is only editable in edit mode. Set `isDraggable`/`isResizable` false outside edit mode, so react-draggable never intercepts touches and tiles (focus-car pickers, tower rows) behave as normal touch targets. In edit mode, use a drag handle so page scrolling still works.
- **dnd-kit** would only help with "drag a tile from the palette into the grid", and RGL 2.3.0 now does that natively on touch [4].

## 2. Rendering approach for tiles updating at up to 60 Hz

### Approaches

| Approach | Good for | Cost at 60 Hz | Evidence |
|---|---|---|---|
| React state per packet (`useState`/context updated per message) | Nothing on the hot path | Reconciles every subscribed subtree 60 times a second, and the timing tower has 22 rows | Zustand's docs call transient (non-rendering) subscription a "drastic performance impact when you are allowed to mutate the view directly" [12] |
| `useSyncExternalStore` on an external store | Slow or medium data (timing tower, session info, tyre wear), with snapshots throttled to about 4–10 Hz | Re-renders when `getSnapshot` changes (`Object.is`). Snapshots must be immutable and cached. Store mutations during a transition force a blocking re-render [13] | [13] |
| Transient subscription plus imperative DOM writes (`ref.current.textContent`, `style.transform`) | Numeric readouts, a few dots or needles | Cheap if limited to `transform`/`opacity` (compositor-only) and text nodes | Only `transform` and `opacity` are compositor-only; layers cost GPU memory, which matters most on low-memory devices [17] |
| Signals (`@preact/signals-react` 3.12) | Text bindings | Passing the signal itself into JSX binds it to the DOM text node with no component re-render, but this needs a Babel transform or the `useSignals()` hook. Signals can't be DOM attributes in this version. No React 19 statement [14] | [14]. Adds a build dependency; the same effect is a few lines of code with a ref |
| SVG updated imperatively | Track map outline (static path) plus 22 car dots via `transform`; vector car image | Fine for tens of elements; poor for thousands of path points per frame | ECharts recommends canvas above about 1 k elements and SVG for lower memory on mobile [16] |
| Canvas 2D, one per tile | Traces against distance, track map dots, needle gauges | Constant cost per frame. Use `alpha:false`, layered canvases (static background and dynamic foreground), integer coordinates, DPR scaling, and draw only on change [15] | [15] |
| WebGL | Very large point clouds | Overkill here. Browsers cap active WebGL contexts at about 16 per page (oldest lost), so one context per tile is unsafe on a grid of tiles [18] | [18] |
| OffscreenCanvas in a worker | Moving heavy drawing off the main thread | Safari and iOS 16.4+ for 2D and `transferControlToOffscreen`; WebGL in OffscreenCanvas from Safari 17 [19]. Adds a worker message hop; the workload doesn't need it yet | [19] |

### Recommended architecture

1. **Telemetry store outside React.** The WebSocket `onmessage` handler writes decoded frames into mutable per-car state and fixed-size ring buffers (typed arrays) for traces. It does no React work per message. Rendering doesn't depend on the message rate, so 20 Hz and 60 Hz game send rates behave the same.
2. **One frame clock.** A single `requestAnimationFrame` loop iterates over registered tile renderers and calls `draw(now)` only on tiles whose inputs changed (dirty flag or version counter). rAF matches the display refresh rate and pauses in background tabs and hidden iframes; animation should use the timestamp, not a per-frame step [20]. Data must therefore never be "consumed" by drawing: ring buffers overwrite, so a paused tab can't build up a backlog.
3. **Tiles as `{ React shell + imperative renderer }`.** React mounts the tile chrome and settings and a `<canvas>`/`<svg>` ref, then registers `draw` with the frame clock in `useEffect`. A `ResizeObserver` on the tile resizes the canvas, including the DPR backing store [15]. This works with RGL resizing, because RGL already measures with ResizeObserver [1].
4. **Slow data through React.** The timing tower, stint and tyre data and session info use `useSyncExternalStore` against a snapshot the store publishes at about 4–10 Hz as a new immutable object, cached between publishes [13]. Twenty-two DOM rows re-rendering at 10 Hz is cheap. Text changing 60 times a second is unreadable anyway.
5. **Frame-rate independent.** Never assume 60 draws per second: Low Power Mode gives 30 fps on iOS [21], and OBS may render at its own rate [22].

## 3. Charting libraries for live traces against lap distance

| | uPlot | Chart.js | ECharts | visx | Custom Canvas 2D | webgl-plot |
|---|---|---|---|---|---|---|
| Rendering | Canvas 2D | Canvas 2D | Canvas or SVG (SVG uses a virtual DOM since 5.3) [16] | SVG through React ("d3 for math, React for DOM") [24] | Canvas 2D | WebGL |
| Latest | 1.6.32 (2025-03-14). Repo active (commits on 2026-10-27) | 4.6.1 (2026-10-22) | 6.1.0 (2026-05-19) | `@visx/xychart` 4.0.0 (2026-06-11); peer `react ^18 \|\| ^19` plus `@react-spring/web` | n/a | 1.1.2 (2026-02-16) |
| Size, min / gzip (Bundlephobia, full entry) | 51 KB / 22 KB | 205 KB / 70 KB (tree-shakable) | 1,115 KB / 368 KB (tree-shakable via `echarts/core`) | 152 KB / 50 KB | about 0 | 80 KB / 18 KB |
| Streaming evidence | Author benchmark (Chrome 113, Ryzen 7 5850U, March 2023), 3,600 points at 60 fps: **uPlot 10% CPU / 12.3 MB**, Chart.js 40% / 77 MB, ECharts 70% / 85 MB. "you can live-stream data with uPlot at 60fps", struggling "beyond 100k in-view points" [23] | Docs: `parsing:false`, `animation:false`, fixed `min`/`max`, decimation, `pointRadius:0`, `spanGaps`, OffscreenCanvas in a worker [25] | Canvas renderer recommended above about 1 k points [16] | No canvas or perf claims [24]; React reconciles the SVG per update | Full control; we own axes and hit-testing | Built for high-rate signals; one WebGL context each (see context cap) |
| X axis = distance | Numeric x; "x-values must be numbers, unique, and in ascending order". Mode 1 shares one x array across series; mode 2 keeps a separate x per series [26] | Linear scale | Value axis | Any scale | Any | Fixed x step |
| React wrapper | Third-party (`uplot-react` 1.2.4, MIT) [23]; a 30-line `useEffect` wrapper is enough | `react-chartjs-2` 5.3.1, peer React 19 | `echarts-for-react` 3.0.6 | Native React | n/a | none |
| Licence | MIT | MIT | Apache-2.0 | MIT | n/a | MIT |
| Non-goals | No animations or transitions, no built-in pan, no data aggregation [23] | | | | | |

**Recommendation:** use **uPlot** for the throttle, brake and speed traces against distance, and for any later time-series tile (delta, ERS). Its no-animation, numeric-x, canvas design is what this tile needs, and it is the lightest option with the best attributed streaming numbers. Overlaying a reference lap needs either both laps resampled onto a shared distance grid (mode 1, simplest, null-padded) or mode 2 [26]. **Custom Canvas 2D** covers the track map (22 dots on a pre-rendered track path on a background layer), the speed and RPM gauges, and the car-image overlay, none of which is a chart. Reconsider ECharts only for rich, non-live analysis views, where its size and features would pay for themselves.

## 4. iPad Safari and OBS constraints

| Constraint | Fact | Source |
|---|---|---|
| Low Power Mode | rAF throttled to 30 fps; WebKit: "This is intentional"; bug closed INVALID | [21] |
| ProMotion (120 Hz) | "Support for 120Hz requestAnimationFrame" is still NEW (open) in WebKit; the "Prefer Page Rendering Updates near 60fps" flag was removed from the menu in 2023 | [27][28] |
| Cross-origin iframes | Safari limits rAF and timers to 30 fps in iframes the user hasn't interacted with (matters if pitwall is ever embedded) | [29] |
| Canvas memory | iOS Safari caps total canvas memory per page; the reported cap varies by version and device (224–576 MB). `getContext('2d')` returns `null` past the cap; release canvases on unmount by setting width and height to 0 | [30][31] |
| WebGL contexts | Oldest context lost after about 16 active (WebKit matched other browsers) | [18] |
| Fullscreen | `requestFullscreen` on iPad only (not iPhone), from 16.4 unprefixed. Shows an overlay button that can't be disabled; swipe-down exits | [19] |
| Screen Wake Lock | iOS Safari 16.4–18.3 partial (not in Home Screen web apps); full from 18.4 | [19] |
| Pointer Events / ResizeObserver | Safari iOS 13 / 13.4+ | [19] |
| `requestIdleCallback` | Not on iOS Safari; don't rely on it for deferred work | [19] |
| OffscreenCanvas | 2D 16.4+, WebGL 17+ | [19] |
| OBS browser source | Chrome Embedded Framework. "Use custom frame rate" is off by default (30 when enabled). Default CSS gives a transparent background. "Shutdown source when not visible" is off by default | [22] |

## Open questions

1. **Distance axis monotonicity.** uPlot needs unique, ascending x [26]. Does the collector send traces already binned by lap distance (for example, 1 m or 5 m buckets per lap), or does the dashboard bin them? Lap distance near the start/finish line, and laps with resets or flashbacks, need defined handling. This belongs to "What goes over the wire?".
2. **Snapshot cadence for slow tiles.** The proposal is 4–10 Hz for the timing tower; it needs a prototype check on an iPad in Low Power Mode.
3. **RGL version.** Pin 2.2.4 now or wait for #2303 to be fixed in 2.3.x? Prototype drag and resize on a real iPad either way.
4. **Responsive breakpoints vs per-device layouts.** Layouts are already saved per device, so breakpoints mainly cover rotation and window resize. Should each layout store one RGL layout per breakpoint, or a single breakpoint with fixed `cols`?
5. **uPlot release cadence.** The last npm release was 2025-03-14 while the repo stays active. It's acceptable given its scope, but worth watching.
6. **OBS frame rate.** Measure what CEF's rAF actually runs at for an OBS scene at 60 fps, with and without "custom frame rate".

## Sources

npm metadata: `https://registry.npmjs.org/<package>`; sizes: `https://bundlephobia.com/api/size?package=<package>@<version>`; downloads: `https://api.npmjs.org/downloads/point/last-week/<package>`; GitHub stars and licences: `https://api.github.com/repos/<owner>/<repo>` (all retrieved 2026-10-29).

1. react-grid-layout README — https://github.com/react-grid-layout/react-grid-layout
2. react-grid-layout releases — https://github.com/react-grid-layout/react-grid-layout/releases
3. react-grid-layout CHANGELOG (nodeRef / ReactDOM removal) — https://github.com/react-grid-layout/react-grid-layout/blob/master/CHANGELOG.md
4. react-grid-layout CHANGELOG, 2.3.0 touch drop and `allowMobileScroll` — https://github.com/react-grid-layout/react-grid-layout/blob/master/CHANGELOG.md ; issues [#1793](https://github.com/react-grid-layout/react-grid-layout/issues/1793), [#2281](https://github.com/react-grid-layout/react-grid-layout/issues/2281)
5. gridstack.js README — https://github.com/gridstack/gridstack.js
6. gridstack React DESIGN-DECISIONS — https://github.com/gridstack/gridstack.js/blob/master/react/DESIGN-DECISIONS.md ; React wrapper — https://github.com/gridstack/gridstack.js/tree/master/react
7. gridstack React wrapper package.json — https://github.com/gridstack/gridstack.js/blob/master/react/package.json
8. react-grid-layout issues #2303, #2268, PR #2304 — https://github.com/react-grid-layout/react-grid-layout/issues/2303 , https://github.com/react-grid-layout/react-grid-layout/issues/2268 , https://github.com/react-grid-layout/react-grid-layout/pull/2304
9. dnd-kit README — https://github.com/clauderic/dnd-kit
10. GitHub API licence fields — https://api.github.com/repos/react-grid-layout/react-grid-layout
11. dockview — https://dockview.dev/
12. Zustand README, "Transient updates" — https://github.com/pmndrs/zustand
13. React docs, `useSyncExternalStore` — https://react.dev/reference/react/useSyncExternalStore
14. `@preact/signals-react` README — https://github.com/preactjs/signals/blob/main/packages/react/README.md
15. MDN, Optimizing canvas — https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API/Tutorial/Optimizing_canvas
16. ECharts handbook, Canvas vs SVG — https://echarts.apache.org/handbook/en/best-practices/canvas-vs-svg/
17. web.dev, Stick to compositor-only properties — https://web.dev/articles/stick-to-compositor-only-properties-and-manage-layer-count
18. WebKit commit "Creating a large number of WebGL contexts should recycle older contexts" — https://webkit.googlesource.com/WebKit/+/70e10861eb2fbd9d0862fd8702874c25ae5d8bfd ; Chromium review — https://codereview.chromium.org/14217005/
19. MDN browser-compat-data 8.1.7 (2026-10-22), keys `api.OffscreenCanvas*`, `api.HTMLCanvasElement.transferControlToOffscreen`, `api.WakeLock`, `api.Element.requestFullscreen`, `api.PointerEvent`, `api.ResizeObserver`, `api.Window.requestIdleCallback` — https://github.com/mdn/browser-compat-data (as rendered at https://developer.mozilla.org/en-US/docs/Web/API/OffscreenCanvas#browser_compatibility)
20. MDN, `requestAnimationFrame` — https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame
21. WebKit bug 215745, Low Power Mode 30 fps — https://bugs.webkit.org/show_bug.cgi?id=215745
22. OBS KB, Browser Source — https://obsproject.com/kb/browser-source
23. uPlot README (performance, benchmarks, non-goals) — https://github.com/leeoniya/uPlot
24. visx README — https://github.com/airbnb/visx
25. Chart.js performance docs — https://www.chartjs.org/docs/latest/general/performance.html
26. uPlot docs (data modes, x ordering) — https://github.com/leeoniya/uPlot/blob/master/docs/README.md
27. WebKit bug 173434, 120 Hz rAF — https://bugs.webkit.org/show_bug.cgi?id=173434
28. WebKit bug 257873, 60 fps preference removed from menu — https://bugs.webkit.org/show_bug.cgi?id=257873
29. Motion, "When browsers throttle requestAnimationFrame" (secondary; cites WebKit behaviour) — https://motion.dev/magazine/when-browsers-throttle-requestanimationframe
30. WebKit bug 195325, canvas memory limit — https://bugs.webkit.org/show_bug.cgi?id=195325
31. PQINA, "Total canvas memory use exceeds the maximum limit" (secondary; test case and `null` context) — https://pqina.nl/blog/total-canvas-memory-use-exceeds-the-maximum-limit/
