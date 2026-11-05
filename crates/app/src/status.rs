//! Native status/settings window. All host commands cross the in-process queue.
use crate::theme;
use gpui::{
    Context, FocusHandle, IntoElement, KeyDownEvent, MouseButton, Render, SharedString, Window,
    div, prelude::*, px,
};
use pitwall_native::host::{Command, Control};
use std::time::{Duration, Instant};

pub struct StatusWindow {
    control: Control,
    focus: FocusHandle,
    fields: Vec<String>,
    selected: usize,
    select_all: bool,
    draft: pitwall_core::ingest::config::Config,
    error: Option<String>,
    confirm_delete: Option<String>,
    waiting: Option<u64>,
    refreshed: Instant,
    dashboard: std::sync::Arc<dyn Fn(&mut gpui::App)>,
}
impl StatusWindow {
    pub fn new(
        control: Control,
        dashboard: std::sync::Arc<dyn Fn(&mut gpui::App)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = control.status().file_config;
        poll(cx.entity().downgrade(), window);
        Self {
            fields: vec![
                draft.udp.port.to_string(),
                draft.forward.targets.join("\n"),
                draft.recording.size_cap_gb.to_string(),
                "0".into(),
                "1".into(),
                "0".into(),
            ],
            draft,
            control,
            focus: cx.focus_handle(),
            selected: 0,
            select_all: false,
            error: None,
            confirm_delete: None,
            waiting: None,
            refreshed: Instant::now(),
            dashboard,
        }
    }
    fn send(&mut self, command: Command) {
        let status = self.control.status();
        if status.busy || self.waiting.is_some() {
            return;
        }
        match self.control.send(command) {
            Ok(()) => {
                self.waiting = Some(status.revision);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        let text = &mut self.fields[self.selected];
        if stroke.modifiers.control && stroke.key == "a" {
            self.select_all = true;
        } else if stroke.key == "tab" {
            self.selected = (self.selected + 1) % self.fields.len();
            self.select_all = true;
        } else if stroke.key == "backspace" {
            if self.select_all {
                text.clear();
            } else {
                text.pop();
            }
            self.select_all = false;
        } else if stroke.modifiers.control && stroke.key == "v" {
            if let Some(value) = cx.read_from_clipboard().and_then(|v| v.text()) {
                if self.select_all {
                    text.clear();
                }
                text.push_str(&value);
                self.select_all = false;
            }
        } else if stroke.key == "enter" && self.selected == 1 {
            text.push('\n');
        } else if !stroke.modifiers.control
            && !stroke.modifiers.alt
            && let Some(value) = &stroke.key_char
        {
            if self.select_all {
                text.clear();
            }
            text.push_str(value);
            self.select_all = false;
        }
        cx.notify();
    }
    fn field(&self, index: usize, label: &str, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(label.to_owned())
            .child(
                div()
                    .id(("field", index))
                    .p_2()
                    .bg(theme::meter_empty())
                    .border_1()
                    .border_color(if self.selected == index {
                        theme::text()
                    } else {
                        theme::muted()
                    })
                    .cursor_text()
                    .child(format!(
                        "{}{}",
                        self.fields[index],
                        if self.selected == index { " ▏" } else { "" }
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.selected = index;
                            this.select_all = true;
                            window.focus(&this.focus, cx);
                            cx.notify();
                        }),
                    ),
            )
    }
    fn button(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        command: Command,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let enabled = enabled && self.waiting.is_none() && !self.control.status().busy;
        div()
            .id(id.into())
            .px_3()
            .py_2()
            .bg(theme::meter_empty())
            .rounded_sm()
            .opacity(if enabled { 1.0 } else { 0.4 })
            .child(label.into())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if enabled {
                        this.send(command.clone());
                        cx.notify();
                    }
                }),
            )
    }
    fn save(&mut self) {
        let result = (|| -> Result<_, String> {
            let mut config = self.draft.clone();
            config.udp.port = self.fields[0]
                .parse()
                .map_err(|_| "UDP port must be 1–65535")?;
            config.forward.targets = self.fields[1]
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            config.recording.size_cap_gb = self.fields[2]
                .parse()
                .map_err(|_| "Recording cap must be a positive integer")?;
            config.validate()?;
            Ok(config)
        })();
        match result {
            Ok(config) => self.send(Command::Save(config)),
            Err(e) => self.error = Some(e),
        }
    }
}
fn poll(view: gpui::WeakEntity<StatusWindow>, window: &mut Window) {
    window.on_next_frame(move |window, cx| {
        if view
            .update(cx, |this, cx| {
                if this.refreshed.elapsed() >= Duration::from_millis(250) {
                    if let Some(revision) = this.waiting
                        && this.control.status().revision != revision
                    {
                        this.waiting = None;
                    }
                    this.refreshed = Instant::now();
                    cx.notify();
                }
            })
            .is_ok()
        {
            poll(view, window);
        }
    });
}
impl Render for StatusWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.control.status();
        let mut body = div()
            .id("status-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .font_family(theme::FONT)
            .text_size(px(14.0))
            .bg(theme::background())
            .text_color(theme::text())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key));
        body = body
            .child(div().text_size(px(22.0)).child("Status and settings"))
            .child(
                div()
                    .id("dashboard")
                    .p_2()
                    .bg(theme::surface())
                    .child("Open dashboard")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            (this.dashboard)(cx);
                        }),
                    ),
            )
            .child(status.error.clone().unwrap_or_else(|| status.line.clone()));
        if let Some(error) = &self.error {
            body = body.child(error.clone());
        }
        for notice in &status.notices {
            body = body.child(notice.clone());
        }
        body = body.child(format!(
            "{}{}",
            if self.waiting.is_some() || status.busy {
                "Working… "
            } else {
                ""
            },
            status.feedback
        ));
        if let Some(live) = &status.live {
            let s = &live.ingest;
            body = body.child(format!("Rate {:.0} Hz | dropped frames {} | recorder drops {} | parse errors {} | UID 0 {} | closed UID {} | forwarding errors {}", s.packet_rate_hz, s.dropped_frames, status.recorder_dropped, s.parse_errors, s.uid_zero, s.closed_uid, live.forwarding_errors));
            if s.packet_rate_hz > 0.0 && s.packet_rate_hz < 60.0 {
                body = body.child(
                    "Below 60 Hz. Set the game's UDP send rate to 60 Hz; 20–60 Hz is supported.",
                );
            }
            if let Some(format) = s.last_format.filter(|f| !matches!(f, 2025 | 2026)) {
                body = body.child(format!("Unsupported telemetry format {format}. These packets are not parsed or recorded. Select 2025 or 2026 in the game."));
            }
        }
        let number = |v: Option<f64>| v.map_or_else(|| "Unavailable".into(), |v| format!("{v:.2}"));
        body = body.child(format!("One-core CPU {} % | resident memory {} MiB | processing p95 {} ms / p99 {} ms", number(status.cpu.map(f64::from)), number(status.memory.map(|v| v as f64 / 1_048_576.0)), number(status.p95), number(status.p99)))
            .child("Processing measures one receive batch. Datagram-to-draw latency remains unmeasured.")
            .child(format!("Game setup: enable UDP telemetry; Format 2025 or 2026; port {}; target 127.0.0.1 on this PC, or broadcast / this PC's IP from another device; send at 60 Hz (20–60 Hz supported). Allow inbound UDP in Windows Firewall. This portable build is unsigned: run it only if you trust its source. Start pitwall before the game.", status.config.udp.port))
            .child(div().text_size(px(20.0)).child("Recordings"))
            .child(div().flex().gap_2().child(self.button("rescan", "Rescan", Command::Rescan, true, cx)).child(self.button("folder", "Open recordings folder", Command::OpenRecordings, true, cx)));
        if status.recordings.is_empty() {
            body = body.child("No recordings");
        }
        for item in &status.recordings {
            let name = item.file_name.clone();
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .bg(theme::surface())
                    .child(format!(
                        "{} | {} | {:.1} s | {} laps | {:.2} MiB | {}",
                        name,
                        item.sidecar.track.as_deref().unwrap_or("Unknown track"),
                        item.sidecar.duration_us as f64 / 1_000_000.0,
                        item.sidecar.laps,
                        item.size as f64 / 1_048_576.0,
                        if item.sidecar.pinned {
                            "Pinned"
                        } else {
                            "Unpinned"
                        }
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.button(
                                format!("play-{name}"),
                                "Start replay",
                                Command::Start(name.clone()),
                                status.loading.is_none(),
                                cx,
                            ))
                            .child(self.button(
                                format!("pin-{name}"),
                                if item.sidecar.pinned { "Unpin" } else { "Pin" },
                                Command::Pin(name.clone(), !item.sidecar.pinned),
                                true,
                                cx,
                            ))
                            .child(
                                div()
                                    .id(SharedString::from(format!("delete-{name}")))
                                    .p_2()
                                    .bg(theme::meter_empty())
                                    .child("Delete…")
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.confirm_delete = Some(name.clone());
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    ),
            );
        }
        if let Some(name) = &self.confirm_delete {
            body = body
                .child(format!("Permanently delete {name} and its metadata?"))
                .child(self.button(
                    "confirm-delete",
                    "Confirm delete",
                    Command::Delete(name.clone()),
                    true,
                    cx,
                ))
                .child(
                    div()
                        .id("cancel-delete")
                        .p_2()
                        .child("Cancel / dismiss")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.confirm_delete = None;
                                cx.notify();
                            }),
                        ),
                );
        }
        if let Some(progress) = status.loading {
            body = body.child(format!("Loading replay {:.0}%", progress * 100.0));
        }
        if status.replay_name.is_some() || status.replay.is_some() || status.loading.is_some() {
            body = body
                .child(format!(
                    "Source: {}",
                    status.replay_name.as_deref().unwrap_or("Replay")
                ))
                .child(self.button("stop", "Stop replay / return live", Command::Stop, true, cx));
        }
        if let Some(replay) = status.replay {
            body = body
                .child(format!(
                    "{} | {:.1} / {:.1} s | {} | {}x | {}",
                    replay.banner,
                    replay.position,
                    replay.duration,
                    if replay.paused { "Paused" } else { "Playing" },
                    replay.speed,
                    replay.background_live.unwrap_or_default()
                ))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(self.button(
                            "pause",
                            if replay.paused { "Resume" } else { "Pause" },
                            Command::Pause(!replay.paused),
                            true,
                            cx,
                        ))
                        .child(self.button("restart", "Restart", Command::Restart, true, cx)),
                );
            for speed in [0.5, 1.0, 2.0, 4.0] {
                body = body.child(self.button(
                    format!("speed-{speed}"),
                    format!("{speed}x"),
                    Command::Speed(speed),
                    true,
                    cx,
                ));
            }
            body = body
                .child(self.field(3, "Seek position (seconds)", cx))
                .child(
                    div()
                        .id("seek")
                        .p_2()
                        .bg(theme::meter_empty())
                        .child("Seek")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                match this.fields[3].parse() {
                                    Ok(v) => this.send(Command::Seek(v)),
                                    Err(_) => {
                                        this.error = Some("Enter a valid seek position".into())
                                    }
                                }
                                cx.notify();
                            }),
                        ),
                )
                .child(self.field(4, "Jump to lap", cx))
                .child(self.field(5, "Car slot (0–23)", cx))
                .child(
                    div()
                        .id("jump")
                        .p_2()
                        .bg(theme::meter_empty())
                        .child("Jump to lap")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                match (this.fields[5].parse(), this.fields[4].parse()) {
                                    (Ok(car), Ok(lap)) => this.send(Command::JumpLap(car, lap)),
                                    _ => {
                                        this.error = Some("Enter a car slot and lap number".into())
                                    }
                                }
                                cx.notify();
                            }),
                        ),
                );
        }
        body = body.child(div().text_size(px(20.0)).child("Host settings"))
            .child(format!("Effective UDP port {}. CLI overrides remain effective after saving. Click a field, type or paste; Ctrl+A replaces text; Tab selects next field; Enter adds a forwarding target line.", status.config.udp.port))
            .child(self.field(0, "UDP port", cx)).child(self.field(1, "Forwarding targets (one host:port per line)", cx)).child(self.field(2, "Recording cap (decimal GB)", cx));
        for (index, label, enabled) in [
            (0usize, "Raw forwarding", self.draft.forward.enabled),
            (1, "Auto recording", self.draft.recording.enabled),
            (2, "Start with Windows", self.draft.app.start_with_windows),
        ] {
            body = body.child(
                div()
                    .id(("toggle", index))
                    .p_2()
                    .bg(theme::surface())
                    .child(format!("[{}] {label}", if enabled { "x" } else { " " }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            match index {
                                0 => this.draft.forward.enabled = !this.draft.forward.enabled,
                                1 => this.draft.recording.enabled = !this.draft.recording.enabled,
                                _ => {
                                    this.draft.app.start_with_windows =
                                        !this.draft.app.start_with_windows
                                }
                            }
                            cx.notify();
                        }),
                    ),
            );
        }
        body.child(div().id("save").p_2().bg(theme::meter_empty()).child("Save settings").on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| { this.save(); cx.notify(); })))
            .child(self.button("relearn", "Relearn current map", Command::Relearn, status.track.is_some(), cx))
            .child("Unofficial. Not associated with the Formula 1 companies or EA.")
            .child("pitwall is unofficial and is not associated in any way with the Formula 1 companies. F1, FORMULA ONE, FORMULA 1, FIA FORMULA ONE WORLD CHAMPIONSHIP, GRAND PRIX and related marks are trade marks of Formula One Licensing B.V.")
            .child("This project is not endorsed by or affiliated with EA or its licensors.")
            .child("Team names, driver names and other third-party names and marks belong to their respective owners and are used only to identify them.")
    }
}
