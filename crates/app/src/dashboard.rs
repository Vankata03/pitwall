//! The dashboard window: a header with the source and focus car, and the slice's two
//! tiles. Repaint control: a next-frame callback checks the feed generations once per
//! display frame and re-renders only when the core published something new.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use gpui::{
    Context, FontWeight, IntoElement, MouseButton, ParentElement, Render, SharedString, Styled,
    WeakEntity, Window, div, prelude::*, px,
};
use pitwall_core::{
    binding::{
        BindingContext, CarBinding, CarIdentity, ResolvedCar, car_binding_of, car_label,
        missing_car_message, participants_of, resolve_binding, resolve_focus,
    },
    feed::Reader,
    model::{Lifecycle, Readout},
    units::Units,
};
use serde_json::{Value, json};

use crate::{
    theme,
    tiles::{lap_timing, speed_rpm_gear},
};

/// Counters a measurement run reads from another thread.
#[derive(Default)]
pub struct Stats {
    /// Dashboard renders (frames with new content).
    pub renders: AtomicU64,
    /// Display frames seen by the generation check.
    pub frames: AtomicU64,
}

const GAP: f32 = 8.0;
const HEADER: f32 = 40.0;
const TILE_HEADER: f32 = 26.0;
const UDP_HINT: &str = "In the game, turn UDP telemetry on, set UDP Format to 2025 or 2026, port 20777, and send to broadcast or this computer\u{2019}s IP.";

/// One placed tile: its type and saved settings (as the layout file stores them).
struct TileSlot {
    kind: TileKind,
    settings: Value,
}

#[derive(Clone, Copy)]
enum TileKind {
    SpeedRpmGear,
    LapTiming,
}

pub struct Dashboard {
    reader: Reader,
    source: SharedString,
    units: Units,
    /// The window's focus car; `None` follows the player car.
    focus: Option<CarIdentity>,
    tiles: Vec<TileSlot>,
    drawn: (u64, u64),
    stats: Arc<Stats>,
}

impl Dashboard {
    pub fn new(
        reader: Reader,
        source: String,
        stats: Arc<Stats>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        poll(cx.entity().downgrade(), window);
        Self {
            reader,
            source: source.into(),
            units: Units::DEFAULT,
            focus: None,
            tiles: vec![
                TileSlot {
                    kind: TileKind::SpeedRpmGear,
                    settings: json!({}),
                },
                TileSlot {
                    kind: TileKind::LapTiming,
                    settings: json!({ "reference": "best" }),
                },
            ],
            drawn: (u64::MAX, u64::MAX),
            stats,
        }
    }

    /// True when the core published since the last render.
    fn changed(&self) -> bool {
        self.stats.frames.fetch_add(1, Ordering::Relaxed);
        (
            self.reader.fast_generation(),
            self.reader.snapshot().generation,
        ) != self.drawn
    }

    /// Player car, then each participant in turn, then back to the player car.
    fn cycle_focus(&mut self) {
        let participants = participants_of(&self.reader.snapshot());
        let next = match &self.focus {
            None => participants.first(),
            Some(current) => participants
                .iter()
                .skip_while(|p| p.identity != *current)
                .nth(1),
        };
        self.focus = next.map(|p| p.identity.clone());
    }
}

