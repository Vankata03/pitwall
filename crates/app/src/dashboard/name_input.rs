//! Single-line layout-name editing, using UTF-16 ranges at the native text seam.
use std::ops::Range;

#[derive(Clone)]
pub struct NameInput {
    pub text: String,
    pub selection: Range<usize>,
    pub marked: Option<Range<usize>>,
}

impl NameInput {
    pub fn new(text: String) -> Self {
        Self {
            selection: 0..text.len(),
            text,
            marked: None,
        }
    }
    pub fn bytes_of(&self, range: Range<usize>) -> Range<usize> {
        let offset = |wanted| {
            let mut units = 0;
            for (byte, c) in self.text.char_indices() {
                if units >= wanted {
                    return byte;
                }
                units += c.len_utf16();
            }
            self.text.len()
        };
        offset(range.start)..offset(range.end)
    }
    pub fn utf16_of(&self, range: Range<usize>) -> Range<usize> {
        self.text[..range.start].encode_utf16().count()
            ..self.text[..range.end].encode_utf16().count()
    }
    pub fn replace(&mut self, range: Option<Range<usize>>, text: &str) {
        let range = range
            .map(|r| self.bytes_of(r))
            .or(self.marked.clone())
            .unwrap_or(self.selection.clone());
        let text = text.replace(['\r', '\n'], "");
        self.text.replace_range(range.clone(), &text);
        let end = range.start + text.len();
        self.selection = end..end;
        self.marked = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_ranges_from_windows_text_services_map_to_whole_characters() {
        let mut input = NameInput::new("Pit °C ✓".into());
        assert_eq!(input.selection, 0..input.text.len());
        let degree = input.bytes_of(4..5);
        assert_eq!(&input.text[degree.clone()], "°");
        assert_eq!(input.utf16_of(degree), 4..5);
        input.replace(Some(7..8), "x\r\n");
        assert_eq!(input.text, "Pit °C x");
        assert_eq!(input.selection, input.text.len()..input.text.len());
    }
}
