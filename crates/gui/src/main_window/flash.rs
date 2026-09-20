mod page_decoder;

use std::{
    io::{self, Read},
    process::{Command, Stdio},
    thread,
};

use anyhow::{Result, anyhow, bail};
use futures::{SinkExt as _, channel::mpsc, executor::block_on};

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
const CHUNK_SIZE: usize = 8 * 1024;

pub(super) fn parse_hex(value: &str) -> Option<u32> {
    let digits = value.strip_prefix("0x")?;
    u32::from_str_radix(digits, 16).ok()
}

/// Runs `espflash read-flash`, forwarding everything it writes to `sender` as
/// it is produced.
///
/// Blocking, so this must not be called on the main thread.
pub(super) fn read(request: FlashRead, mut sender: mpsc::Sender<String>) -> Result<()> {
    let output_dir = tempfile::Builder::new().prefix("esp-alarm-clock-").tempdir()?;
    let output_path = output_dir.path().join("nvs.bin");

    let mut child = Command::new("espflash")
        .arg("read-flash")
        .arg(format!("{:#x}", request.address))
        .arg(format!("{:#x}", request.size))
        .arg(&output_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        bail!("espflash stdout/stderr were not piped");
    };

    // Both pipes need their own reader, a pipe nobody drains fills its buffer
    // and blocks the child. espflash draws its progress bar on stderr.
    let mut stderr_sender = sender.clone();
    let stderr_reader = thread::spawn(move || forward(stderr, &mut stderr_sender));
    let stdout_result = forward(stdout, &mut sender);
    let stderr_result = stderr_reader.join().map_err(|_| anyhow!("stderr reader thread panicked"))?;
    stdout_result?;
    stderr_result?;

    let status = child.wait()?;
    if !status.success() {
        bail!("espflash exited with {status}");
    }

    page_decoder::decode_flash(output_path.as_path())?;

    Ok(())
}

#[derive(Default)]
struct Utf8LossyDecoder {
    pending: Vec<u8>,
}

impl Utf8LossyDecoder {
    fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        self.decode(false)
    }

    fn finish(&mut self) -> String {
        self.decode(true)
    }

    fn decode(&mut self, finish: bool) -> String {
        let mut decoded = String::new();
        let mut consumed = 0;

        while consumed < self.pending.len() {
            let Some(remaining) = self.pending.get(consumed..) else {
                break;
            };

            match std::str::from_utf8(remaining) {
                Ok(valid) => {
                    decoded.push_str(valid);
                    consumed = self.pending.len();
                }
                Err(error) => {
                    let valid_end = consumed.saturating_add(error.valid_up_to());
                    if let Some(valid) = self.pending.get(consumed..valid_end) {
                        decoded.push_str(&String::from_utf8_lossy(valid));
                    }
                    consumed = valid_end;

                    if let Some(error_len) = error.error_len() {
                        decoded.push(char::REPLACEMENT_CHARACTER);
                        consumed = consumed.saturating_add(error_len).min(self.pending.len());
                    } else if finish {
                        decoded.push(char::REPLACEMENT_CHARACTER);
                        consumed = self.pending.len();
                    } else {
                        break;
                    }
                }
            }
        }

        self.pending.drain(..consumed);
        decoded
    }
}

/// Forwards everything `reader` produces to `sender`, a chunk at a time.
fn forward(mut reader: impl Read, sender: &mut mpsc::Sender<String>) -> io::Result<()> {
    let mut buf = [0_u8; CHUNK_SIZE];
    let mut decoder = Utf8LossyDecoder::default();

    loop {
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }

        let chunk = decoder.push(buf.get(..read).unwrap_or_default());
        if !chunk.is_empty() && block_on(sender.send(chunk)).is_err() {
            return Ok(());
        }
    }

    let chunk = decoder.finish();
    if !chunk.is_empty() {
        _ = block_on(sender.send(chunk));
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

    #[test]
    fn test_decoder_preserves_utf8_split_across_reads() {
        let mut decoder = Utf8LossyDecoder::default();

        assert_eq!(decoder.push(&[0xF0, 0x9F]), "");
        assert_eq!(decoder.push(&[0x98, 0x80]), "😀");
        assert_eq!(decoder.finish(), "");
    }

    #[test]
    fn test_decoder_retains_incomplete_utf8_after_invalid_bytes() {
        let mut decoder = Utf8LossyDecoder::default();

        assert_eq!(decoder.push(&[0xFF, 0xF0, 0x9F]), "�");
        assert_eq!(decoder.push(&[0x98, 0x80]), "😀");
    }

    #[test]
    fn test_decoder_replaces_incomplete_utf8_at_end_of_stream() {
        let mut decoder = Utf8LossyDecoder::default();

        assert_eq!(decoder.push(&[0xF0, 0x9F]), "");
        assert_eq!(decoder.finish(), "�");
    }
}
