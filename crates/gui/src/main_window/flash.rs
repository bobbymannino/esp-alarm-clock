use std::{
    io::{self, Read},
    process::{Command, Stdio},
    thread,
};

use anyhow::{Result, anyhow, bail};
use futures::channel::mpsc;

pub(super) const DEFAULT_ADDRESS: &str = "0x9000";
pub(super) const DEFAULT_SIZE: &str = "0x6000";

pub(super) struct FlashRead {
    address: u32,
    size: u32,
}

impl FlashRead {
    pub(super) const fn new(address: u32, size: u32) -> Self {
        Self { address, size }
    }
}

/// How many bytes are read from the child's pipes at a time.
const CHUNK_SIZE: usize = 1024;

pub(super) fn parse_hex(value: &str) -> Option<u32> {
    let digits = value.strip_prefix("0x")?;
    u32::from_str_radix(digits, 16).ok()
}

/// Runs `espflash read-flash`, forwarding everything it writes to `sender` as
/// it is produced.
///
/// Blocking, so this must not be called on the main thread.
pub(super) fn read(request: FlashRead, sender: &mpsc::UnboundedSender<String>) -> Result<()> {
    let output_path = std::env::temp_dir().join("nvs.bin");

    let mut child = Command::new("espflash")
        .arg("read-flash")
        .arg(format!("{:#x}", request.address))
        .arg(format!("{:#x}", request.size))
        .arg(output_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        bail!("espflash stdout/stderr were not piped");
    };

    // Both pipes need their own reader, a pipe nobody drains fills its buffer
    // and blocks the child. espflash draws its progress bar on stderr.
    let stderr_sender = sender.clone();
    let stderr_reader = thread::spawn(move || forward(stderr, &stderr_sender));
    let stdout_result = forward(stdout, sender);
    let stderr_result = stderr_reader.join().map_err(|_| anyhow!("stderr reader thread panicked"))?;
    stdout_result?;
    stderr_result?;

    let status = child.wait()?;
    if !status.success() {
        bail!("espflash exited with {status}");
    }

    Ok(())
}

/// Forwards everything `reader` produces to `sender`, a chunk at a time.
fn forward(mut reader: impl Read, sender: &mpsc::UnboundedSender<String>) -> io::Result<()> {
    let mut buf = [0_u8; CHUNK_SIZE];

    loop {
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }

        let chunk = String::from_utf8_lossy(buf.get(..read).unwrap_or_default()).into_owned();
        if sender.unbounded_send(chunk).is_err() {
            break;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_valid() {
        assert_eq!(parse_hex("0x123abc"), Some(0x123ABC));
    }

    #[test]
    fn test_parse_hex_starts_without_0x() {
        assert_eq!(parse_hex("123abc"), None);
    }

    #[test]
    fn test_parse_hex_invalid_character() {
        assert_eq!(parse_hex("0x1g"), None);
    }

    #[test]
    fn test_parse_hex_overflow() {
        assert_eq!(parse_hex("0x100000000"), None);
    }
}
