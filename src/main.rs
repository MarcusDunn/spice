use std::process::Stdio;
use std::task::Context;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::Command;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let mut child = Command::new("python")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .take()
        .expect("failed to take stdin from child")
        .write_all("print(\"hello world\")\n".as_bytes())
        .await?;

    let mut string_out = String::new();

    child
        .stdout
        .take()
        .expect("failed to take stdout from child")
        .read_to_string(&mut string_out)
        .await?;

    println!("look!: {}", string_out);

    // Await until the command completes
    let output = child.wait_with_output().await?;
    println!("the command exited with: {:?}", output);
    Ok(())
}
