use crossterm::cursor::{MoveLeft, MoveToPreviousLine, MoveUp};
use crossterm::style::Print;
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType;
use crossterm::{queue, terminal};
use portable_pty::{CommandBuilder, MasterPty, PtyPair, PtySize};
use std::io::{stdin, stdout, BufReader, Stdin, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Highlighter, Theme, ThemeSet};
use syntect::parsing::{SyntaxSet, SyntaxSetBuilder};
use vte::{Params, Parser, Perform};

/// a single line of input as well as the state of a cursor.
struct Line<'a, T: Write> {
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
    fn new(stdout: T, highlighter: HighlightLines<'a>, syntax_set: SyntaxSet) -> Line<T> {
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
        queue!(
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

fn main() -> Result<(), anyhow::Error> {
    println!();
    let stdout = stdout();
    let stdout = stdout.lock();
    let stdin = stdin();

    let mut state_machine = Parser::new();

    let set = SyntaxSet::load_defaults_nonewlines();
    let ss = set
        .find_syntax_by_extension("py")
        .expect("failed to make syntax set");
    let ts = ThemeSet::load_defaults();

    let highlightlines = syntect::easy::HighlightLines::new(ss, &ts.themes["base16-ocean.dark"]);

    let mut performer = Line::new(stdout, highlightlines, set);

    if let [_spice, repl, args @ ..] = &std::env::args().collect::<Vec<_>>()[..] {
        let PtyPair { slave, mut master } =
            portable_pty::native_pty_system().openpty(PtySize::default())?;

        let mut repl = CommandBuilder::new(repl);
        repl.args(args);
        let mut child = slave.spawn_command(repl)?;

        let (master_rx, stdin_rx) = spawn_repl(stdin, &mut master);

        terminal::enable_raw_mode()?;

        loop {
            if child.try_wait()?.is_some() {
                break;
            } else {
                while let Ok(c) = master_rx.try_recv() {
                    state_machine.advance(&mut performer, c);
                }
            }
            while let Ok(c) = stdin_rx.try_recv() {
                master.write_all(&[c])?;
            }
            performer.flush();
            sleep(Duration::from_millis(50))
        }
        Ok(())
    } else {
        todo!()
    }
}

fn spawn_repl(
    stdin: Stdin,
    master: &mut Box<dyn MasterPty + Send>,
) -> (Receiver<u8>, Receiver<u8>) {
    let master_buf_reader = BufReader::new(master.try_clone_reader().unwrap());
    let stdin_buf_reader = BufReader::new(stdin);

    let (master_tx, master_rx) = channel();
    let (stdin_tx, stdin_rx) = channel();

    spawn_background_reader(master_tx, master_buf_reader);
    spawn_background_reader(stdin_tx, stdin_buf_reader);
    (master_rx, stdin_rx)
}

fn spawn_background_reader(
    stdin_tx: Sender<u8>,
    mut stdin_buf_reader: impl std::io::Read + Send + 'static,
) -> JoinHandle<()> {
    thread::spawn(move || loop {
        let mut buf = [0; 1];
        stdin_buf_reader
            .read_exact(&mut buf)
            .expect("failed to read byte");
        stdin_tx.send(buf[0]).expect("failed to send char");
    })
}
