# 0002. gpui for the main app, a small native Win32 renderer for overlays

Date: 2026-11-05
Status: Accepted, pending the first vertical slice

## Context

The native rewrite needs a UI framework for the main window, the dashboard windows and the stream window (ADR 0001), and a way to draw in-game overlays. Overlays must be transparent, click-through, always on top and never take focus, with a move/resize mode. They must never inject into or hook the game process.

Findings, checked against Zed `main` at commit `654efae` on 2026-11-05:

- **gpui on Windows** is a first-class platform. The `gpui_windows` crate renders with DirectX 11 through DirectComposition and shapes text with DirectWrite. Zed ships stable builds on Windows. gpui is Apache-2.0.
- **Publishing:** the newest `gpui` on crates.io is 0.2.2 (2025-10-22). Zed has since split the platform code into `gpui_platform`, `gpui_windows` and others that are not on crates.io. Using current gpui means a git dependency pinned to a Zed commit and absorbing API changes when the pin moves. Unofficial republished crates exist; we do not use them.
- **Overlay fit:** `WindowKind::PopUp` creates a window with `WS_EX_TOOLWINDOW | WS_EX_TOPMOST`, and `WindowBackgroundAppearance::Transparent` works through DirectComposition. gpui sets no `WS_EX_TRANSPARENT` or `WS_EX_LAYERED`, so the window is not click-through, and its `WM_MOUSEACTIVATE` handler always returns `MA_ACTIVATE`, so a click takes focus from the game.
- **Alternatives:** egui with wgpu on winit is on crates.io, and winit supports click-through (`set_cursor_hittest(false)`), always-on-top and non-activating windows, but egui repaints the whole window every frame and its text and layout are less refined. iced is on crates.io with wgpu and multi-window support, but overlays would still need the same Win32 changes, and there is less evidence of fast custom drawing.

## Decision

- **Main app:** gpui, pinned by git commit to a Zed revision. The pin moves deliberately, in its own change, never as a side effect.
- **Overlays:** a small native Win32 window on its own thread, without a UI framework. Window styles are `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`, and drawing uses Direct2D. Move/resize mode temporarily removes `WS_EX_TRANSPARENT` so the window takes mouse input, still without activation.
- The overlay renderer reads the same in-process state as the main app. It does not open the game process, read its memory or hook its rendering.

## Consequences

- The first vertical slice must prove gpui on this product: one frame-rate tile, one slow tile, live ingest, and repaint cost at 60 Hz. If gpui fails the slice, the fallback is egui with wgpu for the main app; the overlay decision stands either way.
- Two rendering paths to maintain, but the overlay path is small, and keeping it framework-free makes the "an order of magnitude lighter than the WebView2 overlay" budget easy to meet and to measure.
- Overlay drawing code (bars, traces, best-lap ghost) is not shared with the gpui tiles. The domain logic behind it (smoothing, lap-distance alignment, lap-trace caching) lives in the UI-free core and is shared.
- Builds depend on a git source, so `Cargo.lock` pins the exact Zed commit, and the notices generator must pick up gpui and its dependencies from that source.
