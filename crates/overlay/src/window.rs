//! The Win32 window: its own thread and message loop, drawn with Direct2D through
//! DirectComposition, with an `UpdateLayeredWindow` software fallback.
//!
//! Locked, the window is click-through (`WS_EX_TRANSPARENT`). In move mode it takes the
//! mouse and runs its own drag and resize, so the system move/size loop never activates
//! it. Nothing here opens, reads or writes the game process.

use std::{
    cell::RefCell,
    collections::HashMap,
    io,
    mem::ManuallyDrop,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use pitwall_core::{feed::Reader, traces::Polyline};
use windows::Win32::{
    Foundation::HMODULE,
    Graphics::{
        Direct2D::{
            D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
            D2D1_DEVICE_CONTEXT_OPTIONS_NONE, ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Factory1,
            ID2D1Image, ID2D1RenderTarget,
        },
        Direct3D::D3D_DRIVER_TYPE_HARDWARE,
        Direct3D11::{
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
        },
        DirectComposition::{
            DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
        },
        Dxgi::{
            Common::{DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
            CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH,
            DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISurface,
            IDXGISwapChain1,
        },
    },
    UI::WindowsAndMessaging::{LWA_ALPHA, SetLayeredWindowAttributes, WS_EX_NOREDIRECTIONBITMAP},
};
use windows::{
    Win32::{
        Foundation::{
            COLORREF, ERROR_CLASS_ALREADY_EXISTS, FILETIME, GetLastError, HANDLE, HWND, LPARAM,
            LRESULT, POINT, RECT, SIZE, WPARAM,
        },
        Graphics::{
            Direct2D::{
                Common::{
                    D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F,
                    D2D1_FIGURE_BEGIN_FILLED, D2D1_FIGURE_BEGIN_HOLLOW, D2D1_FIGURE_END_CLOSED,
                    D2D1_FIGURE_END_OPEN, D2D1_PIXEL_FORMAT,
                },
                D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_CAP_STYLE_FLAT, D2D1_DASH_STYLE_CUSTOM,
                D2D1_DASH_STYLE_SOLID, D2D1_DRAW_TEXT_OPTIONS_NONE,
                D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
                D2D1_LAYER_OPTIONS_NONE, D2D1_LAYER_PARAMETERS, D2D1_LINE_JOIN_ROUND,
                D2D1_QUADRATIC_BEZIER_SEGMENT, D2D1_RENDER_TARGET_PROPERTIES,
                D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES,
                D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1DCRenderTarget,
                ID2D1Factory, ID2D1Geometry, ID2D1Layer, ID2D1SolidColorBrush, ID2D1StrokeStyle,
            },
            DirectWrite::{
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_FEATURE,
                DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES, DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER,
                DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_RANGE, DWRITE_WORD_WRAPPING_NO_WRAP,
                DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
                IDWriteTextLayout, IDWriteTextLayout1, IDWriteTypography,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{
                AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
                CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject,
                EnumDisplayMonitors, GetMonitorInfoW, HBITMAP, HDC, HGDIOBJ, HMONITOR,
                MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint, SelectObject,
            },
        },
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, GetThreadTimes,
                INFINITE, SetWaitableTimer, TIMER_ALL_ACCESS,
            },
        },
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, GetDpiForWindow,
                MDT_EFFECTIVE_DPI, SetThreadDpiAwarenessContext,
            },
            Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWL_EXSTYLE,
                GetCursorPos, GetWindowLongPtrW, HTCLIENT, HTTRANSPARENT, HWND_TOPMOST, IDC_ARROW,
                IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, LoadCursorW,
                MA_NOACTIVATE, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
                PeekMessageW, PostMessageW, PostQuitMessage, QS_ALLINPUT, RegisterClassExW,
                SW_SHOWNOACTIVATE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                SWP_NOZORDER, SetCursor, SetWindowLongPtrW, SetWindowPos, ShowWindow,
                TranslateMessage, ULW_ALPHA, UpdateLayeredWindow, WM_APP, WM_CAPTURECHANGED,
                WM_DESTROY, WM_DPICHANGED, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE,
                WM_MOUSEMOVE, WM_NCHITTEST, WM_QUIT, WM_SETCURSOR, WNDCLASSEXW, WS_EX_LAYERED,
                WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
            },
        },
    },
    core::{BOOL, Interface, PCWSTR, Result, w},
};
use windows_numerics::{Matrix3x2, Vector2};

use crate::{
    layout::{self, Box2, Edge, Frame, Hit, Layout, RADIUS, SE_GRIP, TRACK_RADIUS},
    settings::{MIN_SIZE, OverlaySettings, Rect, place},
    view::{OverlayController, OverlayView},
};

/// Frames per second the overlay polls the feed at. It repaints only when something new
/// arrived, so an idle or paused session costs no drawing.
pub const FPS: f64 = 60.0;

const WM_SET_MOVING: u32 = WM_APP + 1;
const WM_CLOSE_OVERLAY: u32 = WM_APP + 2;

/// A running overlay window. Dropping it closes the window.
pub struct Overlay {
    hwnd: isize,
    thread: Option<JoinHandle<OverlaySettings>>,
    metrics: Arc<Counters>,
}

/// Renderer activity since opening, including the first frame. Reading never waits for
/// the renderer; counters may advance independently during a read.
#[derive(Debug, Clone, Copy)]
pub struct RenderMetrics {
    pub frames: u64,
    pub failures: u64,
}

#[derive(Default)]
struct Counters {
    frames: AtomicU64,
    failures: AtomicU64,
}

