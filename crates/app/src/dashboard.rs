//! One dashboard window: chrome (layout switcher, Edit, focus-car picker, Display menu),
//! the tile grid for the window's size class and the edit mode over it.
//!
//! Everything apart from drawing lives in `pitwall_core`: the window's layout, choices and
//! commands in `window::DashboardWindow`, the grid geometry and drag rules in
//! `layout_edit::grid`, tile names and sizes in `tile_catalog`. This module turns clicks
//! and drags into `window::Command`s and draws the result.
//!
//! Repaint control: a next-frame callback checks the feed generations and the controller
//! version once per display frame and re-renders only when one moved.

mod name_input;
pub mod registry;
mod text_input;

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use gpui::{
    AnyElement, Context, FocusHandle, FontWeight, IntoElement, MouseButton, ParentElement, Render,
    SharedString, Styled, WeakEntity, Window, div, prelude::*, px,
};
use name_input::NameInput;
use pitwall_core::{
    binding::{
        BindingContext, CarBinding, CarTarget, ResolvedCar, car_binding_of, car_label,
        missing_car_message, participants_of, resolve_binding, resolve_focus,
    },
    feed::Reader,
    layout_edit::{
        ControllerOptions, Status,
        grid::{Gesture, GridGeometry, shown_tiles},
    },
    layouts::{LayoutLibrary, ScreenClass, Tile},
    model::Lifecycle,
    paths::AppPaths,
    screen::{self, PAGE_PADDING},
    tile_catalog,
    units::{PressureUnit, SpeedUnit, TemperatureUnit, Units},
    window::{Command, DashboardWindow, WindowChoices, WindowSetup, choices_path},
};

use pitwall_native::desktop;

use crate::{
    theme,
    tiles::{lap_timing, memory::Memory},
};
use std::{cell::RefCell, collections::HashMap};

/// Counters a measurement run reads from another thread.
#[derive(Default)]
pub struct Stats {
    /// Dashboard renders (frames with new content).
    pub renders: AtomicU64,
    /// Display frames seen by the generation check.
    pub frames: AtomicU64,
}

/// The one dashboard window this build opens; its choices file is `windows/main.json`.
pub const MAIN_WINDOW: &str = "main";

const TILE_HEADER: f32 = 28.0;
/// Width of the grip at a tile's top left in edit mode; the header text starts after it.
const EDIT_HANDLE: f32 = 28.0;
/// Room for the settings and remove buttons at a tile's top right in edit mode.
const EDIT_ACTIONS: f32 = 64.0;
const SHORT_LINE_HEIGHT: f32 = 26.0;
const SHORT_LINE: &str = "Unofficial. Not associated with the Formula 1 companies or EA.";
const UDP_HINT: &str = "In the game, turn UDP telemetry on, set UDP Format to 2025 or 2026, port 20777, and send to broadcast or this computer\u{2019}s IP.";
const TIME_TRIAL: u8 = 18;

/// The popover or dialog open over the grid; one at a time.
#[derive(Clone, Default)]
enum Menu {
    #[default]
    Closed,
    Layouts,
    Focus,
    Display,
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
    Open(Menu),
    Close,
}

/// A drag on a tile's grip or resize corner, measured from where it started.
struct Drag {
    id: String,
    origin: (f32, f32),
    tiles: Vec<Tile>,
    geometry: GridGeometry,
    gesture: Gesture,
    minimum: (u32, u32),
}

pub struct Dashboard {
    reader: Reader,
    source: SharedString,
    window: DashboardWindow,
    menu: Menu,
    focus_handle: FocusHandle,
    drag: Option<Drag>,
    /// Kept while editing so tiles do not jump as rows are added.
    row_height: f32,
    drawn: (u64, u64, u64),
    stats: Arc<Stats>,
    /// What each placed tile keeps between renders, by tile id.
    memory: RefCell<HashMap<String, Memory>>,
}

