#![feature(buf_read_has_data_left)]

use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};

fn main() -> Result<(), anyhow::Error> {
    if let [_spice, prog, ..] = std::env::args().collect::<Vec<_>>().as_slice() {
        let PtyPair { slave, mut master } = native_pty_system().openpty(PtySize::default())?;

        let _ = slave.spawn_command(CommandBuilder::new(prog))?;

        let mut reader = BufReader::new(master.try_clone_reader()?);

        let write_handle = std::thread::spawn(move || {
            let mut line = String::new();
            loop {
                reader.read_line(&mut line).expect("failed to read line");
                print!("{}", line);
                line.clear();
            }
        });

        let read_handle = std::thread::spawn(move || {
            let mut line = String::new();
            loop {
                std::io::stdin()
                    .read_line(&mut line)
                    .expect("failed to read line");
                write!(master, "{}", line);
                line.clear();
            }
        });

        read_handle.join().expect("penis");
        write_handle.join().expect("penis");

        Ok(())
    } else {
        todo!()
    }
}