/// Open the overlay on its own thread, reading `reader`. `settings` gives the saved
/// position and size; with a `path`, leaving move mode and closing save `overlay.json`
/// there. Returns once the window exists, or the error that stopped it.
pub fn spawn(
    reader: Reader,
    settings: OverlaySettings,
    path: Option<PathBuf>,
) -> io::Result<Overlay> {
    let (ready, opened) = mpsc::channel();
    let metrics = Arc::new(Counters::default());
    let thread_metrics = metrics.clone();
    let thread = std::thread::Builder::new()
        .name("pitwall-overlay".into())
        .spawn(move || run(reader, settings, path, ready, thread_metrics))?;
    match opened.recv() {
        Ok(Ok(hwnd)) => Ok(Overlay {
            hwnd,
            thread: Some(thread),
            metrics,
        }),
        Ok(Err(error)) => {
            let _ = thread.join();
            Err(io::Error::other(error))
        }
        Err(_) => Err(io::Error::other("overlay thread stopped before opening")),
    }
}

impl Overlay {
    pub fn metrics(&self) -> RenderMetrics {
        RenderMetrics {
            frames: self.metrics.frames.load(Ordering::Relaxed),
            failures: self.metrics.failures.load(Ordering::Relaxed),
        }
    }

    /// Move mode takes the mouse so the overlay can be dragged and resized; locked passes
    /// it through. Leaving move mode saves the geometry.
    pub fn set_moving(&self, moving: bool) {
        self.post(WM_SET_MOVING, WPARAM(usize::from(moving)));
    }

    /// Close the window and return the settings to remember (also saved to the path given
    /// to [`spawn`], if any).
    pub fn close(mut self) -> OverlaySettings {
        self.shut().unwrap_or_default()
    }

    /// CPU time the overlay thread has used so far.
    pub fn cpu_time(&self) -> Duration {
        let Some(thread) = &self.thread else {
            return Duration::ZERO;
        };
        let (mut created, mut exited, mut kernel, mut user) = Default::default();
        let handle = HANDLE(thread.as_raw_handle());
        // SAFETY: the handle belongs to a live JoinHandle; the out-pointers are locals.
        let ok =
            unsafe { GetThreadTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
        if ok.is_err() {
            return Duration::ZERO;
        }
        let ticks = |t: FILETIME| u64::from(t.dwHighDateTime) << 32 | u64::from(t.dwLowDateTime);
        Duration::from_nanos((ticks(kernel) + ticks(user)) * 100)
    }

    fn post(&self, message: u32, wparam: WPARAM) {
        // SAFETY: posting to a window that may already be gone fails harmlessly.
        let _ =
            unsafe { PostMessageW(Some(HWND(self.hwnd as *mut _)), message, wparam, LPARAM(0)) };
    }

    fn shut(&mut self) -> Option<OverlaySettings> {
        let thread = self.thread.take()?;
        self.post(WM_CLOSE_OVERLAY, WPARAM(0));
        thread.join().ok()
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.shut();
    }
}

fn run(
    reader: Reader,
    settings: OverlaySettings,
    path: Option<PathBuf>,
    ready: mpsc::Sender<std::result::Result<isize, String>>,
    metrics: Arc<Counters>,
) -> OverlaySettings {
    let hwnd = match open(reader, settings, path, metrics) {
        Ok(hwnd) => hwnd,
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
            return settings;
        }
    };
    let _ = ready.send(Ok(hwnd.0 as isize));
    if let Err(error) = pump() {
        eprintln!("Overlay: {error}");
    }
    // SAFETY: our window, on its owning thread. Harmless if WM_QUIT already closed it.
    let _ = unsafe { DestroyWindow(hwnd) };
    STATE
        .with_borrow_mut(|state| state.take().map(|s| s.settings()))
        .unwrap_or(settings)
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn open(
    reader: Reader,
    settings: OverlaySettings,
    path: Option<PathBuf>,
    metrics: Arc<Counters>,
) -> Result<HWND> {
    // SAFETY: plain Win32 calls on this thread with valid arguments.
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: w!("pitwall-overlay"),
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            return Err(windows::core::Error::from_thread());
        }

        let (monitors, primary, scale) = monitors();
        let placed = place(&settings, &monitors, primary, scale);
        let frame = Frame {
            x: placed.position.0,
            y: placed.position.1,
            width: placed.size.0,
            height: placed.size.1,
        };
        let preferred = Mode::from_env();
        let mut modes = vec![preferred];
        if preferred == Mode::Composition {
            // Fall back to the layered surface if composition is unavailable.
            modes.push(Mode::Layered { software: true });
        }
        let mut failure = None;
        let mut opened = None;
        for mode in modes {
            let mut ex_style = WS_EX_LAYERED
                | WS_EX_TRANSPARENT
                | WS_EX_TOPMOST
                | WS_EX_NOACTIVATE
                | WS_EX_TOOLWINDOW;
            if mode == Mode::Composition {
                ex_style |= WS_EX_NOREDIRECTIONBITMAP;
            }
            let hwnd = CreateWindowExW(
                ex_style,
                w!("pitwall-overlay"),
                w!("pitwall overlay"),
                WS_POPUP,
                frame.x,
                frame.y,
                frame.width as i32,
                frame.height as i32,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            if mode == Mode::Composition {
                // A layered window without UpdateLayeredWindow shows nothing until it has
                // an alpha; the composition visual then supplies per-pixel alpha.
                if let Err(error) = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) {
                    let _ = DestroyWindow(hwnd);
                    failure = Some(error);
                    continue;
                }
            }
            let graphics = match Graphics::new(mode, frame) {
                Ok(graphics) => graphics,
                Err(error) => {
                    let _ = DestroyWindow(hwnd);
                    failure = Some(error);
                    continue;
                }
            };
            let mut state = State {
                hwnd,
                controller: OverlayController::new(reader.clone()),
                frame,
                dpi: GetDpiForWindow(hwnd).max(96),
                moving: false,
                drag: None,
                dirty: true,
                topmost_at: Instant::now(),
                profile: std::env::var_os("PITWALL_OVERLAY_PROFILE")
                    .map(|_| (Duration::ZERO, 0, Instant::now())),
                path: path.clone(),
                graphics,
                metrics: metrics.clone(),
            };
            state.poll();
            match state.present() {
                Ok(()) => {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    opened = Some(state);
                    break;
                }
                Err(error) => {
                    eprintln!("Overlay: {mode:?} rendering failed: {error}");
                    let _ = DestroyWindow(hwnd);
                    failure = Some(error);
                }
            }
        }
        let Some(state) = opened else {
            return Err(failure.unwrap_or_else(windows::core::Error::empty));
        };
        let hwnd = state.hwnd;
        STATE.with_borrow_mut(|slot| *slot = Some(state));
        Ok(hwnd)
    }
}

