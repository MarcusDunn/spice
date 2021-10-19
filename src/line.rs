use crossterm::cursor::MoveToPreviousLine;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::io::Write;
use syntect::easy::HighlightLines;
use syntect::parsing::SyntaxSet;
use vte::{Params, Perform};
/// a single line of input as well as the state of a cursor.
pub struct Line<'a, T: Write> {
    /// the current line
    inner: String,
    /// the position of the cursor
    cursor_pos: usize,
    /// where to flush [inner] on newline
    stdout: T,
    highlighter: HighlightLines<'a>,
    syntax_set: SyntaxSet,
}

impl<'a, T: Write> Line<'a, T> {
    pub fn new(stdout: T, highlighter: HighlightLines<'a>, syntax_set: SyntaxSet) -> Line<T> {
        Line {
            inner: String::new(),
            cursor_pos: 0,
            stdout,
            highlighter,
            syntax_set,
        }
    }
}

impl<T: Write> Line<'_, T> {
    /// deletes the previous line
    pub(crate) fn flush(&mut self) {
        self.stdout.flush().unwrap();
    }
}

impl<T: Write> Line<'_, T> {
    fn print_highlighted(&mut self) {
        let string = self.inner.clone();
        let highlighted = self
            .highlighter
            .highlight(string.as_str(), &self.syntax_set);
        let yeet = syntect::util::as_24_bit_terminal_escaped(&highlighted[..], false);
        crossterm::queue!(
            &mut self.stdout,
            MoveToPreviousLine(1),
            Clear(ClearType::CurrentLine),
            Print(yeet),
            Print("\n")
        )
        .unwrap();
    }
}

impl<T: Write> Perform for Line<'_, T> {
    fn print(&mut self, c: char) {
        self.inner.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
        self.print_highlighted();
    }

    fn execute(&mut self, byte: u8) {
        if char::from(byte) == '\n' {
            queue!(&mut self.stdout, Print("\n")).unwrap();
            self.inner.clear();
        } else if char::from(byte) == '\r' {
            self.cursor_pos = 0;
        } else if char::from(byte) == '\x08' {
            self.cursor_pos -= 1;
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {
        todo!("hook")
    }

    fn put(&mut self, _byte: u8) {
        todo!("put")
    }

    fn unhook(&mut self) {
        todo!("unhook")
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {
        todo!("osc_dispatch")
    }

    fn csi_dispatch(
        &mut self,
        _params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
        let todo = || {
            todo!(
                "csi_dispatch: _params: {:?} _intermediates: {:?} _ignore: {:?} _action: {:?}",
                _params,
                _intermediates,
                _ignore,
                _action
            )
        };
        match _action {
            'K' => {
                if self.cursor_pos == self.inner.len() {
                    self.inner.pop();
                } else {
                    self.inner.remove(self.cursor_pos);
                }
                self.print_highlighted();
            }
            _ => todo(),
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {
        todo!("esc_dispatch")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::stdout;
    use syntect::highlighting::ThemeSet;
    use vte::Parser;

    #[test]
    fn test_writing_to_line() {
        let set = SyntaxSet::load_defaults_nonewlines();
        let ss = set
            .find_syntax_by_extension("py")
            .expect("failed to make syntax set");
        let ts = ThemeSet::load_defaults();

        let highlightlines =
            syntect::easy::HighlightLines::new(ss, &ts.themes["base16-ocean.dark"]);

        let mut performer = Line::new(stdout(), highlightlines, set);

        let mut state_machine = Parser::new();

        for x in "hello world".as_bytes() {
            state_machine.advance(&mut performer, *x)
        }

        assert_eq!("hello world", performer.inner)
    }
}
