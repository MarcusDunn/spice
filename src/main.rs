use std::io::{stdin, stdout, BufReader, Write};
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;

use crossterm::terminal;
use crossterm::terminal::ClearType;
use portable_pty::{CommandBuilder, PtyPair, PtySize};
use std::convert::TryFrom;
use std::iter::FromIterator;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

fn main() -> Result<(), anyhow::Error> {
    let mut stdout = stdout();
    let stdin = stdin();

    let PtyPair { slave, mut master } =
        portable_pty::native_pty_system().openpty(PtySize::default())?;

    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    if let [_spice, repl, args @ ..] = &std::env::args().collect::<Vec<_>>()[..] {
        let ext = if repl == "python" {
            "py"
        } else if repl == "lein" {
            "clj"
        } else {
            "hs"
        };
        let mut repl = CommandBuilder::new(repl);
        repl.args(args);
        let mut child = slave.spawn_command(repl)?;

        let master_buf_reader = BufReader::new(master.try_clone_reader()?);
        let stdin_buf_reader = BufReader::new(stdin);

        let (master_tx, master_rx) = channel::<u8>();
        let (stdin_tx, stdin_rx) = channel::<u8>();

        spawn_background_reader(master_tx, master_buf_reader);
        spawn_background_reader(stdin_tx, stdin_buf_reader);

        terminal::enable_raw_mode()?;

        let mut current_line = String::new();

        loop {
            if let Some(exit_code) = child.try_wait()? {
                write!(stdout, "child exited with {:?}", exit_code)?;
                terminal::disable_raw_mode()?;
                break;
            } else {
                while let Ok(c) = master_rx.try_recv() {
                    let c = char::from(c);

                    if c == '\n' {
                        current_line.clear();
                        writeln!(stdout)?;
                    } else {
                        crossterm::queue!(stdout, crossterm::cursor::MoveToPreviousLine(1))?;
                        crossterm::queue!(
                            stdout,
                            crossterm::terminal::Clear(ClearType::CurrentLine)
                        )?;
                        stdout.flush()?;
                        if c == '\x08' {
                            current_line.remove(current_line.len() - 1);
                        } else {
                            current_line.push(c);
                        }
                        current_line = current_line.replace("\x1b[K", "");
                        let string = highlight(&ps, &ts, current_line.clone(), ext);
                        writeln!(stdout, "{}", string)?;
                    }
                }
                while let Ok(c) = stdin_rx.try_recv() {
                    master.write_all(&[c])?;
                }
                stdout.flush()?;
                sleep(Duration::from_millis(50))
            }
        }

        Ok(())
    } else {
        todo!()
    }
}

fn highlight(ps: &SyntaxSet, ts: &ThemeSet, input: String, ext: &str) -> String {
    let syntax = ps.find_syntax_by_extension(ext).unwrap();
    let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
    let line = LinesWithEndings::from(&*input).next().unwrap();
    let ranges = h.highlight(line, ps);
    as_24_bit_terminal_escaped(&ranges[..], true)
}

fn spawn_background_reader(
    stdin_tx: Sender<u8>,
    mut stdin_buf_reader: impl std::io::Read + Send + 'static,
) -> JoinHandle<()> {
    thread::spawn(move || loop {
        let mut buf = [0u8];
        stdin_buf_reader
            .read_exact(&mut buf)
            .expect("failed to read byte");
        stdin_tx.send(buf[0]).expect("failed to send char");
    })
}