/// The message loop: pump messages, poll the feed once per frame, sleep on a
/// high-resolution timer in between.
fn pump() -> Result<()> {
    // SAFETY: Win32 calls on this thread; `msg` and `due` are locals.
    unsafe {
        let timer = CreateWaitableTimerExW(
            None,
            PCWSTR::null(),
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
            TIMER_ALL_ACCESS.0,
        )?;
        // SAFETY: this successful creation transfers a unique, owned kernel handle.
        // Keep it alive through the loop and close it on every exit path.
        let _timer_owner = OwnedHandle::from_raw_handle(timer.0);
        let period = Duration::from_secs_f64(1.0 / FPS);
        let mut next = Instant::now();
        let mut msg = MSG::default();
        loop {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    return Ok(());
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let now = Instant::now();
            if now >= next {
                STATE.with_borrow_mut(|state| {
                    if let Some(state) = state {
                        state.poll();
                        if let Err(error) = state.present() {
                            eprintln!("Overlay: {error}");
                        }
                    }
                });
                next += period;
                if next < now {
                    next = now + period;
                }
            }
            // Relative due time in 100 ns units, negative.
            let due =
                -((next.saturating_duration_since(Instant::now()).as_nanos() / 100) as i64).max(1);
            SetWaitableTimer(timer, &due, 0, None, None, false)?;
            MsgWaitForMultipleObjectsEx(Some(&[timer]), INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
    }
}

struct State {
    hwnd: HWND,
    controller: OverlayController,
    frame: Frame,
    dpi: u32,
    moving: bool,
    /// The grip, the frame and the cursor at the press that started a drag.
    drag: Option<(Hit, Frame, POINT)>,
    dirty: bool,
    topmost_at: Instant,
    /// With `PITWALL_OVERLAY_PROFILE` set: draw time, frames drawn, last report.
    profile: Option<(Duration, u32, Instant)>,
    path: Option<PathBuf>,
    graphics: Graphics,
    metrics: Arc<Counters>,
}

impl State {
    fn scale(&self) -> f32 {
        self.dpi as f32 / 96.0
    }

    /// Refresh when either feed path changes.
    fn poll(&mut self) {
        self.dirty |= self.controller.poll();
    }

    fn present(&mut self) -> Result<()> {
        // Games in borderless mode can rise above other topmost windows; reassert once a
        // second while locked.
        if !self.moving && self.topmost_at.elapsed() >= Duration::from_secs(1) {
            self.topmost_at = Instant::now();
            // SAFETY: our own window handle.
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
        if !self.dirty {
            return Ok(());
        }
        self.dirty = false;
        let scene = Scene {
            view: self.controller.view(),
            trace: |width: f64, height: f64| self.controller.trace(width, height),
            legend: self.controller.has_reference(),
            moving: self.moving,
        };
        let started = Instant::now();
        let drawn = self
            .graphics
            .draw(self.hwnd, self.frame, self.dpi as f32, &scene);
        if drawn.is_ok() {
            self.metrics.frames.fetch_add(1, Ordering::Relaxed);
        } else {
            self.metrics.failures.fetch_add(1, Ordering::Relaxed);
            self.dirty = true;
        }
        if let Some((total, count, since)) = &mut self.profile {
            *total += started.elapsed();
            *count += 1;
            if since.elapsed() >= Duration::from_secs(10) {
                eprintln!(
                    "Overlay: {count} frames drawn, {:.3} ms each",
                    total.as_secs_f64() * 1000.0 / f64::from(*count)
                );
                *since = Instant::now();
            }
        }
        drawn
    }

    fn set_moving(&mut self, moving: bool) {
        if moving == self.moving {
            return;
        }
        self.moving = moving;
        self.drag = None;
        // SAFETY: our own window handle.
        unsafe {
            let style = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            let transparent = WS_EX_TRANSPARENT.0 as isize;
            let style = if moving {
                style & !transparent
            } else {
                style | transparent
            };
            SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, style);
            let _ = SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
        if !moving {
            self.save();
        }
        self.dirty = true;
    }

    fn settings(&self) -> OverlaySettings {
        OverlaySettings {
            visible: true,
            position: Some((self.frame.x, self.frame.y)),
            size: Some((self.frame.width, self.frame.height)),
        }
    }

    fn save(&self) {
        if let Some(path) = &self.path
            && let Err(error) = self.settings().save(path)
        {
            eprintln!("Overlay: could not save {}: {error}", path.display());
        }
    }

    fn hit(&self, lparam: LPARAM) -> Hit {
        let x = (lparam.0 & 0xFFFF) as i16 as f32 / self.scale();
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32 / self.scale();
        layout::hit_test(
            x,
            y,
            self.frame.width as f32 / self.scale(),
            self.frame.height as f32 / self.scale(),
        )
    }

    fn min_size(&self) -> (u32, u32) {
        let scale = f64::from(self.scale());
        ((MIN_SIZE.0 * scale) as u32, (MIN_SIZE.1 * scale) as u32)
    }

    /// Handle a message; `None` falls through to the default procedure.
    fn handle(&mut self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match message {
            WM_MOUSEACTIVATE => Some(LRESULT(MA_NOACTIVATE as isize)),
            WM_NCHITTEST => Some(LRESULT(if self.moving {
                HTCLIENT as isize
            } else {
                HTTRANSPARENT as isize
            })),
            WM_SETCURSOR if self.moving => {
                let mut at = POINT::default();
                // SAFETY: a local out-pointer.
                let _ = unsafe { GetCursorPos(&mut at) };
                let local = LPARAM(
                    ((at.x - self.frame.x) & 0xFFFF) as isize
                        | (((at.y - self.frame.y) & 0xFFFF) as isize) << 16,
                );
                let cursor = match self.drag.map_or_else(|| self.hit(local), |d| d.0) {
                    Hit::Move => IDC_SIZEALL,
                    Hit::Resize(Edge::North | Edge::South) => IDC_SIZENS,
                    Hit::Resize(Edge::East | Edge::West) => IDC_SIZEWE,
                    Hit::Resize(Edge::NorthEast | Edge::SouthWest) => IDC_SIZENESW,
                    Hit::Resize(Edge::NorthWest | Edge::SouthEast) => IDC_SIZENWSE,
                };
                // SAFETY: a system cursor.
                unsafe { SetCursor(LoadCursorW(None, cursor).ok()) };
                Some(LRESULT(1))
            }
            WM_LBUTTONDOWN if self.moving => {
                let mut at = POINT::default();
                // SAFETY: a local out-pointer and our own window.
                unsafe {
                    let _ = GetCursorPos(&mut at);
                    SetCapture(self.hwnd);
                }
                self.drag = Some((self.hit(lparam), self.frame, at));
                Some(LRESULT(0))
            }
            WM_MOUSEMOVE => {
                let (hit, start, from) = self.drag?;
                let mut at = POINT::default();
                // SAFETY: a local out-pointer.
                let _ = unsafe { GetCursorPos(&mut at) };
                let frame =
                    layout::drag(hit, start, (at.x - from.x, at.y - from.y), self.min_size());
                if frame != self.frame {
                    self.frame = frame;
                    self.dirty = true;
                    if let Err(error) = self.present() {
                        eprintln!("Overlay: {error}");
                    }
                }
                Some(LRESULT(0))
            }
            WM_LBUTTONUP => {
                self.drag.take()?;
                // SAFETY: releases our own capture.
                let _ = unsafe { ReleaseCapture() };
                Some(LRESULT(0))
            }
            WM_CAPTURECHANGED => {
                self.drag = None;
                Some(LRESULT(0))
            }
            WM_DPICHANGED => {
                self.dpi = (wparam.0 & 0xFFFF) as u32;
                // SAFETY: the system passes the suggested rectangle.
                let rect = unsafe { *(lparam.0 as *const RECT) };
                self.frame = Frame {
                    x: rect.left,
                    y: rect.top,
                    width: (rect.right - rect.left) as u32,
                    height: (rect.bottom - rect.top) as u32,
                };
                self.dirty = true;
                Some(LRESULT(0))
            }
            WM_SET_MOVING => {
                self.set_moving(wparam.0 != 0);
                Some(LRESULT(0))
            }
            WM_CLOSE_OVERLAY => {
                // The window is destroyed in `window_proc`, once this borrow has ended.
                self.save();
                None
            }
            _ => None,
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Messages sent while the state is borrowed (by SetWindowPos or UpdateLayeredWindow
    // inside a handler) take the default path.
    let handled = STATE.with(|cell| {
        let mut state = cell.try_borrow_mut().ok()?;
        state.as_mut()?.handle(message, wparam, lparam)
    });
    // SAFETY: our own window, on its own thread.
    unsafe {
        match message {
            WM_CLOSE_OVERLAY => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                // Failed startup candidates are destroyed before the real window exists.
                // They must not leave WM_QUIT queued for the fallback window's loop.
                let running = STATE.with(|cell| {
                    cell.try_borrow()
                        .is_ok_and(|state| state.as_ref().is_some_and(|state| state.hwnd == hwnd))
                });
                if running {
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            _ => handled.unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam)),
        }
    }
}

/// Monitor rectangles, the primary monitor and its scale, for placement.
fn monitors() -> (Vec<Rect>, Rect, f64) {
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        // SAFETY: `data` is the Vec passed below, alive for the call.
        let rects = unsafe { &mut *(data.0 as *mut Vec<Rect>) };
        rects.push(monitor_rect(monitor));
        BOOL(1)
    }
    let mut rects: Vec<Rect> = vec![];
    // SAFETY: the callback only runs during this call.
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut rects as *mut _ as isize),
        );
        let primary = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let (mut dpi, mut _y) = (96, 96);
        let _ = GetDpiForMonitor(primary, MDT_EFFECTIVE_DPI, &mut dpi, &mut _y);
        (rects, monitor_rect(primary), f64::from(dpi) / 96.0)
    }
}

fn monitor_rect(monitor: HMONITOR) -> Rect {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: a valid monitor handle and a sized MONITORINFO.
    let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };
    let r = info.rcMonitor;
    Rect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

/// What one frame draws.
struct Scene<'a, T: Fn(f64, f64) -> pitwall_core::traces::InputTrace> {
    view: Option<&'a OverlayView>,
    trace: T,
    legend: bool,
    moving: bool,
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

// The page's design tokens (`dashboard/src/styles/tokens.css`).
const PANEL: D2D1_COLOR_F = rgba(10, 14, 16, 0.62);
const MOVE_SHADE: D2D1_COLOR_F = rgba(10, 14, 16, 0.55);
const BORDER: D2D1_COLOR_F = rgba(255, 255, 255, 0.12);
const TRACK: D2D1_COLOR_F = rgba(255, 255, 255, 0.1);
const TEXT: D2D1_COLOR_F = rgba(0xF3, 0xF5, 0xF6, 1.0);
const MUTED: D2D1_COLOR_F = rgba(0x9A, 0xA7, 0xAD, 1.0);
const THROTTLE: D2D1_COLOR_F = rgba(0x56, 0xD9, 0xCA, 1.0);
const BRAKE: D2D1_COLOR_F = rgba(0xF5, 0x5C, 0x72, 1.0);

fn with_alpha(colour: D2D1_COLOR_F, alpha: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        a: colour.a * alpha,
        ..colour
    }
}

