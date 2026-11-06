//! Native status and settings window. Content comes from `status_view`; every action
//! crosses the host's in-process command queue, never a network connection.
use crate::theme;
use gpui::{
    App, Context, FocusHandle, IntoElement, KeyDownEvent, MouseButton, Render, SharedString,
    Window, div, prelude::*, px,
};
use pitwall_core::ingest::config::Config;
use pitwall_native::{
    host::{Command, Control},
    status_view::{FULL_NOTICE, MEASURE_NOTE, NO_RECORDINGS, SHORT_LINE, StatusView},
};
use std::{sync::Arc, time::Duration};

/// Status refresh interval. The web page polled every two seconds; reading in-process
/// state is cheap, so refresh faster.
const REFRESH: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, PartialEq)]
enum Field {
    UdpPort,
    Targets,
    Cap,
    Seek,
    Lap,
    Car,
}
const FIELDS: [Field; 6] = [
    Field::UdpPort,
    Field::Targets,
    Field::Cap,
    Field::Seek,
    Field::Lap,
    Field::Car,
];

pub struct StatusWindow {
    scroll: gpui::ScrollHandle,
    control: Control,
    focus: FocusHandle,
    text: [String; 6],
    selected: usize,
    replace: bool,
    draft: Config,
    error: Option<String>,
    confirm_delete: Option<String>,
    waiting: Option<u64>,
    dashboard: Arc<dyn Fn(&mut App)>,
}

impl StatusWindow {
    /// The full notice is the last section of the existing status surface.
    pub fn show_notice(&mut self) {
        self.scroll.scroll_to_bottom();
    }
    pub fn new(
        control: Control,
        dashboard: Arc<dyn Fn(&mut App)>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let draft = control.status().file_config;
        let view = cx.entity().downgrade();
        let executor = cx.background_executor().clone();
        cx.spawn(async move |_, cx| {
            loop {
                executor.timer(REFRESH).await;
                let alive = view.update(cx, |this, cx| {
                    let status = this.control.status();
                    if this.waiting.is_some_and(|r| r != status.revision) && !status.busy {
                        this.waiting = None;
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            scroll: gpui::ScrollHandle::new(),
            text: [
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
            replace: false,
            error: None,
            confirm_delete: None,
            waiting: None,
            dashboard,
        }
    }

    fn busy(&self) -> bool {
        self.waiting.is_some() || self.control.status().busy
    }

    /// Busy actions disable repeat submission until the host acknowledges.
    fn send(&mut self, command: Command) {
        if self.busy() {
            return;
        }
        let revision = self.control.status().revision;
        match self.control.send(command) {
            Ok(()) => {
                self.waiting = Some(revision);
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    fn key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
        let multiline = FIELDS[self.selected] == Field::Targets;
        let text = &mut self.text[self.selected];
        if stroke.modifiers.control && stroke.key == "a" {
            self.replace = true;
        } else if stroke.key == "tab" {
            self.selected = (self.selected + 1) % FIELDS.len();
            self.replace = true;
        } else if stroke.key == "backspace" {
            if self.replace {
                text.clear();
            } else {
                text.pop();
            }
            self.replace = false;
        } else if stroke.modifiers.control && stroke.key == "v" {
            if let Some(value) = cx.read_from_clipboard().and_then(|v| v.text()) {
                if self.replace {
                    text.clear();
                }
                text.push_str(&value);
                self.replace = false;
            }
        } else if stroke.key == "enter" && multiline {
            text.push('\n');
        } else if !stroke.modifiers.control
            && !stroke.modifiers.alt
            && let Some(value) = &stroke.key_char
        {
            if self.replace {
                text.clear();
            }
            text.push_str(value);
            self.replace = false;
        }
        cx.notify();
    }

    fn field(&self, field: Field, label: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let index = FIELDS.iter().position(|f| *f == field).unwrap_or(0);
        let selected = self.selected == index;
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
                    .border_color(if selected {
                        theme::text()
                    } else {
                        theme::muted()
                    })
                    .cursor_text()
                    .child(format!(
                        "{}{}",
                        self.text[index],
                        if selected { " ▏" } else { "" }
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.selected = index;
                            this.replace = true;
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
        enabled: bool,
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self) + 'static,
    ) -> impl IntoElement {
        let enabled = enabled && !self.busy();
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
                        action(this);
                        cx.notify();
                    }
                }),
            )
    }

    fn command(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        command: Command,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.button(id, label, enabled, cx, move |this| {
            this.send(command.clone())
        })
    }

    fn save(&mut self) {
        let parsed = (|| -> Result<Config, String> {
            let mut config = self.draft.clone();
            config.udp.port = self.text[0]
                .trim()
                .parse()
                .map_err(|_| "UDP port must be 1–65535")?;
            config.forward.targets = self.text[1]
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            config.recording.size_cap_gb = self.text[2]
                .trim()
                .parse()
                .map_err(|_| "Recording size cap must be a whole number of GB")?;
            config.validate()?;
            Ok(config)
        })();
        match parsed {
            Ok(config) => self.send(Command::Save(config)),
            Err(error) => self.error = Some(format!("Settings rejected: {error}")),
        }
    }
}

fn heading(text: &'static str) -> impl IntoElement {
    div().pt_2().text_size(px(20.0)).child(text)
}

impl Render for StatusWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status = self.control.status();
        let view = StatusView::new(&status);
        let mut body = div()
            .id("status")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p_5()
            .flex()
            .flex_col()
            .gap_2()
            .font_family(theme::FONT)
            .text_size(px(14.0))
            .bg(theme::background())
            .text_color(theme::text())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key))
            .child(div().text_size(px(24.0)).child("pitwall status"));
        // The dashboard opener needs `App`, which a plain action does not get.
        body = body.child(
            div()
                .id("open-dashboard")
                .px_3()
                .py_2()
                .bg(theme::surface())
                .child("Open dashboard")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| (this.dashboard)(cx)),
                ),
        );
        if let Some(error) = &self.error {
            body = body.child(div().text_color(theme::hot()).child(error.clone()));
        }
        for alert in &view.alerts {
            body = body.child(div().text_color(theme::hot()).child(alert.clone()));
        }
        if self.busy() {
            body = body.child("Working…");
        } else if !status.feedback.is_empty() {
            body = body.child(status.feedback.clone());
        }

