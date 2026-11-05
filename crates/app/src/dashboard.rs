//! Native editable dashboard over the shared core layout controller.

mod grid;
pub mod model;
mod name_input;
pub mod preferences;
pub mod registry;
mod text_input;

pub use model::DashboardSetup;
pub use preferences::WindowPreferences;

use crate::theme;
use gpui::{
    Context, FocusHandle, IntoElement, MouseButton, ParentElement, Render, SharedString, Styled,
    WeakEntity, Window, div, prelude::*, px,
};
use grid::GridGeometry;
use model::{Command, DashboardModel};
use name_input::NameInput;
use pitwall_core::{
    binding::{
        BindingContext, ResolvedCar, car_binding_of, car_label, missing_car_message,
        participants_of, resolve_binding, resolve_focus,
    },
    feed::Reader,
    layout_edit::{ControllerOptions, Status},
    layouts::ScreenClass,
    model::Lifecycle,
    screen::{self, PAGE_PADDING},
    units::{PressureUnit, SpeedUnit, TemperatureUnit},
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
pub struct Stats {
    pub renders: AtomicU64,
    pub frames: AtomicU64,
}

const TILE_HEADER: f32 = 30.0;
const SHORT_LINE: &str = "Unofficial. Not associated with the Formula 1 companies or EA.";
const UDP_HINT: &str = "Turn UDP telemetry on in the game, choose format 2025 or 2026, port 20777, and send to this computer.";

#[derive(Clone, Default)]
enum Menu {
    #[default]
    Closed,
    Layouts,
    Focus,
    Units,
    Add,
    Settings(String),
    Rename {
        name: String,
        input: NameInput,
    },
    Delete(String),
}

#[derive(Clone)]
enum Action {
    Command(Command),
    Menu(Menu),
    Close,
    BeginEdit(ScreenClass),
}

struct Gesture {
    id: String,
    origin: (f32, f32),
    tiles: Vec<pitwall_core::layouts::Tile>,
    geometry: GridGeometry,
    resize: bool,
    minimum: (u32, u32),
}

pub struct Dashboard {
    reader: Reader,
    source: SharedString,
    pub model: DashboardModel,
    menu: Menu,
    focus_handle: FocusHandle,
    gesture: Option<Gesture>,
    row_height: f32,
    drawn: (u64, u64, u64),
    stats: Arc<Stats>,
}

impl Dashboard {
    /// Compatibility entry point for the developer slice. The host should use
    /// `with_setup` so every window receives the same LayoutLibrary instance.
    pub fn new(
        reader: Reader,
        source: String,
        stats: Arc<Stats>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let setup = (|| -> std::io::Result<DashboardSetup> {
            let paths = pitwall_core::paths::AppPaths::resolve()?;
            let data = pitwall_core::data::DataFolder::open(paths, &Default::default())?;
            let path = data.paths.root.join("windows").join("dashboard.json");
            let preferences = WindowPreferences::load(&path)?;
            Ok(DashboardSetup {
                library: data.layouts,
                preferences,
                preferences_path: Some(path),
                options: ControllerOptions::default(),
            })
        })();
        match setup {
            Ok(setup) => Self::with_setup(reader, source, stats, setup, window, cx),
            Err(error) => {
                // Never replace unreadable user data. Keep the window usable with an
                // explicitly reported temporary library until the host can retry.
                let path = std::env::temp_dir()
                    .join(format!("pitwall-recovery-{}", std::process::id()))
                    .join("layouts.json");
                let library = pitwall_core::layouts::LayoutLibrary::open(path)
                    .expect("temporary layout library");
                let mut this = Self::with_setup(
                    reader,
                    source,
                    stats,
                    DashboardSetup {
                        library,
                        preferences: Default::default(),
                        preferences_path: None,
                        options: Default::default(),
                    },
                    window,
                    cx,
                );
                this.model.error = Some(format!(
                    "Could not open saved dashboard data: {error}. Using a temporary library."
                ));
                eprintln!(
                    "pitwall-native: {}",
                    this.model.error.as_deref().unwrap_or_default()
                );
                this
            }
        }
    }

    pub fn with_setup(
        reader: Reader,
        source: String,
        stats: Arc<Stats>,
        setup: DashboardSetup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        poll(cx.entity().downgrade(), window);
        Self {
            reader,
            source: source.into(),
            model: DashboardModel::new(setup),
            menu: Menu::Closed,
            focus_handle: cx.focus_handle(),
            gesture: None,
            row_height: screen::ROW_HEIGHT,
            drawn: (u64::MAX, u64::MAX, u64::MAX),
            stats,
        }
    }

    fn changed(&mut self) -> bool {
        self.stats.frames.fetch_add(1, Ordering::Relaxed);
        self.model.sync();
        (
            self.reader.fast_generation(),
            self.reader.snapshot().generation,
            self.model.controller.version(),
        ) != self.drawn
    }

    fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::Command(command) => {
                let keep_open = matches!(command, Command::Setting(..) | Command::Units(..));
                self.model.execute(command);
                if !keep_open {
                    self.menu = Menu::Closed;
                }
                self.gesture = None;
            }
            Action::Menu(menu) => {
                self.menu = menu;
                window.focus(&self.focus_handle, cx);
            }
            Action::Close => self.menu = Menu::Closed,
            Action::BeginEdit(class) => {
                self.model.execute(Command::Edit(class));
                self.menu = Menu::Closed;
            }
        }
        cx.notify();
    }

    fn button(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        action: Action,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let id = id.into();
        let label = label.into();
        div()
            .id(id.clone())
            .accessibility_id(id)
            .role(gpui::Role::Button)
            .aria_label(label.clone())
            .px_2()
            .py_1()
            .rounded_sm()
            .bg(theme::meter_empty())
            .text_size(px(13.0))
            .cursor_pointer()
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.act(action.clone(), window, cx);
                }),
            )
            .into_any_element()
    }

    fn menu(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .w(px(370.0))
            .max_w_full()
            .max_h(px(600.0))
            .text_size(px(14.0))
            .bg(theme::surface())
            .rounded_md();
        let participants = participants_of(&self.reader.snapshot());
        match &self.menu {
            Menu::Closed => return div().into_any_element(),
            Menu::Layouts => {
                body = body.child("Layouts");
                for (index, layout) in self.model.controller.layouts().iter().enumerate() {
                    let name = layout.name.clone();
                    let mut row = div()
                        .flex()
                        .flex_wrap()
                        .gap_1()
                        .child(self.button(
                            format!("select-{index}"),
                            format!("{}{}", name, if layout.preset { " (preset)" } else { "" }),
                            Action::Command(Command::Select(name.clone())),
                            cx,
                        ))
                        .child(self.button(
                            format!("copy-{index}"),
                            "Duplicate",
                            Action::Command(Command::Duplicate(name.clone())),
                            cx,
                        ));
                    if !layout.preset {
                        row = row
                            .child(self.button(
                                format!("rename-{index}"),
                                "Rename",
                                Action::Menu(Menu::Rename {
                                    name: name.clone(),
                                    input: NameInput::new(name.clone()),
                                }),
                                cx,
                            ))
                            .child(self.button(
                                format!("delete-{index}"),
                                "Delete",
                                Action::Menu(Menu::Delete(name)),
                                cx,
                            ));
                    }
                    body = body.child(row);
                }
            }
            Menu::Focus => {
                body = body.child("Focus car").child(self.button(
                    "player",
                    "Player car",
                    Action::Command(Command::Focus(None)),
                    cx,
                ));
                for p in &participants {
                    body = body.child(self.button(
                        format!("focus-{}", p.index),
                        format!(
                            "{}{}",
                            car_label(p),
                            if p.retired { " · Retired" } else { "" }
                        ),
                        Action::Command(Command::Focus(Some(p.identity.clone()))),
                        cx,
                    ));
                }
            }
            Menu::Units => {
                let units = self.model.preferences.units;
                body = body.child("Display units (this window)");
                for (key, label, value) in [
                    ("kmh", "km/h", SpeedUnit::Kmh),
                    ("mph", "mph", SpeedUnit::Mph),
                ] {
                    body = body.child(self.button(
                        key,
                        format!("{} {label}", if units.speed == value { "●" } else { "○" }),
                        Action::Command(Command::Units(pitwall_core::units::Units {
                            speed: value,
                            ..units
                        })),
                        cx,
                    ));
                }
                for (key, label, value) in [
                    ("c", "°C", TemperatureUnit::Celsius),
                    ("f", "°F", TemperatureUnit::Fahrenheit),
                ] {
                    body = body.child(self.button(
                        key,
                        format!(
                            "{} {label}",
                            if units.temperature == value {
                                "●"
                            } else {
                                "○"
                            }
                        ),
                        Action::Command(Command::Units(pitwall_core::units::Units {
                            temperature: value,
                            ..units
                        })),
                        cx,
                    ));
                }
                for (key, label, value) in [
                    ("psi", "psi", PressureUnit::Psi),
                    ("bar", "bar", PressureUnit::Bar),
                ] {
                    body = body.child(self.button(
                        key,
                        format!(
                            "{} {label}",
                            if units.pressure == value {
                                "●"
                            } else {
                                "○"
                            }
                        ),
                        Action::Command(Command::Units(pitwall_core::units::Units {
                            pressure: value,
                            ..units
                        })),
                        cx,
                    ));
                }
            }
            Menu::Add => {
                body = body.child("Add tile");
                let mut registrations: Vec<_> = registry::ALL.iter().collect();
                registrations.sort_by_key(|r| r.name);
                for registration in registrations {
                    body = if registration.render.is_some() {
                        body.child(self.button(
                            registration.kind,
                            registration.name,
                            Action::Command(Command::Add(registration.kind.into())),
                            cx,
                        ))
                    } else {
                        body.child(
                            div()
                                .text_color(theme::muted())
                                .child(format!("{} — not yet ported", registration.name)),
                        )
                    };
                }
            }
            Menu::Settings(id) => {
                if let Some(edit) = self.model.controller.edit()
                    && let Some(tile) = edit
                        .draft
                        .arrangement(edit.class)
                        .iter()
                        .find(|t| &t.i == id)
                    && let Some(registration) =
                        registry::get(&tile.kind).filter(|r| r.render.is_some())
                {
                    body = body.child(format!("{} settings", registration.name));
                    if registration.follows_car {
                        let car = car_binding_of(&tile.settings);
                        body = body.child("Follows");
                        for (key, label) in [
                            ("focus", "Focus car"),
                            ("pinned", "Pinned driver"),
                            ("car-ahead", "Car ahead"),
                            ("car-behind", "Car behind"),
                        ] {
                            let mut next = tile
                                .settings
                                .get("car")
                                .filter(|v| v.is_object())
                                .cloned()
                                .unwrap_or_else(|| json!({}));
                            next["target"] = json!(key);
                            let selected = match car.target {
                                pitwall_core::binding::CarTarget::Focus => "focus",
                                pitwall_core::binding::CarTarget::Pinned => "pinned",
                                pitwall_core::binding::CarTarget::CarAhead => "car-ahead",
                                pitwall_core::binding::CarTarget::CarBehind => "car-behind",
                            } == key;
                            body = body.child(self.button(
                                key,
                                format!("{} {label}", if selected { "●" } else { "○" }),
                                Action::Command(Command::Setting(id.clone(), "car".into(), next)),
                                cx,
                            ));
                        }
                        if car.target == pitwall_core::binding::CarTarget::Pinned {
                            body = body.child("Driver");
                            if car.pin.is_none() {
                                body = body.child("Choose a driver");
                            }
                            if car
                                .pin
                                .as_ref()
                                .is_some_and(|pin| !participants.iter().any(|p| &p.identity == pin))
                            {
                                body = body.child("Car not in session");
                            }
                            for p in &participants {
                                let mut next = tile
                                    .settings
                                    .get("car")
                                    .filter(|v| v.is_object())
                                    .cloned()
                                    .unwrap_or_else(|| json!({}));
                                next["target"] = json!("pinned");
                                next["pin"] = json!(p.identity);
                                body = body.child(self.button(
                                    format!("pin-{}", p.index),
                                    format!(
                                        "{} {}",
                                        if car.pin.as_ref() == Some(&p.identity) {
                                            "●"
                                        } else {
                                            "○"
                                        },
                                        car_label(p)
                                    ),
                                    Action::Command(Command::Setting(
                                        id.clone(),
                                        "car".into(),
                                        next,
                                    )),
                                    cx,
                                ));
                            }
                        }
                    }
                    for field in registration.settings {
                        body = body.child(field.label);
                        let selected = tile
                            .settings
                            .get(field.key)
                            .and_then(Value::as_str)
                            .unwrap_or(field.default);
                        for &(value, label) in field.choices {
                            body = body.child(self.button(
                                format!("{}-{value}", field.key),
                                format!("{} {label}", if selected == value { "●" } else { "○" }),
                                Action::Command(Command::Setting(
                                    id.clone(),
                                    field.key.into(),
                                    json!(value),
                                )),
                                cx,
                            ));
                        }
                    }
                }
            }
            Menu::Rename { name, input } => {
                body = body
                    .child("Rename layout")
                    .child(
                        div()
                            .p_2()
                            .bg(theme::background())
                            .text_color(if !input.selection.is_empty() {
                                theme::personal_best()
                            } else {
                                theme::text()
                            })
                            .relative()
                            .id("layout-name-input")
                            .role(gpui::Role::TextInput)
                            .aria_label("Layout name")
                            .aria_value(input.text.clone())
                            .child(format!(
                                "{}│{}",
                                &input.text[..input.selection.end],
                                &input.text[input.selection.end..]
                            ))
                            .child({
                                let focus = self.focus_handle.clone();
                                let entity = cx.entity();
                                gpui::canvas(
                                    |_, _, _| (),
                                    move |bounds, _, window, cx| {
                                        window.handle_input(
                                            &focus,
                                            gpui::ElementInputHandler::new(bounds, entity.clone()),
                                            cx,
                                        );
                                    },
                                )
                                .absolute()
                                .inset_0()
                            }),
                    )
                    .child("Type a name, then Enter. Ctrl+A selects all; Ctrl+V pastes.")
                    .child(self.button(
                        "save-rename",
                        "Rename",
                        Action::Command(Command::Rename(name.clone(), input.text.clone())),
                        cx,
                    ));
            }
            Menu::Delete(name) => {
                body = body
                    .child(format!("Delete “{name}”?"))
                    .child(self.button(
                        "confirm-delete",
                        "Delete",
                        Action::Command(Command::Delete(name.clone())),
                        cx,
                    ))
                    .child(self.button("keep", "Keep", Action::Close, cx));
            }
        }
        body = body.child(self.button("close-menu", "Close", Action::Close, cx));
        div()
            .id("dashboard-menu")
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_center()
            .bg(gpui::rgba(0x00000099))
            .child(
                div()
                    .id("menu-scroll")
                    .max_h_full()
                    .overflow_y_scroll()
                    .child(body),
            )
            .into_any_element()
    }
}

