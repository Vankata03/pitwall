//! Windows text services adapter for the layout-name field, including IME input.
use super::{Action, Command, Dashboard, Menu, name_input::NameInput};
use gpui::{
    Bounds, Context, EntityInputHandler, KeyDownEvent, Pixels, Point, UTF16Selection, Window,
};
use std::ops::Range;

impl Dashboard {
    fn name_input(&mut self) -> Option<&mut NameInput> {
        match &mut self.menu {
            Menu::Rename { input, .. } => Some(input),
            _ => None,
        }
    }

    pub(super) fn handle_name_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Menu::Rename { name, input } = &mut self.menu else {
            return;
        };
        let key = &event.keystroke;
        if key.key == "enter" && input.marked.is_none() {
            let action = Action::Command(Command::Rename(name.clone(), input.text.clone()));
            self.act(action, window, cx);
            cx.stop_propagation();
            return;
        }
        match key.key.as_str() {
            "a" if key.modifiers.control => input.selection = 0..input.text.len(),
            "v" if key.modifiers.control => {
                if let Some(paste) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    input.replace(None, &paste);
                }
            }
            "c" | "x" if key.modifiers.control => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                    input.text[input.selection.clone()].to_owned(),
                ));
                if key.key == "x" {
                    input.replace(None, "");
                }
            }
            "backspace" | "delete" => {
                if input.selection.is_empty() {
                    if key.key == "backspace" {
                        input.selection.start = input.text[..input.selection.start]
                            .char_indices()
                            .next_back()
                            .map_or(0, |(i, _)| i);
                    } else {
                        input.selection.end += input.text[input.selection.end..]
                            .chars()
                            .next()
                            .map_or(0, char::len_utf8);
                    }
                }
                input.replace(None, "");
            }
            "left" | "right" | "home" | "end" => {
                let position = match key.key.as_str() {
                    "home" => 0,
                    "end" => input.text.len(),
                    "left" => input.text[..input.selection.start]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(i, _)| i),
                    _ => {
                        input.selection.end
                            + input.text[input.selection.end..]
                                .chars()
                                .next()
                                .map_or(0, char::len_utf8)
                    }
                };
                input.selection = position..position;
            }
            _ => return, // Printable and composed text goes through Windows text services.
        }
        window.notify_text_input_changed(&self.focus_handle);
        cx.stop_propagation();
        cx.notify();
    }
}

impl EntityInputHandler for Dashboard {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let input = self.name_input()?;
        let bytes = input.bytes_of(range);
        *actual = Some(input.utf16_of(bytes.clone()));
        Some(input.text[bytes].to_owned())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let input = self.name_input()?;
        Some(UTF16Selection {
            range: input.utf16_of(input.selection.clone()),
            reversed: false,
        })
    }
    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = self.name_input() {
            input.selection = input.bytes_of(range);
        }
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.name_input()?.text.encode_utf16().count())
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        if let Menu::Rename { input, .. } = &self.menu {
            input.marked.clone().map(|r| input.utf16_of(r))
        } else {
            None
        }
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = self.name_input() {
            input.marked = None;
        }
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = self.name_input() {
            input.replace(range, text);
        }
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = self.name_input() {
            input.replace(range, text);
            let length = text.replace(['\r', '\n'], "").len();
            let end = input.selection.end;
            let start = end.saturating_sub(length);
            input.marked = (!text.is_empty()).then_some(start..end);
            if let Some(selected) = selected {
                let offset = input.text[..start].encode_utf16().count();
                input.selection = input.bytes_of(offset + selected.start..offset + selected.end);
            }
        }
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(bounds)
    }
    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let input = self.name_input()?;
        Some(input.utf16_of(input.selection.clone()).end)
    }
}