fn rect(b: Box2) -> D2D_RECT_F {
    D2D_RECT_F {
        left: b.left,
        top: b.top,
        right: b.right,
        bottom: b.bottom,
    }
}

fn rounded(b: Box2, radius: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: rect(b),
        radiusX: radius,
        radiusY: radius,
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// A memory DC holding a 32-bit top-down DIB the render target draws into.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    size: (u32, u32),
    /// Whether the render target is bound to this DC.
    bound: std::cell::Cell<bool>,
}

impl Surface {
    fn new(size: (u32, u32)) -> Result<Self> {
        // SAFETY: GDI object creation; failures are returned and cleaned up.
        unsafe {
            let dc = CreateCompatibleDC(None);
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size.0 as i32,
                    biHeight: -(size.1 as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            {
                Ok(bitmap) => bitmap,
                Err(error) => {
                    let _ = DeleteDC(dc);
                    return Err(error);
                }
            };
            let old = SelectObject(dc, bitmap.into());
            Ok(Self {
                dc,
                bitmap,
                old,
                size,
                bound: std::cell::Cell::new(false),
            })
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: objects this surface created and still owns.
        unsafe {
            SelectObject(self.dc, self.old);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.dc);
        }
    }
}

/// How the overlay reaches the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Direct2D on the GPU into a DirectComposition swap chain with premultiplied alpha
    /// (`WS_EX_NOREDIRECTIONBITMAP`). The default: no per-frame readback or CPU raster.
    Composition,
    /// A Direct2D DC render target over a DIB, shown with `UpdateLayeredWindow`.
    Layered { software: bool },
}