impl Dashboard {
    /// Open the main dashboard window over the host's layout library (`DataFolder`'s,
    /// shared by every window) with the choices saved under `paths.root`. An unreadable
    /// choices file is kept as `.bad`, logged and shown; the window starts at defaults.
    pub fn new(
        reader: Reader,
        source: String,
        stats: Arc<Stats>,
        library: LayoutLibrary,
        paths: &AppPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let path = choices_path(&paths.root, MAIN_WINDOW);
        let (choices, note) = match WindowChoices::load(&path) {
            Ok((choices, bad)) => (
                choices,
                bad.map(|kept| {
                    format!(
                        "This window's saved choices could not be read; kept as {} and started with the defaults.",
                        kept.display()
                    )
                }),
            ),
            Err(error) => (
                WindowChoices::default(),
                Some(format!(
                    "Could not read this window's saved choices ({error}); started with the defaults."
                )),
            ),
        };
        if let Some(note) = &note {
            desktop::log(paths, note);
        }
        let setup = WindowSetup {
            library,
            choices,
            path: Some(path),
            options: ControllerOptions::default(),
        };
        let mut this = Self::with_setup(reader, source, stats, setup, window, cx);
        if note.is_some() {
            this.window.set_error(note);
        }
        this
    }

    /// Open a window over a library the host shares between all its windows.
    pub fn with_setup(
        reader: Reader,
        source: String,
        stats: Arc<Stats>,
        setup: WindowSetup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        poll(cx.entity().downgrade(), window);
        Self {
            reader,
            source: source.into(),
            window: DashboardWindow::open(setup),
            menu: Menu::Closed,
            focus_handle: cx.focus_handle(),
            drag: None,
            row_height: screen::ROW_HEIGHT,
            drawn: (u64::MAX, u64::MAX, u64::MAX),
            stats,
            memory: RefCell::default(),
        }
    }

    /// True when the core or the layout library changed since the last render.
    fn changed(&mut self) -> bool {
        self.stats.frames.fetch_add(1, Ordering::Relaxed);
        self.window.sync();
        (
            self.reader.fast_generation(),
            self.reader.snapshot().generation,
            self.window.controller().version(),
        ) != self.drawn
    }

