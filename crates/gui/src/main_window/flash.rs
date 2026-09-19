use std::{
    process::{Command, Stdio},
    sync::LazyLock,
    thread,
};

use anyhow::{Result, bail};
use futures::channel::mpsc;
use regex::Regex;

pub(super) const DEFAULT_ADDRESS: &str = "0x9000";
pub(super) const DEFAULT_SIZE: &str = "0x6000";

/// How many bytes are read from the child's pipes at a time.
const CHUNK_SIZE: usize = 1024;

static HEX_REGEX: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| Regex::new(r"^0x[0-9a-f]+$"));

pub(super) fn is_valid_hex(value: &str) -> bool {
    HEX_REGEX.as_ref().is_ok_and(|regex| regex.is_match(value))
}

/// Runs `espflash read-flash`, forwarding everything it writes to `sender` as
/// it is produced.
///
/// Blocking, so this must not be called on the main thread.
pub(super) fn read(flash_address: &str, flash_size: &str, sender: &mpsc::UnboundedSender<String>) -> Result<()> {
    let output_path = std::env::temp_dir().join("nvs.bin");

    let mut child = Command::new("espflash")
        .args(["read-flash", flash_address, flash_size])
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
    forward(stdout, sender);
    stderr_reader.join().ok();

    let status = child.wait()?;
    if !status.success() {
        bail!("espflash exited with {status}");
    }

    Ok(())
}

/// Forwards everything `reader` produces to `sender`, a chunk at a time.
fn forward(mut reader: impl std::io::Read, sender: &mpsc::UnboundedSender<String>) {
    let mut buf = [0_u8; CHUNK_SIZE];

    while let Ok(read) = reader.read(&mut buf) {
        if read == 0 {
            break;
        }

        let chunk = String::from_utf8_lossy(buf.get(..read).unwrap_or_default()).into_owned();
        if sender.unbounded_send(chunk).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_regex_valid() {
        let hex = "0x123abc";
        assert!(is_valid_hex(hex));
    }

    #[test]
    fn test_hex_regex_starts_without_0x() {
        let hex = "123abc";
        assert!(!is_valid_hex(hex));
    }

    #[test]
    fn test_hex_regex_invalid_character() {
        let hex = "0x1g";
        assert!(!is_valid_hex(hex));
    }
}