impl Mode {
    /// `PITWALL_OVERLAY_RENDER=software|hardware` picks the layered surface, for measuring.
    fn from_env() -> Self {
        match std::env::var("PITWALL_OVERLAY_RENDER").as_deref() {
            Ok("software") => Self::Layered { software: true },
            Ok("hardware") => Self::Layered { software: false },
            _ => Self::Composition,
        }
    }
}

/// Device-dependent drawing resources, recreated when the device is lost.
enum Backend {
    Layered {
        target: ID2D1DCRenderTarget,
        brush: ID2D1SolidColorBrush,
        surface: Option<Surface>,
    },
    Composition(Composition),
}

struct Composition {
    swap: IDXGISwapChain1,
    context: ID2D1DeviceContext,
    brush: ID2D1SolidColorBrush,
    bitmap: Option<ID2D1Bitmap1>,
    size: (u32, u32),
    /// Where the window was last placed; composition moves it with `SetWindowPos`.
    placed: Frame,
    _device: IDCompositionDevice,
    _target: IDCompositionTarget,
    _visual: IDCompositionVisual,
}

impl Composition {
    fn new(factory: &ID2D1Factory1, hwnd: HWND, frame: Frame) -> Result<Self> {
        let size = (frame.width.max(1), frame.height.max(1));
        // SAFETY: device, swap chain and composition creation with valid descriptors.
        unsafe {
            let mut device: Option<ID3D11Device> = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )?;
            let dxgi: IDXGIDevice = device.ok_or_else(windows::core::Error::empty)?.cast()?;
            let dxgi_factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))?;
            let swap = dxgi_factory.CreateSwapChainForComposition(
                &dxgi,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: size.0,
                    Height: size.1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    Stereo: false.into(),
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    Scaling: DXGI_SCALING_STRETCH,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                    AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                    Flags: 0,
                },
                None,
            )?;
            let context = factory
                .CreateDevice(&dxgi)?
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            context.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let brush = context.CreateSolidColorBrush(&TEXT, None)?;
            let composition: IDCompositionDevice = DCompositionCreateDevice(&dxgi)?;
            let target = composition.CreateTargetForHwnd(hwnd, true)?;
            let visual = composition.CreateVisual()?;
            visual.SetContent(&swap)?;
            target.SetRoot(&visual)?;
            composition.Commit()?;
            Ok(Self {
                swap,
                context,
                brush,
                bitmap: None,
                size,
                placed: frame,
                _device: composition,
                _target: target,
                _visual: visual,
            })
        }
    }

    /// Place the window and size the swap chain for `frame`, and bind the back buffer.
    fn prepare(&mut self, hwnd: HWND, frame: Frame, dpi: f32) -> Result<()> {
        let size = (frame.width.max(1), frame.height.max(1));
        // SAFETY: our own window and the swap chain we own.
        unsafe {
            if frame != self.placed {
                SetWindowPos(
                    hwnd,
                    None,
                    frame.x,
                    frame.y,
                    size.0 as i32,
                    size.1 as i32,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )?;
                self.placed = frame;
            }
            if size != self.size {
                self.context.SetTarget(None::<&ID2D1Image>);
                self.bitmap = None;
                self.swap.ResizeBuffers(
                    2,
                    size.0,
                    size.1,
                    DXGI_FORMAT_UNKNOWN,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )?;
                self.size = size;
            }
            if self.bitmap.is_none() {
                let surface: IDXGISurface = self.swap.GetBuffer(0)?;
                let bitmap = self.context.CreateBitmapFromDxgiSurface(
                    &surface,
                    Some(&D2D1_BITMAP_PROPERTIES1 {
                        pixelFormat: D2D1_PIXEL_FORMAT {
                            format: DXGI_FORMAT_B8G8R8A8_UNORM,
                            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                        },
                        dpiX: 96.0,
                        dpiY: 96.0,
                        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                        colorContext: ManuallyDrop::new(None),
                    }),
                )?;
                self.context.SetTarget(&bitmap);
                self.bitmap = Some(bitmap);
            }
            self.context.SetDpi(dpi, dpi);
        }
        Ok(())
    }
}

/// Direct2D and DirectWrite resources.
struct Graphics {
    mode: Mode,
    factory: ID2D1Factory1,
    /// The same factory through its base interface, whose methods `ID2D1Factory1` shadows.
    base: ID2D1Factory,
    backend: Option<Backend>,
    line: ID2D1StrokeStyle,
    dashed: ID2D1StrokeStyle,
    write: IDWriteFactory,
    value_format: IDWriteTextFormat,
    label_format: IDWriteTextFormat,
    small_format: IDWriteTextFormat,
    handle_format: IDWriteTextFormat,
    legend: Option<IDWriteTextLayout>,
    tabular: IDWriteTypography,
    values: HashMap<String, IDWriteTextLayout>,
    labels: Option<[IDWriteTextLayout; 2]>,
    /// The frame the window was created at, for the first composition placement.
    created: Frame,
}