fn poll(view: WeakEntity<Dashboard>, window: &mut Window) {
    window.on_next_frame(move |window, cx| {
        if view
            .update(cx, |this, cx| {
                if this.changed() {
                    cx.notify();
                }
            })
            .is_ok()
        {
            poll(view, window);
        }
    });
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.stats.renders.fetch_add(1, Ordering::Relaxed);
        let fast = self.reader.fast();
        let snap = self.reader.snapshot();
        self.drawn = (
            fast.generation,
            snap.generation,
            self.model.controller.version(),
        );
        let viewport = window.viewport_size();
        let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
        let editing = self.model.controller.is_editing();
        let class = self
            .model
            .controller
            .edit()
            .map_or_else(|| screen::screen_class_of(width), |e| e.class);
        let tiles = self.model.tiles(class);
        let participants = participants_of(&snap);
        let context = BindingContext {
            participants: &participants,
            player_car: snap.player_car_index(),
            focus: self.model.preferences.focus.as_ref(),
        };
        let focus = resolve_focus(&context);
        let focused = focus
            .index()
            .and_then(|i| participants.iter().find(|p| p.index == i))
            .map(car_label);
        let focus_text = match (self.model.preferences.focus.is_some(), focused) {
            (false, Some(label)) => format!("Focus: player ({label})"),
            (false, None) => "Focus: player".into(),
            (true, Some(label)) => format!("Focus: {label}"),
            (true, None) => "Focus: car not in session".into(),
        };
        let header_height = if width < 768.0 {
            112.0
        } else if width < 1280.0 {
            76.0
        } else {
            44.0
        };
        let banner = match snap.lifecycle() {
            Lifecycle::Waiting => Some("Waiting for the game"),
            Lifecycle::Lost => Some("No data: game paused or closed"),
            _ => None,
        };
        let banner_height = if banner.is_some() { 52.0 } else { 0.0 };
        let notice_height = self.model.controller.notices().len() as f32 * 36.0
            + if self.model.error.is_some() {
                48.0
            } else {
                0.0
            };
        let edit_height = if editing { 68.0 } else { 0.0 };
        let available = (height
            - header_height
            - banner_height
            - notice_height
            - edit_height
            - 30.0
            - 2.0 * PAGE_PADDING)
            .max(0.0);
        let last_row = tiles.iter().map(|t| t.y + t.h).max().unwrap_or(0);
        let notice_only = tiles
            .iter()
            .filter(|t| t.y + t.h == last_row)
            .all(|t| t.kind == "notice" && t.h == 1);
        let visible: Vec<_> = tiles
            .iter()
            .filter(|t| {
                editing
                    || tiles.len() <= 1
                    || !notice_only
                    || t.kind != "notice"
                    || t.y + t.h < last_row
            })
            .collect();
        if !editing {
            self.row_height = screen::fit_row_height(
                &visible.iter().map(|t| (t.y, t.h)).collect::<Vec<_>>(),
                available,
            );
        }
        let geometry = GridGeometry::new(
            class,
            (width - 2.0 * PAGE_PADDING - screen::SCROLLBAR).max(1.0),
            self.row_height,
        );
        let grid_height = visible
            .iter()
            .map(|t| {
                let (_, y, _, h) = geometry.rect(t);
                y + h
            })
            .fold(0.0, f32::max);
        let mut grid = div().relative().w(px(geometry.width)).h(px(grid_height));
        for tile in visible {
            let registration = registry::get(&tile.kind);
            let name = registration
                .map_or(tile.kind.as_str(), |r| r.name)
                .to_owned();
            let (x, y, w, h) = geometry.rect(tile);
            let binding = car_binding_of(&tile.settings);
            let resolved = resolve_binding(&binding, &context);
            let mut head = div()
                .flex()
                .justify_between()
                .items_center()
                .h(px(TILE_HEADER))
                .px_2()
                .text_size(px(12.0))
                .text_color(theme::muted())
                .child(name.clone());
            if registration.is_some_and(|r| r.follows_car)
                && let Some(p) = resolved
                    .index()
                    .and_then(|i| participants.iter().find(|p| p.index == i))
            {
                head = head.child(format!(
                    "{}{}",
                    car_label(p),
                    if matches!(resolved, ResolvedCar::Retired(_)) {
                        " · Retired"
                    } else {
                        ""
                    }
                ));
            }
            let content = match registration.and_then(|r| r.render) {
                None => div()
                    .p_3()
                    .text_size(px(14.0))
                    .text_color(theme::muted())
                    .child(format!(
                        "{name}: {}. Saved settings are retained.",
                        if registration.is_some() {
                            "not yet ported"
                        } else {
                            "unknown tile type"
                        }
                    ))
                    .into_any_element(),
                Some(render) => {
                    match registration
                        .filter(|r| r.follows_car)
                        .and_then(|_| missing_car_message(&binding, resolved))
                    {
                        Some(message) => div()
                            .p_3()
                            .text_color(theme::muted())
                            .child(message)
                            .into_any_element(),
                        None => render(&registry::TileInputs {
                            tile,
                            snapshot: &snap,
                            fast: &fast,
                            car: resolved,
                            units: self.model.preferences.units,
                            size: (w, (h - TILE_HEADER).max(0.0)),
                        }),
                    }
                }
            };
            let mut cell = div()
                .id(SharedString::from(tile.i.clone()))
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .h(px(h))
                .rounded_md()
                .bg(theme::surface())
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(head)
                .child(div().flex_1().min_h_0().overflow_hidden().child(content));
            if editing {
                let id = tile.i.clone();
                let original = tiles.clone();
                let minimum = registration.map_or((1, 1), |r| r.minimum);
                let grip = div()
                    .id(SharedString::from(format!("grip-{id}")))
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(TILE_HEADER))
                    .cursor_pointer()
                    .bg(gpui::rgba(0x00000099))
                    .px_2()
                    .text_size(px(13.0))
                    .child("↔ Drag")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            this.gesture = Some(Gesture {
                                id: id.clone(),
                                origin: (event.position.x.into(), event.position.y.into()),
                                tiles: original.clone(),
                                geometry,
                                resize: false,
                                minimum,
                            });
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    );
                let mut actions = div().absolute().top_0().right_0().flex().gap_1();
                if registration.is_some_and(|r| r.render.is_some()) {
                    actions = actions.child(self.button(
                        format!("settings-{}", tile.i),
                        "Settings",
                        Action::Menu(Menu::Settings(tile.i.clone())),
                        cx,
                    ));
                }
                actions = actions.child(self.button(
                    format!("remove-{}", tile.i),
                    "×",
                    Action::Command(Command::RemoveTile(tile.i.clone())),
                    cx,
                ));
                let id = tile.i.clone();
                let original = tiles.clone();
                let resize = div()
                    .id(SharedString::from(format!("resize-{id}")))
                    .absolute()
                    .bottom_0()
                    .right_0()
                    .w(px(28.0))
                    .h(px(28.0))
                    .text_size(px(20.0))
                    .cursor_pointer()
                    .child("◢")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            this.gesture = Some(Gesture {
                                id: id.clone(),
                                origin: (event.position.x.into(), event.position.y.into()),
                                tiles: original.clone(),
                                geometry,
                                resize: true,
                                minimum,
                            });
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    );
                cell = cell.child(grip).child(actions).child(resize);
            }
            grid = grid.child(cell);
        }
        let layout_name = self
            .model
            .controller
            .edit()
            .map(|e| e.target_name.as_str())
            .or_else(|| self.model.controller.current().map(|l| l.name.as_str()))
            .unwrap_or("No layout");
        let mut header = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .h(px(header_height))
            .px_2()
            .flex_shrink_0()
            .text_size(px(13.0))
            .child("pitwall")
            .child(self.source.clone())
            .child(self.button("focus-picker", focus_text, Action::Menu(Menu::Focus), cx))
            .child(self.button("display-menu", "Display", Action::Menu(Menu::Units), cx));
        if editing {
            header = header
                .child(div().child(layout_name.to_owned()))
                .child(self.button("add-tile", "Add tile", Action::Menu(Menu::Add), cx))
                .child(self.button("done", "Done", Action::Command(Command::Done), cx))
                .child(self.button("cancel", "Cancel", Action::Command(Command::Cancel), cx));
        } else {
            header = header
                .child(self.button(
                    "layout-picker",
                    layout_name.to_owned(),
                    Action::Menu(Menu::Layouts),
                    cx,
                ))
                .child(self.button("edit", "Edit", Action::BeginEdit(class), cx));
        }
        let mut root = div()
            .id("dashboard")
            .relative()
            .track_focus(&self.focus_handle)
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
                    .flex_shrink_0()
                    .px_2()
                    .text_size(px(13.0))
                    .child(banner)
                    .child(div().text_color(theme::muted()).child(UDP_HINT)),
            );
        }
        if let Some(edit) = self.model.controller.edit() {
            root = root.child(
                div()
                    .h(px(edit_height))
                    .flex_shrink_0()
                    .px_2()
                    .text_size(px(13.0))
                    .child(format!(
                        "Editing {class:?} arrangement. {}",
                        if edit.base_preset {
                            format!("Read-only preset. Done saves as “{}”.", edit.target_name)
                        } else {
                            "Done saves changes. Cancel discards them.".into()
                        }
                    ))
                    .children(edit.error.as_ref().map(|e| {
                        div()
                            .text_color(theme::hot())
                            .child(format!("Could not save: {e}"))
                    })),
            );
        }
        for notice in self.model.controller.notices() {
            root = root.child(
                div()
                    .h(px(36.0))
                    .flex_shrink_0()
                    .flex()
                    .gap_2()
                    .px_2()
                    .text_size(px(13.0))
                    .child(notice.text.clone())
                    .child(self.button(
                        format!("dismiss-{}", notice.id),
                        "Dismiss",
                        Action::Command(Command::Dismiss(notice.id)),
                        cx,
                    )),
            );
        }
        if let Some(error) = &self.model.error {
            root = root.child(
                div()
                    .h(px(48.0))
                    .flex_shrink_0()
                    .text_size(px(13.0))
                    .text_color(theme::hot())
                    .child(error.clone())
                    .child(self.button(
                        "retry-preferences",
                        "Retry",
                        Action::Command(Command::Retry),
                        cx,
                    )),
            );
        }
        if self.model.controller.status() == Status::Error {
            root = root.child(div().child("Could not load layouts.").child(self.button(
                "retry-layouts",
                "Try again",
                Action::Command(Command::Retry),
                cx,
            )));
        }
        root.child(
            div()
                .id("grid-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(PAGE_PADDING))
                .child(grid),
        )
        .child(
            div()
                .h(px(30.0))
                .flex_shrink_0()
                .px_2()
                .text_size(px(11.0))
                .text_color(theme::muted())
                .child(SHORT_LINE),
        )
        .child(self.menu(cx))
        .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
            if let Some(gesture) = &this.gesture {
                if !event.pressed_button.is_some_and(|b| b == MouseButton::Left) {
                    this.gesture = None;
                    return;
                }
                let delta = (
                    f32::from(event.position.x) - gesture.origin.0,
                    f32::from(event.position.y) - gesture.origin.1,
                );
                let items = gesture.geometry.change(
                    &gesture.tiles,
                    &gesture.id,
                    delta,
                    gesture.resize,
                    gesture.minimum,
                );
                this.model.execute(Command::MoveResize(items));
                cx.notify();
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.gesture = None;
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.gesture = None;
                cx.notify();
            }),
        )
        .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                this.menu = Menu::Closed;
                this.gesture = None;
                cx.notify();
                return;
            }
            this.handle_name_key(event, window, cx);
        }))
    }
}