/// Check for new data once per display frame. gpui keeps calling back while the window
/// lives; nothing is rendered unless the generations moved.
fn poll(view: WeakEntity<Dashboard>, window: &mut Window) {
    window.on_next_frame(move |window, cx| {
        let alive = view
            .update(cx, |this, cx| {
                if this.changed() {
                    cx.notify();
                }
            })
            .is_ok();
        if alive {
            poll(view, window);
        }
    });
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.stats.renders.fetch_add(1, Ordering::Relaxed);
        let fast = self.reader.fast();
        let snap = self.reader.snapshot();
        self.drawn = (fast.generation, snap.generation);
        let lifecycle = snap.lifecycle();
        let participants = participants_of(&snap);
        let ctx = BindingContext {
            participants: &participants,
            player_car: snap.player_car_index(),
            focus: self.focus.as_ref(),
        };
        let label_of = |resolved: ResolvedCar| {
            resolved
                .index()
                .and_then(|i| participants.iter().find(|p| p.index == i))
                .map(car_label)
        };
        let focus_text = match (&self.focus, label_of(resolve_focus(&ctx))) {
            (None, Some(label)) => format!("Focus: player ({label})"),
            (None, None) => "Focus: player".into(),
            (Some(_), Some(label)) => format!("Focus: {label}"),
            (Some(_), None) => "Focus: car not in session".into(),
        };

        let viewport = window.viewport_size();
        let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
        let banner = match lifecycle {
            Lifecycle::Waiting => Some("Waiting for the game"),
            Lifecycle::Lost => Some("No data: game paused or closed"),
            Lifecycle::Live | Lifecycle::Ended => None,
        };
        let banner_height = if banner.is_some() { 48.0 } else { 0.0 };
        let count = self.tiles.len() as f32;
        let tile_w = ((width - GAP * (count + 1.0)) / count).max(0.0);
        let tile_h = (height - HEADER - banner_height - GAP * 2.0).max(0.0);
        let body = (tile_w, (tile_h - TILE_HEADER - 24.0).max(0.0));

        let mut row = div().flex().gap(px(GAP)).px(px(GAP)).flex_1();
        for slot in &self.tiles {
            let binding: CarBinding = car_binding_of(&slot.settings);
            let resolved = resolve_binding(&binding, &ctx);
            let car = resolved.index().and_then(|i| snap.car(i));
            let name = match slot.kind {
                TileKind::SpeedRpmGear => speed_rpm_gear::NAME,
                TileKind::LapTiming => lap_timing::NAME,
            };
            let mut head = div()
                .flex()
                .justify_between()
                .h(px(TILE_HEADER))
                .px_3()
                .pt_2()
                .text_size(px(12.0))
                .text_color(theme::muted())
                .child(name);
            if let Some(label) = label_of(resolved) {
                let retired = matches!(resolved, ResolvedCar::Retired(_));
                head = head.child(if retired {
                    format!("{label} \u{b7} Retired")
                } else {
                    label
                });
            }
            let content = match missing_car_message(&binding, resolved) {
                Some(message) => div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme::muted())
                    .child(message)
                    .into_any_element(),
                None => match slot.kind {
                    TileKind::SpeedRpmGear => {
                        let inputs = speed_rpm_gear::Inputs {
                            car: resolved.index().map_or(Readout::Absent, |i| fast.cars[i]),
                            max_rpm: car
                                .and_then(|c| c.status().value().map(|s| s.max_rpm))
                                .filter(|&m| m > 0),
                            units: self.units,
                            lifecycle,
                            width: body.0,
                            height: body.1,
                        };
                        speed_rpm_gear::render(speed_rpm_gear::view(&inputs)).into_any_element()
                    }
                    TileKind::LapTiming => {
                        let inputs = lap_timing::Inputs::of(
                            car,
                            lap_timing::LapReference::of(&slot.settings),
                            lifecycle,
                            body,
                        );
                        lap_timing::render(lap_timing::view(&inputs)).into_any_element()
                    }
                },
            };
            row = row.child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(tile_w))
                    .h(px(tile_h))
                    .rounded_md()
                    .bg(theme::surface())
                    .child(head)
                    .child(div().flex_1().child(content)),
            );
        }

        let header = div()
            .flex()
            .justify_between()
            .items_center()
            .h(px(HEADER))
            .px_3()
            .text_size(px(13.0))
            .text_color(theme::muted())
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child("pitwall"),
            )
            .child(self.source.clone())
            .child(
                div()
                    .id("focus")
                    .cursor_pointer()
                    .child(focus_text)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.cycle_focus();
                            cx.notify();
                        }),
                    ),
            );

        let mut root = div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::background())
            .text_color(theme::text())
            .font_family(theme::FONT)
            .child(header);
        if let Some(banner) = banner {
            root = root.child(
                div()
                    .h(px(banner_height))
                    .px_3()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .text_size(px(13.0))
                    .child(banner)
                    .child(div().text_color(theme::muted()).child(UDP_HINT)),
            );
        }
        root.child(row)
    }
}