/// Text layouts one frame draws.
struct Text<'a> {
    labels: &'a [IDWriteTextLayout; 2],
    values: &'a [IDWriteTextLayout; 2],
    legend: &'a IDWriteTextLayout,
}

impl Graphics {
    fn new(mode: Mode, created: Frame) -> Result<Self> {
        // SAFETY: factory and resource creation with valid descriptors.
        unsafe {
            let factory: ID2D1Factory1 =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let base: ID2D1Factory = factory.cast()?;
            let stroke = |dash| D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_FLAT,
                endCap: D2D1_CAP_STYLE_FLAT,
                dashCap: D2D1_CAP_STYLE_FLAT,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 10.0,
                dashStyle: dash,
                dashOffset: 0.0,
            };
            let line = base.CreateStrokeStyle(&stroke(D2D1_DASH_STYLE_SOLID), None)?;
            let dashed =
                base.CreateStrokeStyle(&stroke(D2D1_DASH_STYLE_CUSTOM), Some(&[3.0, 3.0]))?;
            let write: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            // The page asks for Oxanium and falls back to the system UI font when it is not
            // installed; do the same.
            let mut fonts: Option<IDWriteFontCollection> = None;
            write.GetSystemFontCollection(&mut fonts, false)?;
            let oxanium = fonts.as_ref().is_some_and(|fonts| {
                let (mut index, mut exists) = (0, BOOL(0));
                fonts
                    .FindFamilyName(w!("Oxanium"), &mut index, &mut exists)
                    .is_ok()
                    && exists.as_bool()
            });
            let value_family = if oxanium {
                w!("Oxanium")
            } else {
                w!("Segoe UI")
            };
            let format = |family: PCWSTR, size: f32, weight, centred: bool| {
                let format = write.CreateTextFormat(
                    family,
                    None,
                    weight,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!("en-us"),
                )?;
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                format.SetTextAlignment(if centred {
                    DWRITE_TEXT_ALIGNMENT_CENTER
                } else {
                    DWRITE_TEXT_ALIGNMENT_LEADING
                })?;
                format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                Result::Ok(format)
            };
            // `<b>` makes the value bold.
            let value_format = format(value_family, 13.0, DWRITE_FONT_WEIGHT_BOLD, true)?;
            let label_format = format(w!("Arial"), 10.0, DWRITE_FONT_WEIGHT_NORMAL, true)?;
            let small_format = format(w!("Arial"), 10.0, DWRITE_FONT_WEIGHT_NORMAL, false)?;
            let handle_format = format(w!("Arial"), 12.0, DWRITE_FONT_WEIGHT_NORMAL, true)?;
            let tabular = write.CreateTypography()?;
            tabular.AddFontFeature(DWRITE_FONT_FEATURE {
                nameTag: DWRITE_FONT_FEATURE_TAG_TABULAR_FIGURES,
                parameter: 1,
            })?;
            Ok(Self {
                mode,
                factory,
                base,
                backend: None,
                legend: None,
                created,
                line,
                dashed,
                write,
                value_format,
                label_format,
                small_format,
                handle_format,
                tabular,
                values: HashMap::new(),
                labels: None,
            })
        }
    }

    fn value_layout(&mut self, text: &str) -> Result<IDWriteTextLayout> {
        if let Some(layout) = self.values.get(text) {
            return Ok(layout.clone());
        }
        // SAFETY: DirectWrite layout creation from owned strings.
        let layout = unsafe {
            let layout =
                self.write
                    .CreateTextLayout(&wide(text), &self.value_format, 30.0, 17.0)?;
            layout.SetTypography(
                &self.tabular,
                DWRITE_TEXT_RANGE {
                    startPosition: 0,
                    length: u32::MAX,
                },
            )?;
            layout
        };
        self.values.insert(text.to_owned(), layout.clone());
        Ok(layout)
    }

    fn labels(&mut self) -> Result<[IDWriteTextLayout; 2]> {
        if let Some(labels) = &self.labels {
            return Ok(labels.clone());
        }
        let make = |text: &str| {
            // SAFETY: DirectWrite layout creation from owned strings.
            unsafe {
                let layout =
                    self.write
                        .CreateTextLayout(&wide(text), &self.label_format, 30.0, 12.0)?;
                // letter-spacing: 0.06em of 10 px.
                if let Ok(spaced) = layout.cast::<IDWriteTextLayout1>() {
                    spaced.SetCharacterSpacing(
                        0.0,
                        0.6,
                        0.0,
                        DWRITE_TEXT_RANGE {
                            startPosition: 0,
                            length: u32::MAX,
                        },
                    )?;
                }
                Result::Ok(layout)
            }
        };
        let labels = [make("THR")?, make("BRK")?];
        self.labels = Some(labels.clone());
        Ok(labels)
    }

    fn backend(&mut self, hwnd: HWND) -> Result<()> {
        if self.backend.is_some() {
            return Ok(());
        }
        let backend = match self.mode {
            Mode::Composition => {
                Backend::Composition(Composition::new(&self.factory, hwnd, self.created)?)
            }
            Mode::Layered { software } => {
                let properties = D2D1_RENDER_TARGET_PROPERTIES {
                    r#type: if software {
                        D2D1_RENDER_TARGET_TYPE_SOFTWARE
                    } else {
                        D2D1_RENDER_TARGET_TYPE_DEFAULT
                    },
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 0.0,
                    dpiY: 0.0,
                    usage: D2D1_RENDER_TARGET_USAGE_NONE,
                    minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
                };
                // SAFETY: a valid descriptor; the brush belongs to the new target.
                unsafe {
                    let target = self.base.CreateDCRenderTarget(&properties)?;
                    target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
                    let brush = target.CreateSolidColorBrush(&TEXT, None)?;
                    Backend::Layered {
                        target,
                        brush,
                        surface: None,
                    }
                }
            }
        };
        self.backend = Some(backend);
        Ok(())
    }

    fn legend(&mut self) -> Result<IDWriteTextLayout> {
        if let Some(legend) = &self.legend {
            return Ok(legend.clone());
        }
        // SAFETY: DirectWrite layout creation from an owned string.
        let legend = unsafe {
            self.write
                .CreateTextLayout(&wide("Dim: best lap"), &self.small_format, 200.0, 12.0)?
        };
        self.legend = Some(legend.clone());
        Ok(legend)
    }

    fn draw<T: Fn(f64, f64) -> pitwall_core::traces::InputTrace>(
        &mut self,
        hwnd: HWND,
        frame: Frame,
        dpi: f32,
        scene: &Scene<'_, T>,
    ) -> Result<()> {
        let size = (frame.width.max(1), frame.height.max(1));
        let labels = self.labels()?;
        let legend = self.legend()?;
        let absent = crate::view::ABSENT_TEXT;
        let values = match scene.view {
            Some(view) => [
                self.value_layout(&view.throttle.text)?,
                self.value_layout(&view.brake.text)?,
            ],
            None => [self.value_layout(absent)?, self.value_layout(absent)?],
        };
        let scale = dpi / 96.0;
        let l = layout::layout(size.0 as f32 / scale, size.1 as f32 / scale);
        let text = Text {
            labels: &labels,
            values: &values,
            legend: &legend,
        };
        self.backend(hwnd)?;
        let mut backend = self.backend.take().unwrap();
        let drawn = self.draw_on(&mut backend, hwnd, frame, dpi, &l, scene, &text);
        // A lost device recreates the backend on the next frame.
        if drawn.is_ok() {
            self.backend = Some(backend);
        }
        drawn
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_on<T: Fn(f64, f64) -> pitwall_core::traces::InputTrace>(
        &self,
        backend: &mut Backend,
        hwnd: HWND,
        frame: Frame,
        dpi: f32,
        l: &Layout,
        scene: &Scene<'_, T>,
        text: &Text<'_>,
    ) -> Result<()> {
        let size = (frame.width.max(1), frame.height.max(1));
        match backend {
            Backend::Composition(composition) => {
                composition.prepare(hwnd, frame, dpi)?;
                let target: &ID2D1RenderTarget = &composition.context;
                // SAFETY: drawing into the bound back buffer, then presenting it.
                unsafe {
                    target.BeginDraw();
                    target.SetTransform(&Matrix3x2::identity());
                    target.Clear(Some(&D2D1_COLOR_F::default()));
                    self.paint(target, &composition.brush, l, scene, text)?;
                    target.EndDraw(None, None)?;
                    // The waitable timer already caps us at 60 Hz. Waiting for vsync
                    // here can stall on an occluded or remotely composited desktop.
                    composition.swap.Present(0, DXGI_PRESENT(0)).ok()
                }
            }
            Backend::Layered {
                target,
                brush,
                surface,
            } => {
                if surface.as_ref().is_none_or(|s| s.size != size) {
                    *surface = None;
                    *surface = Some(Surface::new(size)?);
                }
                let surface = surface.as_ref().unwrap();
                // SAFETY: drawing into our DIB, then showing it on our window.
                unsafe {
                    if !surface.bound.replace(true) {
                        target.BindDC(
                            surface.dc,
                            &RECT {
                                left: 0,
                                top: 0,
                                right: size.0 as i32,
                                bottom: size.1 as i32,
                            },
                        )?;
                    }
                    target.SetDpi(dpi, dpi);
                    target.BeginDraw();
                    target.SetTransform(&Matrix3x2::identity());
                    target.Clear(Some(&D2D1_COLOR_F::default()));
                    self.paint(target, brush, l, scene, text)?;
                    target.EndDraw(None, None)?;
                    let blend = BLENDFUNCTION {
                        BlendOp: AC_SRC_OVER as u8,
                        BlendFlags: 0,
                        SourceConstantAlpha: 255,
                        AlphaFormat: AC_SRC_ALPHA as u8,
                    };
                    UpdateLayeredWindow(
                        hwnd,
                        None,
                        Some(&POINT {
                            x: frame.x,
                            y: frame.y,
                        }),
                        Some(&SIZE {
                            cx: size.0 as i32,
                            cy: size.1 as i32,
                        }),
                        Some(surface.dc),
                        Some(&POINT { x: 0, y: 0 }),
                        COLORREF(0),
                        Some(&blend),
                        ULW_ALPHA,
                    )
                }
            }
        }
    }

    /// # Safety
    /// Called between `BeginDraw` and `EndDraw` on `target`.
    unsafe fn paint<T: Fn(f64, f64) -> pitwall_core::traces::InputTrace>(
        &self,
        target: &ID2D1RenderTarget,
        brush: &ID2D1SolidColorBrush,
        l: &Layout,
        scene: &Scene<'_, T>,
        text: &Text<'_>,
    ) -> Result<()> {
        unsafe {
            let fill = |colour: &D2D1_COLOR_F| brush.SetColor(colour);
            fill(&PANEL);
            target.FillRoundedRectangle(&rounded(l.panel, RADIUS), brush);

            // The trace box's left and bottom border.
            let t = l.trace;
            fill(&BORDER);
            target.FillRectangle(
                &rect(Box2 {
                    left: t.left - 1.0,
                    right: t.left,
                    ..t
                }),
                brush,
            );
            target.FillRectangle(
                &rect(Box2 {
                    left: t.left - 1.0,
                    top: t.bottom,
                    bottom: t.bottom + 1.0,
                    ..t
                }),
                brush,
            );

            let view = scene.view.filter(|v| v.showing);
            if let Some(view) = view
                && t.width() > 0.0
                && t.height() > 0.0
            {
                let trace = (scene.trace)(f64::from(t.width()), f64::from(t.height()));
                let alpha = if view.stale { 0.5 } else { 1.0 };
                target.PushAxisAlignedClip(&rect(t), D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                // Reference lap first, thin and dim, so the live lap draws over it.
                for (lines, colour, width, opacity) in [
                    (&trace.reference.throttle, THROTTLE, 1.5, 0.4 * alpha),
                    (&trace.reference.brake, BRAKE, 1.5, 0.4 * alpha),
                    (&trace.throttle, THROTTLE, 3.0, alpha),
                    (&trace.brake, BRAKE, 3.0, alpha),
                ] {
                    if let Some(path) = self.path(lines, t.left, t.top)? {
                        fill(&with_alpha(colour, opacity));
                        target.DrawGeometry(&path, brush, width, &self.line);
                    }
                }
                target.PopAxisAlignedClip();
            }

            for (index, bar) in l.bars.iter().enumerate() {
                fill(&TEXT);
                target.DrawTextLayout(
                    Vector2 {
                        X: bar.value.left,
                        Y: bar.value.top,
                    },
                    &text.values[index],
                    brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
                fill(&TRACK);
                let track = rounded(bar.track, TRACK_RADIUS);
                target.FillRoundedRectangle(&track, brush);
                let level = view.map_or(0.0, |v| {
                    if index == 0 {
                        v.throttle.fill
                    } else {
                        v.brake.fill
                    }
                });
                if level > 0.0 {
                    // The track clips its fill (`overflow: hidden`): the rounded track,
                    // cut to the filled height from the bottom.
                    target.PushAxisAlignedClip(
                        &rect(Box2 {
                            top: bar.track.bottom - bar.track.height() * level,
                            ..bar.track
                        }),
                        D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    );
                    fill(if index == 0 { &THROTTLE } else { &BRAKE });
                    target.FillRoundedRectangle(&track, brush);
                    target.PopAxisAlignedClip();
                }
                fill(&MUTED);
                target.DrawTextLayout(
                    Vector2 {
                        X: bar.label.left,
                        Y: bar.label.top,
                    },
                    &text.labels[index],
                    brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }

            if scene.legend && view.is_some() {
                fill(&MUTED);
                target.DrawTextLayout(
                    Vector2 {
                        X: l.legend.0,
                        Y: l.legend.1,
                    },
                    text.legend,
                    brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }

            if scene.moving {
                let p = l.panel;
                fill(&THROTTLE);
                let outline = Box2 {
                    left: p.left + 1.0,
                    top: p.top + 1.0,
                    right: p.right - 1.0,
                    bottom: p.bottom - 1.0,
                };
                target.DrawRoundedRectangle(
                    &rounded(outline, RADIUS - 1.0),
                    brush,
                    2.0,
                    &self.dashed,
                );
                fill(&MOVE_SHADE);
                target.FillRoundedRectangle(&rounded(p, RADIUS), brush);
                fill(&TEXT);
                target.DrawText(
                    &wide("Drag to move \u{b7} edges to resize"),
                    &self.handle_format,
                    &rect(p),
                    brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    Default::default(),
                );
                // The visible bottom-right grip, clipped to the panel's rounded corner.
                let panel = self
                    .base
                    .CreateRoundedRectangleGeometry(&rounded(p, RADIUS))?;
                let grip = self.base.CreatePathGeometry()?;
                let sink = grip.Open()?;
                sink.BeginFigure(
                    Vector2 {
                        X: p.right - SE_GRIP,
                        Y: p.bottom,
                    },
                    D2D1_FIGURE_BEGIN_FILLED,
                );
                sink.AddLine(Vector2 {
                    X: p.right,
                    Y: p.bottom - SE_GRIP,
                });
                sink.AddLine(Vector2 {
                    X: p.right,
                    Y: p.bottom,
                });
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                sink.Close()?;
                self.clipped(target, &panel.cast()?, || {
                    fill(&THROTTLE);
                    target.FillGeometry(&grip, brush, None);
                });
            }
            Ok(())
        }
    }

    /// Draw `paint` through a geometric mask.
    unsafe fn clipped(
        &self,
        target: &ID2D1RenderTarget,
        mask: &ID2D1Geometry,
        paint: impl FnOnce(),
    ) {
        let params = D2D1_LAYER_PARAMETERS {
            contentBounds: D2D_RECT_F {
                left: -f32::MAX,
                top: -f32::MAX,
                right: f32::MAX,
                bottom: f32::MAX,
            },
            geometricMask: ManuallyDrop::new(Some(mask.clone())),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            opacityBrush: ManuallyDrop::new(None),
            layerOptions: D2D1_LAYER_OPTIONS_NONE,
        };
        // SAFETY: inside BeginDraw/EndDraw; the mask outlives the layer.
        unsafe {
            target.PushLayer(&params, None::<&ID2D1Layer>);
            paint();
            target.PopLayer();
        }
        drop(ManuallyDrop::into_inner(params.geometricMask));
    }

    /// One path for every line of a series, offset to the trace box: quadratic curves
    /// through the midpoints between samples, with the samples as control points, so the
    /// steps between 60 Hz samples round off and a curve never overshoots 0 % or 100 %.
    unsafe fn path(
        &self,
        lines: &[Polyline],
        left: f32,
        top: f32,
    ) -> Result<Option<windows::Win32::Graphics::Direct2D::ID2D1PathGeometry>> {
        if lines.iter().all(|line| line.len() < 2) {
            return Ok(None);
        }
        let point = |(x, y): (f64, f64)| Vector2 {
            X: left + x as f32,
            Y: top + y as f32,
        };
        // SAFETY: building a geometry we own.
        unsafe {
            let path = self.base.CreatePathGeometry()?;
            let sink = path.Open()?;
            for line in lines.iter().filter(|line| line.len() >= 2) {
                sink.BeginFigure(point(line[0]), D2D1_FIGURE_BEGIN_HOLLOW);
                for pair in line[1..].windows(2) {
                    let (x, y) = pair[0];
                    let (nx, ny) = pair[1];
                    sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                        point1: point((x, y)),
                        point2: point(((x + nx) / 2.0, (y + ny) / 2.0)),
                    });
                }
                sink.AddLine(point(line[line.len() - 1]));
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
            }
            sink.Close()?;
            Ok(Some(path))
        }
    }
}