        body = body
            .child(heading("Connection"))
            .child(view.connection.clone());
        if let Some(guidance) = &view.rate_guidance {
            body = body.child(guidance.clone());
        }
        for (label, value) in view.counters.iter().chain(&view.measures) {
            body = body.child(format!("{label}: {value}"));
        }
        body = body.child(div().text_color(theme::muted()).child(MEASURE_NOTE));

        body = body.child(heading("Game setup checklist"));
        for line in &view.checklist {
            body = body.child(format!("• {line}"));
        }
        body = body.child(view.format.clone());
        for line in &view.trust {
            body = body.child(div().text_color(theme::muted()).child(*line));
        }

        body = body
            .child(heading("Recordings and replay"))
            .child(view.replay.clone())
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.command("rescan", "Rescan recordings", Command::Rescan, true, cx))
                    .child(self.command(
                        "folder",
                        "Open recordings folder",
                        Command::OpenRecordings,
                        true,
                        cx,
                    ))
                    .child(self.command(
                        "stop",
                        "Stop replay",
                        Command::Stop,
                        status.replay.is_some() || status.loading.is_some(),
                        cx,
                    )),
            );
        if let Some(replay) = &status.replay {
            let mut transport = div().flex().flex_wrap().gap_2().child(self.command(
                "pause",
                if replay.paused { "Resume" } else { "Pause" },
                Command::Pause(!replay.paused),
                true,
                cx,
            ));
            transport =
                transport.child(self.command("restart", "Restart", Command::Restart, true, cx));
            for speed in [0.5, 1.0, 2.0, 4.0] {
                transport = transport.child(self.command(
                    format!("speed-{speed}"),
                    format!("{speed}x"),
                    Command::Speed(speed),
                    replay.speed != speed,
                    cx,
                ));
            }
            body = body
                .child(transport)
                .child(self.field(Field::Seek, "Seek position (seconds)", cx))
                .child(self.button("seek", "Seek", true, cx, |this| {
                    match this.text[3].trim().parse::<f64>() {
                        Ok(seconds) => this.send(Command::Seek(seconds)),
                        Err(_) => this.error = Some("Enter a seek position in seconds".into()),
                    }
                }))
                .child(self.field(Field::Lap, "Lap", cx))
                .child(self.field(Field::Car, "Car slot (0–23)", cx))
                .child(self.button("jump", "Jump to lap", true, cx, |this| {
                    match (this.text[5].trim().parse(), this.text[4].trim().parse()) {
                        (Ok(car), Ok(lap)) => this.send(Command::JumpLap(car, lap)),
                        _ => this.error = Some("Enter a car slot and a lap number".into()),
                    }
                }));
        }
        if view.recordings.is_empty() {
            body = body.child(NO_RECORDINGS);
        }
        let loading = status.loading.is_some();
        for row in &view.recordings {
            let name = row.name.clone();
            let confirm = name.clone();
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .bg(theme::surface())
                    .child(name.clone())
                    .child(div().text_color(theme::muted()).child(row.summary.clone()))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(self.command(
                                format!("play-{name}"),
                                "Start replay",
                                Command::Start(name.clone()),
                                !loading,
                                cx,
                            ))
                            .child(self.command(
                                format!("pin-{name}"),
                                if row.pinned { "Unpin" } else { "Pin" },
                                Command::Pin(name.clone(), !row.pinned),
                                true,
                                cx,
                            ))
                            .child(self.button(
                                format!("delete-{name}"),
                                "Delete…",
                                true,
                                cx,
                                move |this| {
                                    this.confirm_delete = Some(confirm.clone());
                                },
                            )),
                    ),
            );
        }
        if let Some(name) = self.confirm_delete.clone() {
            body = body
                .child(format!("Delete {name}? This cannot be undone."))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            self.button("confirm-delete", "Delete", true, cx, move |this| {
                                this.confirm_delete = None;
                                this.send(Command::Delete(name.clone()));
                            }),
                        )
                        .child(self.button("cancel-delete", "Cancel", true, cx, |this| {
                            this.confirm_delete = None;
                        })),
                );
        }

        body = body
            .child(heading("Host settings"))
            .child(self.field(Field::UdpPort, "UDP port", cx))
            .child(self.field(
                Field::Targets,
                "Forward targets (one host:port per line; Enter adds a line)",
                cx,
            ))
            .child(self.field(Field::Cap, "Recording size cap (GB)", cx));
        let toggles = [
            ("Forward live telemetry", self.draft.forward.enabled),
            ("Record live telemetry", self.draft.recording.enabled),
            ("Start with Windows", self.draft.app.start_with_windows),
        ];
        for (index, (label, on)) in toggles.into_iter().enumerate() {
            body = body.child(self.button(
                SharedString::from(format!("toggle-{index}")),
                format!("[{}] {label}", if on { "x" } else { " " }),
                true,
                cx,
                move |this| match index {
                    0 => this.draft.forward.enabled = !this.draft.forward.enabled,
                    1 => this.draft.recording.enabled = !this.draft.recording.enabled,
                    _ => this.draft.app.start_with_windows = !this.draft.app.start_with_windows,
                },
            ));
        }
        body = body
            .child(div().text_color(theme::muted()).child(format!(
                "Settings persist in config.toml. Command-line flags override saved values (--udp-port, --config, --data-dir); the effective UDP port is {}.",
                status.config.udp.port
            )))
            .child(self.button("save", "Save settings", true, cx, Self::save))
            .child(self.command(
                "relearn",
                "Relearn map",
                Command::Relearn,
                view.relearn,
                cx,
            ));

        body = body.child(heading("Notice")).child(SHORT_LINE);
        for paragraph in FULL_NOTICE {
            body = body.child(div().text_color(theme::muted()).child(paragraph));
        }
        body
    }
}
