//! Native Windows tray. gpui owns the Windows message loop; no second UI runtime.
use pitwall_core::{feed::Reader, paths::AppPaths};
use pitwall_native::{
    desktop,
    host::{Command, Control},
};
use pitwall_overlay::{Overlay, settings::OverlaySettings};
use std::io;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem},
};

pub enum Action {
    Dashboard,
    Settings,
    Quit,
}
pub struct Tray {
    _icon: TrayIcon,
    line: MenuItem,
    dashboard: MenuItem,
    settings: MenuItem,
    logs: MenuItem,
    quit: MenuItem,
    visible: CheckMenuItem,
    moving: CheckMenuItem,
    retry: MenuItem,
    overlay: Option<Overlay>,
    overlay_settings: OverlaySettings,
    reader: Reader,
    paths: AppPaths,
    failed: bool,
}
impl Tray {
    pub fn new(
        reader: Reader,
        paths: AppPaths,
        settings: pitwall_core::overlay::OverlaySettings,
    ) -> io::Result<Self> {
        let menu = Menu::new();
        let line = MenuItem::new("Starting", false, None);
        let dashboard = MenuItem::new("Open dashboard", true, None);
        let settings_item = MenuItem::new("Status and settings", true, None);
        let logs = MenuItem::new("Open log folder", true, None);
        let quit = MenuItem::new("Quit", true, None);
        let visible = CheckMenuItem::new("Throttle/brake overlay", true, settings.visible, None);
        let moving = CheckMenuItem::new("Move / resize overlay", settings.visible, false, None);
        let retry = MenuItem::new("Retry overlay", settings.visible, None);
        menu.append_items(&[
            &line,
            &dashboard,
            &settings_item,
            &logs,
            &visible,
            &moving,
            &retry,
            &quit,
        ])
        .map_err(io::Error::other)?;
        let mut pixels = vec![0; 16 * 16 * 4];
        for y in 2..14 {
            for x in [4, 5, 10, 11] {
                pixels[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4]
                    .copy_from_slice(&[210, 210, 210, 255]);
            }
        }
        let icon = Icon::from_rgba(pixels, 16, 16).map_err(io::Error::other)?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("pitwall")
            .build()
            .map_err(io::Error::other)?;
        Ok(Self {
            _icon: tray,
            line,
            dashboard,
            settings: settings_item,
            logs,
            quit,
            visible,
            moving,
            retry,
            overlay: None,
            overlay_settings: OverlaySettings {
                visible: settings.visible,
                position: settings.position,
                size: settings.size,
            },
            reader,
            paths,
            failed: false,
        })
    }
    fn save(&self) {
        if let Err(e) = self.overlay_settings.save(&self.paths.overlay) {
            desktop::log(&self.paths, &format!("Overlay settings: {e}"));
        }
    }
    fn close_overlay(&mut self) {
        if let Some(overlay) = self.overlay.take() {
            self.overlay_settings = overlay.close();
        }
    }
    pub fn tick(&mut self, control: &Control) -> Vec<Action> {
        let state = control.status();
        self.line
            .set_text(state.error.as_deref().unwrap_or(&state.line));
        let mut actions = vec![];
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.dashboard.id() {
                actions.push(Action::Dashboard);
            } else if event.id == self.settings.id() {
                actions.push(Action::Settings);
            } else if event.id == self.logs.id() {
                let _ = control.send(Command::OpenLogs);
            } else if event.id == self.quit.id() {
                actions.push(Action::Quit);
            } else if event.id == self.visible.id() {
                let visible = self.visible.is_checked();
                self.close_overlay();
                self.overlay_settings.visible = visible;
                self.failed = false;
                self.moving.set_checked(false);
                self.moving.set_enabled(visible);
                self.retry.set_enabled(visible);
                self.save();
            } else if event.id == self.moving.id() {
                if let Some(overlay) = &self.overlay {
                    overlay.set_moving(self.moving.is_checked());
                }
            } else if event.id == self.retry.id() {
                self.close_overlay();
                self.failed = false;
            }
        }
        if self.overlay_settings.visible && self.overlay.is_none() && !self.failed {
            match pitwall_overlay::spawn(
                self.reader.clone(),
                self.overlay_settings,
                Some(self.paths.overlay.clone()),
            ) {
                Ok(overlay) => self.overlay = Some(overlay),
                Err(e) => {
                    self.failed = true;
                    let message = format!("Overlay: {e}. Use Retry overlay in the tray.");
                    desktop::log(&self.paths, &message);
                    let _ = notify_rust::Notification::new()
                        .summary("pitwall overlay failed")
                        .body(&message)
                        .show();
                }
            }
        }
        actions
    }
}
impl Drop for Tray {
    fn drop(&mut self) {
        self.close_overlay();
        self.save();
    }
}
