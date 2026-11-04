//! Caret and selection over a text label's string, in byte offsets on char boundaries.
//! Labels never wrap, so a line is the run between two newlines.

use std::ops::Range;

pub struct TextEdit {
    pub id: u64,
    pub caret: usize,
    /// The fixed end of the selection; equal to `caret` when nothing is selected.
    pub anchor: usize,
    /// Text an input method is still composing.
    pub marked: Option<Range<usize>>,
}

impl TextEdit {
    pub fn new(id: u64, at: usize) -> Self {
        TextEdit { id, caret: at, anchor: at, marked: None }
    }

    pub fn selection(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    /// Moves the caret, dragging the selection along when `extend`.
    pub fn go(&mut self, to: usize, extend: bool) {
        self.caret = to;
        if !extend {
            self.anchor = to;
        }
    }

    /// Replaces `range` with `s` and puts the caret after it.
    pub fn replace(&mut self, text: &mut String, range: Range<usize>, s: &str) {
        text.replace_range(range.clone(), s);
        self.go(range.start + s.len(), false);
        self.marked = None;
    }
}

pub fn prev_char(t: &str, i: usize) -> usize {
    t[..i].char_indices().next_back().map_or(0, |(j, _)| j)
}

pub fn next_char(t: &str, i: usize) -> usize {
    t[i..].chars().next().map_or(i, |c| i + c.len_utf8())
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The start of the word before `i`, as ⌥← finds it.
pub fn prev_word(t: &str, i: usize) -> usize {
    let mut j = i;
    let mut seen = false;
    for (k, c) in t[..i].char_indices().rev() {
        if is_word(c) {
            seen = true;
        } else if seen {
            break;
        }
        j = k;
    }
    j
}

/// The end of the word after `i`, as ⌥→ finds it.
pub fn next_word(t: &str, i: usize) -> usize {
    let mut seen = false;
    for (k, c) in t[i..].char_indices() {
        if is_word(c) {
            seen = true;
        } else if seen {
            return i + k;
        }
    }
    t.len()
}

pub fn line_start(t: &str, i: usize) -> usize {
    t[..i].rfind('\n').map_or(0, |j| j + 1)
}

pub fn line_end(t: &str, i: usize) -> usize {
    t[i..].find('\n').map_or(t.len(), |j| i + j)
}

/// The caret one line up or down at the same x; past the first or last line it goes to the end.
pub fn vertical(t: &str, i: usize, down: bool, width: impl Fn(&str) -> f32) -> usize {
    let start = line_start(t, i);
    let x = width(&t[start..i]);
    let s = if down {
        let end = line_end(t, i);
        if end == t.len() {
            return t.len();
        }
        end + 1
    } else {
        if start == 0 {
            return 0;
        }
        line_start(t, start - 1)
    };
    s + index_at(&t[s..line_end(t, s)], x, width)
}

/// The char boundary in a single line closest to `x`.
pub fn index_at(line: &str, x: f32, width: impl Fn(&str) -> f32) -> usize {
    line.char_indices()
        .map(|(k, _)| k)
        .chain([line.len()])
        .min_by(|a, b| (width(&line[..*a]) - x).abs().total_cmp(&(width(&line[..*b]) - x).abs()))
        .unwrap_or(0)
}

pub fn to_utf16(t: &str, i: usize) -> usize {
    t[..i].encode_utf16().count()
}

pub fn from_utf16(t: &str, u: usize) -> usize {
    let mut n = 0;
    for (k, c) in t.char_indices() {
        if n >= u {
            return k;
        }
        n += c.len_utf16();
    }
    t.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> f32 {
        s.chars().count() as f32
    }

    #[test]
    fn words_skip_spaces_and_punctuation() {
        let t = "hello, big world";
        assert_eq!(prev_word(t, t.len()), 11);
        assert_eq!(prev_word(t, 11), 7);
        assert_eq!(prev_word(t, 5), 0);
        assert_eq!(next_word(t, 0), 5);
        assert_eq!(next_word(t, 5), 10);
        assert_eq!(next_word(t, 10), t.len());
    }

    #[test]
    fn vertical_keeps_the_column_and_clamps_at_the_ends() {
        let t = "abcdef\nab\nabcd";
        assert_eq!(vertical(t, 4, true, w), 9);
        assert_eq!(vertical(t, 9, true, w), 12);
        assert_eq!(vertical(t, 12, false, w), 9);
        assert_eq!(vertical(t, 2, false, w), 0);
        assert_eq!(vertical(t, 12, true, w), t.len());
    }

    #[test]
    fn utf16_offsets_round_trip_through_wide_chars() {
        let t = "a😀é";
        assert_eq!(to_utf16(t, t.len()), 4);
        assert_eq!(from_utf16(t, 3), 5);
        assert_eq!(from_utf16(t, 4), t.len());
    }
}
