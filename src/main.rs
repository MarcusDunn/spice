use crate::line::Line;

use crossterm::{terminal};
use portable_pty::{CommandBuilder, MasterPty, PtyPair, PtySize};
use std::io::{stdin, stdout, BufReader, Stdin, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use vte::{Parser};

mod line;

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
