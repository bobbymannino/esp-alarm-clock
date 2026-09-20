use std::path::Path;

use anyhow::{Context as _, Result, bail};

const PAGE_SIZE: usize = 4096;
const HEADER_SIZE: usize = 32;
const ENTRY_STATE_BITMAP_SIZE: usize = 32;
const ENTRY_SIZE: usize = 32;
const ENTRIES_PER_PAGE: usize = 126;
const ENTRY_TABLE_OFFSET: usize = HEADER_SIZE + ENTRY_STATE_BITMAP_SIZE;
const KEY_SIZE: usize = 16;
const DATA_SIZE: usize = 8;

struct PageEntry {
    ns: u8,
    typ: u8,
    span: u8,
    chunk_index: u8,
    crc32: u32,
    key: String,
    data: [u8; DATA_SIZE],
}

struct PageHeader {
    state: u32,
    sequence: u32,
    /// This is reversed so `0xFF` is version 1, `0xFE` is version 2, etc.
    version: u8,
    crc32: u32,
}

struct Page {
    header: PageHeader,
    entries: Vec<PageEntry>,
}

impl TryFrom<&[u8]> for PageEntry {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() != ENTRY_SIZE {
            bail!("NVS entry must be {ENTRY_SIZE} bytes, got {}", value.len());
        }

        let key_bytes = value.get(8..8 + KEY_SIZE).context("NVS entry is missing its key")?;
        let key_end = key_bytes.iter().position(|&byte| byte == 0).unwrap_or(KEY_SIZE);
        let key = String::from_utf8_lossy(key_bytes.get(..key_end).context("NVS entry key has an invalid length")?).into_owned();

        Ok(Self {
            ns: byte_at(value, 0, "namespace")?,
            typ: byte_at(value, 1, "type")?,
            span: byte_at(value, 2, "span")?,
            chunk_index: byte_at(value, 3, "chunk index")?,
            crc32: read_u32_le(value, 4, "entry CRC")?,
            key,
            data: value
                .get(24..24 + DATA_SIZE)
                .context("NVS entry is missing its data")?
                .try_into()
                .context("NVS entry data has an invalid length")?,
        })
    }
}

impl TryFrom<&[u8]> for Page {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() != PAGE_SIZE {
            bail!("NVS page must be {PAGE_SIZE} bytes, got {}", value.len());
        }

        let header = PageHeader {
            state: read_u32_le(value, 0, "page state")?,
            sequence: read_u32_le(value, 4, "page sequence")?,
            version: byte_at(value, 8, "page version")?,
            crc32: read_u32_le(value, 28, "page CRC")?,
        };
        let entry_states = value
            .get(HEADER_SIZE..ENTRY_TABLE_OFFSET)
            .context("NVS page is missing its entry-state bitmap")?;
        let entry_data = value.get(ENTRY_TABLE_OFFSET..).context("NVS page is missing its entry table")?;
        let (entry_chunks, remainder) = entry_data.as_chunks::<ENTRY_SIZE>();
        if !remainder.is_empty() || entry_chunks.len() != ENTRIES_PER_PAGE {
            bail!("NVS page has an invalid entry table length");
        }

        let mut entries = Vec::new();
        for (index, bytes) in entry_chunks.iter().enumerate() {
            if entry_is_written(entry_states, index)? {
                entries.push(PageEntry::try_from(bytes.as_slice()).with_context(|| format!("failed to decode NVS entry {index}"))?);
            }
        }

        Ok(Self { header, entries })
    }
}

fn byte_at(bytes: &[u8], offset: usize, field: &str) -> Result<u8> {
    bytes.get(offset).copied().with_context(|| format!("NVS data is missing {field}"))
}

fn read_u32_le(bytes: &[u8], offset: usize, field: &str) -> Result<u32> {
    let end = offset.checked_add(size_of::<u32>()).context("NVS field offset overflowed")?;
    let encoded = bytes
        .get(offset..end)
        .with_context(|| format!("NVS data is missing {field}"))?
        .try_into()
        .with_context(|| format!("NVS {field} has an invalid length"))?;
    Ok(u32::from_le_bytes(encoded))
}

fn entry_is_written(states: &[u8], index: usize) -> Result<bool> {
    const ENTRIES_PER_STATE_BYTE: usize = 4;
    const SHIFTS: [u32; ENTRIES_PER_STATE_BYTE] = [0, 2, 4, 6];
    const WRITTEN: u8 = 0b10;

    let state_byte = byte_at(states, index / ENTRIES_PER_STATE_BYTE, "entry state")?;
    let shift = SHIFTS
        .get(index % ENTRIES_PER_STATE_BYTE)
        .copied()
        .context("NVS entry state index is invalid")?;

    Ok((state_byte >> shift) & 0b11 == WRITTEN)
}

pub(super) fn decode_flash(nvs_path: &Path) -> Result<()> {
    let nvs_data = std::fs::read(nvs_path).with_context(|| format!("failed to read NVS data from {}", nvs_path.display()))?;
    let (page_chunks, remainder) = nvs_data.as_chunks::<PAGE_SIZE>();
    if !remainder.is_empty() {
        bail!(
            "NVS data length {} is not a multiple of the {PAGE_SIZE}-byte page size",
            nvs_data.len()
        );
    }

    println!("nvs_data byte count: {}", nvs_data.len());
    println!("page count: {}", page_chunks.len());

    for (index, bytes) in page_chunks.iter().enumerate() {
        let page = Page::try_from(bytes.as_slice()).with_context(|| format!("failed to decode NVS page {index}"))?;
        println!("{index}.");
        println!(
            "  state={:#010X}, sequence={}, version={:#04X}, crc32={:#010X}",
            page.header.state, page.header.sequence, page.header.version, page.header.crc32
        );
        for entry in &page.entries {
            println!(
                "  ns={}, type={:#04X}, span={}, chunk={}, crc32={:#010X}, key={:?}, data={:02X?}",
                entry.ns, entry.typ, entry.span, entry.chunk_index, entry.crc32, entry.key, entry.data
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoding_pages() {
        const PATH: &str = "../../nvs.bin";

        let path = Path::new(PATH);
        if !path.exists() {
            panic!("Path {PATH} does not exist");
        }

        _ = decode_flash(path).expect(format!("Failed to decode {PATH}").as_str());
    }
}