    fn act(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            Action::Command(command) => {
                // Choosing inside a settings dialog or the Display menu keeps it open.
                let keep_open = matches!(
                    command,
                    Command::Setting(..) | Command::Car(..) | Command::Units(..)
                );
                self.window.apply(command);
                if !keep_open {
                    self.menu = Menu::Closed;
                }
                self.drag = None;
            }
            Action::Open(menu) => {
                self.menu = menu;
                window.focus(&self.focus_handle, cx);
            }
            Action::Close => self.menu = Menu::Closed,
        }
        cx.notify();
    }

    fn button(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        action: Action,
        cx: &Context<Self>,
    ) -> AnyElement {
        let label = label.into();
        self.icon_button(id, label.clone(), label, action, cx)
    }

    /// A button whose visible text differs from its accessible name, such as "×" for
    /// "Remove Fuel".
    fn icon_button(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        name: impl Into<SharedString>,
        action: Action,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = id.into();
        let label = label.into();
        div()
            .id(id.clone())
            .role(gpui::Role::Button)
            .aria_label(name.into())
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

    fn choice(
        &self,
        id: impl Into<SharedString>,
        label: &str,
        selected: bool,
        action: Action,
        cx: &Context<Self>,
    ) -> AnyElement {
        let mark = if selected { "\u{25CF}" } else { "\u{25CB}" };
        self.button(id, format!("{mark} {label}"), action, cx)
    }

    fn time_trial(&self) -> bool {
        self.reader
            .snapshot()
            .session()
            .value()
            .is_some_and(|s| s.session_type == TIME_TRIAL)
    }

    fn layouts_menu(&self, mut body: gpui::Div, cx: &Context<Self>) -> gpui::Div {
        body = body.child("Layouts");
        let current = self.window.controller().current().map(|l| l.name.clone());
        for (index, layout) in self.window.controller().layouts().iter().enumerate() {
            let name = layout.name.clone();
            let label = format!("{}{}", name, if layout.preset { " (preset)" } else { "" });
            let mut row = div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(self.choice(
                    format!("select-{index}"),
                    &label,
                    current.as_deref() == Some(name.as_str()),
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
                        Action::Open(Menu::Rename {
                            name: name.clone(),
                            input: NameInput::new(name.clone()),
                        }),
                        cx,
                    ))
                    .child(self.button(
                        format!("delete-{index}"),
                        "Delete",
                        Action::Open(Menu::Delete(name)),
                        cx,
                    ));
            }
            body = body.child(row);
        }
        body
    }

    fn focus_menu(&self, mut body: gpui::Div, cx: &Context<Self>) -> gpui::Div {
        let participants = participants_of(&self.reader.snapshot());
        let focus = self.window.choices().focus.as_ref();
        body = body.child("Focus car").child(self.choice(
            "follow-player",
            "Follow player",
            focus.is_none(),
            Action::Command(Command::Focus(None)),
            cx,
        ));
        if let Some(focus) = focus
            && !participants.iter().any(|p| &p.identity == focus)
        {
            body = body.child(
                div()
                    .text_color(theme::muted())
                    .child("\u{25CF} Car not in session"),
            );
        }
        for p in &participants {
            let label = format!(
                "{}{}",
                car_label(p),
                if p.retired { " \u{B7} Retired" } else { "" }
            );
            body = body.child(self.choice(
                format!("focus-{}", p.index),
                &label,
                focus == Some(&p.identity),
                Action::Command(Command::Focus(Some(p.identity.clone()))),
                cx,
            ));
        }
        body
    }

    fn display_menu(&self, mut body: gpui::Div, cx: &Context<Self>) -> gpui::Div {
        let units = self.window.choices().units;
        let set = |next: Units| Action::Command(Command::Units(next));
        body = body.child("Display units (this window)").child("Speed");
        for (key, label, value) in [
            ("kmh", "km/h", SpeedUnit::Kmh),
            ("mph", "mph", SpeedUnit::Mph),
        ] {
            let next = Units {
                speed: value,
                ..units
            };
            body = body.child(self.choice(key, label, units.speed == value, set(next), cx));
        }
        body = body.child("Temperature");
        for (key, label, value) in [
            ("c", "\u{B0}C", TemperatureUnit::Celsius),
            ("f", "\u{B0}F", TemperatureUnit::Fahrenheit),
        ] {
            let next = Units {
                temperature: value,
                ..units
            };
            body = body.child(self.choice(key, label, units.temperature == value, set(next), cx));
        }
        body = body.child("Tyre pressure");
        for (key, label, value) in [
            ("psi", "psi", PressureUnit::Psi),
            ("bar", "bar", PressureUnit::Bar),
        ] {
            let next = Units {
                pressure: value,
                ..units
            };
            body = body.child(self.choice(key, label, units.pressure == value, set(next), cx));
        }
        body
    }

    fn add_menu(&self, mut body: gpui::Div, cx: &Context<Self>) -> gpui::Div {
        body = body.child("Add a tile");
        for d in tile_catalog::picker() {
            let mut label = format!("{}  {}\u{D7}{}", d.name, d.default.0, d.default.1);
            if d.follows_car() {
                label.push_str(" \u{B7} follows car");
            }
            if registry::renderer(d.kind).is_none() {
                label.push_str(" \u{B7} not yet drawn natively");
            }
            body = body.child(self.button(
                format!("add-{}", d.kind),
                label,
                Action::Command(Command::Add(d.kind.into())),
                cx,
            ));
        }
        body
    }

    fn settings_menu(&self, mut body: gpui::Div, id: &str, cx: &Context<Self>) -> gpui::Div {
        let Some(edit) = self.window.controller().edit() else {
            return body;
        };
        let Some(tile) = edit
            .draft
            .arrangement(edit.class)
            .into_iter()
            .find(|t| t.i == id)
        else {
            return body;
        };
        let Some(declaration) = tile_catalog::declaration(&tile.kind) else {
            return body;
        };
        body = body.child(format!("{} settings", declaration.name));
        if declaration.follows_car() {
            let participants = participants_of(&self.reader.snapshot());
            let car = car_binding_of(&tile.settings);
            body = body.child("Follows");
            for target in CarTarget::ALL {
                let next = CarBinding {
                    target,
                    pin: car.pin.clone(),
                };
                body = body.child(self.choice(
                    format!("target-{}", target.key()),
                    target.label(),
                    car.target == target,
                    Action::Command(Command::Car(id.to_owned(), next)),
                    cx,
                ));
            }
            if car.target == CarTarget::Pinned {
                body = body.child("Driver");
                match &car.pin {
                    None => {
                        body = body.child(div().text_color(theme::muted()).child("Choose a driver"))
                    }
                    Some(pin) if !participants.iter().any(|p| &p.identity == pin) => {
                        body = body.child(
                            div()
                                .text_color(theme::muted())
                                .child("\u{25CF} Car not in session"),
                        );
                    }
                    Some(_) => {}
                }
                for p in &participants {
                    let next = CarBinding {
                        target: CarTarget::Pinned,
                        pin: Some(p.identity.clone()),
                    };
                    body = body.child(self.choice(
                        format!("pin-{}", p.index),
                        &car_label(p),
                        car.pin.as_ref() == Some(&p.identity),
                        Action::Command(Command::Car(id.to_owned(), next)),
                        cx,
                    ));
                }
            }
        }
        let time_trial = self.time_trial();
        for field in registry::renderer(&tile.kind).map_or(&[][..], |r| r.settings) {
            body = body.child(field.label);
            let selected = field.selected(&tile.settings);
            for choice in field.choices {
                if choice.time_trial_only && !time_trial && selected != choice.value {
                    body = body.child(
                        div()
                            .text_color(theme::muted())
                            .px_2()
                            .child(format!("\u{25CB} {}", choice.label)),
                    );
                    continue;
                }
                body = body.child(self.choice(
                    format!("{}-{}", field.key, choice.value),
                    choice.label,
                    selected == choice.value,
                    Action::Command(Command::Setting(
                        id.to_owned(),
                        field.key.into(),
                        field.stored(choice),
                    )),
                    cx,
                ));
            }
        }
        body
    }

    fn rename_menu(
        &self,
        body: gpui::Div,
        name: &str,
        input: &NameInput,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let field = div()
            .p_2()
            .bg(theme::background())
            .relative()
            .id("layout-name-input")
            .role(gpui::Role::TextInput)
            .aria_label("Layout name")
            .aria_value(input.text.clone())
            .child(format!(
                "{}\u{2502}{}",
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
            });
        body.child("Rename layout")
            .child(field)
            .child(
                div()
                    .text_color(theme::muted())
                    .text_size(px(12.0))
                    .child("Enter renames. Ctrl+A selects all; Ctrl+V pastes."),
            )
            .child(self.button(
                "save-rename",
                "Rename",
                Action::Command(Command::Rename(name.to_owned(), input.text.clone())),
                cx,
            ))
    }

    fn menu(&self, cx: &Context<Self>) -> AnyElement {
        let body = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .w(px(380.0))
            .max_w_full()
            .text_size(px(14.0))
            .bg(theme::surface())
            .rounded_md();
        let body = match &self.menu {
            Menu::Closed => return div().into_any_element(),
            Menu::Layouts => self.layouts_menu(body, cx),
            Menu::Focus => self.focus_menu(body, cx),
            Menu::Display => self.display_menu(body, cx),
            Menu::Add => self.add_menu(body, cx),
            Menu::Settings(id) => self.settings_menu(body, id, cx),
            Menu::Rename { name, input } => self.rename_menu(body, name, input, cx),
            Menu::Delete(name) => body
                .child(format!("Delete \u{201C}{name}\u{201D}?"))
                .child(self.button(
                    "confirm-delete",
                    "Delete",
                    Action::Command(Command::Delete(name.clone())),
                    cx,
                ))
                .child(self.button("keep", "Keep", Action::Close, cx)),
        };
        let close = if matches!(self.menu, Menu::Delete(_)) {
            None
        } else {
            Some(self.button("close-menu", "Close", Action::Close, cx))
        };
        div()
            .id("dashboard-menu")
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_center()
            .bg(gpui::rgba(0x00000099))
            // Pressing the scrim closes, like the web dialog.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.menu = Menu::Closed;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("menu-scroll")
                    .max_h_full()
                    .overflow_y_scroll()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(body.children(close)),
            )
            .into_any_element()
    }

    /// The edit handles laid over one tile: grip, settings, remove and resize corner.
    fn edit_handles(
        &self,
        tile: &Tile,
        tiles: &[Tile],
        geometry: GridGeometry,
        cx: &Context<Self>,
    ) -> [AnyElement; 3] {
        let declaration = tile_catalog::declaration(&tile.kind);
        let minimum = declaration.map_or((1, 1), |d| d.minimum);
        let start = |gesture: Gesture| {
            let id = tile.i.clone();
            let tiles = tiles.to_vec();
            cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                this.drag = Some(Drag {
                    id: id.clone(),
                    origin: (event.position.x.into(), event.position.y.into()),
                    tiles: tiles.clone(),
                    geometry,
                    gesture,
                    minimum,
                });
                cx.stop_propagation();
                cx.notify();
            })
        };
        let name = declaration.map_or(tile.kind.as_str(), |d| d.name);
        let grip = div()
            .id(SharedString::from(format!("grip-{}", tile.i)))
            .role(gpui::Role::Button)
            .aria_label(SharedString::from(format!("Move {name}")))
            .absolute()
            .top_0()
            .left_0()
            .w(px(EDIT_HANDLE))
            .h(px(TILE_HEADER))
            .cursor_move()
            .bg(theme::meter_empty())
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(14.0))
            .child("\u{2725}")
            .on_mouse_down(MouseButton::Left, start(Gesture::Move))
            .into_any_element();
        let mut actions = div().absolute().top_0().right_0().flex().gap_1().p(px(2.0));
        // Unknown types keep their data but have no settings.
        if declaration.is_some() {
            actions = actions.child(self.icon_button(
                format!("settings-{}", tile.i),
                "\u{2699}",
                format!("Settings for {name}"),
                Action::Open(Menu::Settings(tile.i.clone())),
                cx,
            ));
        }
        actions = actions.child(self.icon_button(
            format!("remove-{}", tile.i),
            "\u{D7}",
            format!("Remove {name}"),
            Action::Command(Command::RemoveTile(tile.i.clone())),
            cx,
        ));
        let resize = div()
            .id(SharedString::from(format!("resize-{}", tile.i)))
            .absolute()
            .bottom_0()
            .right_0()
            .w(px(24.0))
            .h(px(24.0))
            .text_size(px(18.0))
            .cursor_nwse_resize()
            .child("\u{25E2}")
            .on_mouse_down(MouseButton::Left, start(Gesture::Resize))
            .into_any_element();
        [grip, actions.into_any_element(), resize]
    }
}

