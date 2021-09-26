use std::io::{stdin, stdout, BufReader, Read, Write};
use std::sync::mpsc::{channel, TryRecvError};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use crossterm::{terminal, Command, ExecutableCommand};
use portable_pty::{CommandBuilder, PtyPair, PtySize};

fn main() -> Result<(), anyhow::Error> {
    let mut stdout = stdout();
    let mut stdin = stdin();

    let PtyPair { slave, mut master } =
        portable_pty::native_pty_system().openpty(PtySize::default())?;

    let repl = &std::env::args().collect::<Vec<_>>()[1];

    let (master_tx, master_rx) = channel::<u8>();
    let (stdin_tx, stdin_rx) = channel::<u8>();

    let mut child = slave.spawn_command(CommandBuilder::new(repl))?;

    let mut master_buf_reader = BufReader::new(master.try_clone_reader()?);
    let mut stdin_buf_reader = BufReader::new(stdin);

    let master_reader_thread = thread::spawn(move || loop {
        let mut buf = [0u8];
        master_buf_reader
            .read_exact(&mut buf)
            .expect("failed to read byte");
        master_tx.send(buf[0]).expect("failed to send char");
    });

    thread::spawn(move || loop {
        let mut buf = [0u8];
        stdin_buf_reader
            .read_exact(&mut buf)
            .expect("failed to read byte");
        stdin_tx.send(buf[0]).expect("failed to send char");
    });

    terminal::enable_raw_mode()?;

    'outer: loop {
        if let Some(exit_code) = child.try_wait()? {
            write!(stdout, "child exited with {:?}", exit_code)?;
            break;
        } else {
            sleep(Duration::from_millis(100));
        }
        'master: loop {
            match master_rx.try_recv() {
                Ok(c) => {
                    stdout.write_all(&[c])?;
                }
                Err(TryRecvError::Empty) => {
                    break 'master;
                }
                Err(TryRecvError::Disconnected) => {
                    break 'outer;
                }
            }
        }
        while let Ok(c) = stdin_rx.try_recv() {
            master.write_all(&[c])?;
        }
        stdout.flush()?;
    }

    terminal::disable_raw_mode()?;

    Ok(())
}