/// Check for new data once per display frame. gpui keeps calling back while the window
/// lives; nothing is rendered unless something moved.
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

fn class_name(class: ScreenClass) -> &'static str {
    match class {
        ScreenClass::Wide => "wide",
        ScreenClass::Tablet => "tablet",
        ScreenClass::Phone => "phone",
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.stats.renders.fetch_add(1, Ordering::Relaxed);
        let fast = self.reader.fast();
        let snap = self.reader.snapshot();
        self.drawn = (
            fast.generation,
            snap.generation,
            self.window.controller().version(),
        );
        let viewport = window.viewport_size();
        let (width, height) = (f32::from(viewport.width), f32::from(viewport.height));
        let editing = self.window.controller().is_editing();
        let class = self
            .window
            .controller()
            .edit()
            .map_or_else(|| screen::screen_class_of(width), |e| e.class);
        let tiles = self.window.tiles(class);
        let banner = match snap.lifecycle() {
            Lifecycle::Waiting => Some("Waiting for the game"),
            Lifecycle::Lost => Some("No data: game paused or closed"),
            Lifecycle::Live | Lifecycle::Ended => None,
        };
        // Rows share the height left under the chrome outside edit mode (GRID-01); while
        // editing they keep their height so tiles do not jump.
        let shown = shown_tiles(&tiles, editing);
        let grid_width = (width - 2.0 * PAGE_PADDING - screen::SCROLLBAR).max(1.0);
        if !editing {
            let chrome_height = self.chrome_estimate(banner.is_some());
            let available =
                (height - chrome_height - SHORT_LINE_HEIGHT - 2.0 * PAGE_PADDING).max(0.0);
            self.row_height = screen::fit_row_height(
                &shown.iter().map(|t| (t.y, t.h)).collect::<Vec<_>>(),
                available,
            );
        }
        let controller = self.window.controller();
        let participants = participants_of(&snap);
        let choices = self.window.choices();
        let context = BindingContext {
            participants: &participants,
            player_car: snap.player_car_index(),
            focus: choices.focus.as_ref(),
        };
        let focused = resolve_focus(&context)
            .index()
            .and_then(|i| participants.iter().find(|p| p.index == i))
            .map(car_label);
        let focus_text = match (choices.focus.is_some(), focused) {
            (false, Some(label)) => format!("Focus: player ({label})"),
            (false, None) => "Focus: player".into(),
            (true, Some(label)) => format!("Focus: {label}"),
            (true, None) => "Focus: car not in session".into(),
        };
        let time_trial = snap
            .session()
            .value()
            .is_some_and(|s| s.session_type == TIME_TRIAL);

        // Chrome, top to bottom. The row height fills what is left (GRID-01).
        let mut chrome = div().flex().flex_col().flex_shrink_0();
        let layout_name = self.window.current_name().unwrap_or("No layout").to_owned();
        let mut header = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .px_2()
            .py_1()
            .text_size(px(13.0))
            .child(div().font_weight(FontWeight::SEMIBOLD).child("pitwall"))
            .child(div().text_color(theme::muted()).child(self.source.clone()));
        if editing {
            header = header
                .child(div().child(layout_name))
                .child(self.button("add-tile", "Add tile", Action::Open(Menu::Add), cx))
                .child(self.button("done", "Done", Action::Command(Command::Done), cx))
                .child(self.button("cancel", "Cancel", Action::Command(Command::Cancel), cx));
        } else {
            header = header
                .child(self.button(
                    "layout-picker",
                    format!("Layout: {layout_name}"),
                    Action::Open(Menu::Layouts),
                    cx,
                ))
                .child(self.button("edit", "Edit", Action::Command(Command::Edit(class)), cx));
        }
        header = header
            .child(self.button("focus-picker", focus_text, Action::Open(Menu::Focus), cx))
            .child(self.button("display-menu", "Display", Action::Open(Menu::Display), cx));
        chrome = chrome.child(header);
        if let Some(banner) = banner {
            chrome = chrome.child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(13.0))
                    .child(banner)
                    .child(div().text_color(theme::muted()).child(UDP_HINT)),
            );
        }
        if let Some(edit) = controller.edit() {
            let what = if edit.base_preset && edit.dirty {
                format!(
                    "Presets are read-only: Done saves your changes as \u{201C}{}\u{201D}.",
                    edit.target_name
                )
            } else {
                "Done saves; Cancel discards every change.".into()
            };
            chrome = chrome.child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(13.0))
                    .child(format!(
                        "Editing the {} arrangement (windows {}). {what}",
                        class_name(edit.class),
                        match edit.class {
                            ScreenClass::Wide => "1280 px and wider",
                            ScreenClass::Tablet => "768\u{2013}1279 px",
                            ScreenClass::Phone => "under 768 px",
                        }
                    ))
                    .children(edit.error.as_ref().map(|e| {
                        div()
                            .text_color(theme::hot())
                            .child(format!("Could not save: {e}"))
                    })),
            );
        }
        for notice in controller.notices() {
            chrome = chrome.child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .px_2()
                    .py_1()
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
        if let Some(error) = self.window.error() {
            chrome = chrome.child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(13.0))
                    .text_color(theme::hot())
                    .child(error.to_owned()),
            );
        }
        if controller.status() == Status::Error {
            chrome = chrome.child(
                div()
                    .flex()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .child("Could not load layouts.")
                    .child(self.button(
                        "retry-layouts",
                        "Try again",
                        Action::Command(Command::Retry),
                        cx,
                    )),
            );
        }

        let geometry = GridGeometry::new(class, grid_width, self.row_height);
        let mut grid = div()
            .relative()
            .w(px(grid_width))
            .h(px(geometry.height(shown.iter().copied())));
        for tile in shown.iter().copied() {
            let declaration = tile_catalog::declaration(&tile.kind);
            let renderer = registry::renderer(&tile.kind);
            let name = renderer
                .map(|r| r.name)
                .or(declaration.map(|d| d.name))
                .unwrap_or(tile.kind.as_str())
                .to_owned();
            let (x, y, w, h) = geometry.rect(tile);
            let follows = declaration.is_some_and(|d| d.follows_car());
            let binding = car_binding_of(&tile.settings);
            let resolved = resolve_binding(&binding, &context);
            let mut head = div()
                .flex()
                .justify_between()
                .items_center()
                .gap_2()
                .h(px(TILE_HEADER))
                .px_2()
                .when(editing, |head| {
                    head.pl(px(EDIT_HANDLE + 6.0)).pr(px(EDIT_ACTIONS))
                })
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(12.0))
                .text_color(theme::muted())
                .child(name.clone());
            if tile.kind == "lap-timing" {
                let label = format!(
                    "Compare: {}",
                    lap_timing::LapReference::of(&tile.settings).label()
                );
                head = head.child(if editing {
                    let mut next = tile.settings.clone();
                    lap_timing::LapReference::cycle(&mut next, time_trial);
                    self.button(
                        format!("reference-{}", tile.i),
                        label,
                        Action::Command(Command::Setting(
                            tile.i.clone(),
                            "reference".into(),
                            next["reference"].clone(),
                        )),
                        cx,
                    )
                } else {
                    div().child(label).into_any_element()
                });
            }
            if follows
                && let Some(p) = resolved
                    .index()
                    .and_then(|i| participants.iter().find(|p| p.index == i))
            {
                head = head.child(format!(
                    "{}{}",
                    car_label(p),
                    if matches!(resolved, ResolvedCar::Retired(_)) {
                        " \u{B7} Retired"
                    } else {
                        ""
                    }
                ));
            }
            let message = |text: String| {
                div()
                    .p_3()
                    .text_size(px(14.0))
                    .text_color(theme::muted())
                    .child(text)
                    .into_any_element()
            };
            let content = match (declaration, renderer) {
                (None, _) => message(format!(
                    "Unknown tile type \u{201C}{}\u{201D}. Its place and settings are kept.",
                    tile.kind
                )),
                (Some(_), None) => message(format!("{name} is not yet drawn natively.")),
                (Some(_), Some(renderer)) => {
                    match missing_car_message(&binding, resolved).filter(|_| follows) {
                        Some(text) => message(text.to_owned()),
                        None => (renderer.render)(
                            &registry::TileInputs {
                                tile,
                                snapshot: &snap,
                                fast: &fast,
                                reader: &self.reader,
                                car: resolved,
                                units: choices.units,
                                size: (w, (h - TILE_HEADER).max(0.0)),
                            },
                            self.memory.borrow_mut().entry(tile.i.clone()).or_default(),
                        ),
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
                cell = cell.children(self.edit_handles(tile, &tiles, geometry, cx));
            }
            grid = grid.child(cell);
        }

        div()
            .id("dashboard")
            .relative()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::background())
            .text_color(theme::text())
            .font_family(theme::FONT)
            .child(chrome)
            .child(
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
                    .h(px(SHORT_LINE_HEIGHT))
                    .flex_shrink_0()
                    .px_2()
                    .text_size(px(11.0))
                    .text_color(theme::muted())
                    .child(SHORT_LINE),
            )
            .child(self.menu(cx))
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                let Some(drag) = &this.drag else { return };
                if event.pressed_button != Some(MouseButton::Left) {
                    this.drag = None;
                    return;
                }
                let delta = (
                    f32::from(event.position.x) - drag.origin.0,
                    f32::from(event.position.y) - drag.origin.1,
                );
                let items =
                    drag.geometry
                        .drag(&drag.tiles, &drag.id, delta, drag.gesture, drag.minimum);
                this.window.apply(Command::MoveResize(items));
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = None;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag = None;
                    cx.notify();
                }),
            )
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.menu = Menu::Closed;
                    this.drag = None;
                    cx.notify();
                    return;
                }
                this.handle_name_key(event, window, cx);
            }))
    }
}

impl Dashboard {
    /// Height of the chrome above the grid, estimated from what it shows; rows only
    /// need it to decide how much space to share.
    fn chrome_estimate(&self, banner: bool) -> f32 {
        const LINE: f32 = 30.0;
        let controller = self.window.controller();
        let mut lines = 1.0;
        if banner {
            lines += 2.0;
        }
        lines += controller.notices().len() as f32;
        if self.window.error().is_some() {
            lines += 1.0;
        }
        if controller.status() == Status::Error {
            lines += 1.0;
        }
        lines * LINE + 8.0
    }
}
