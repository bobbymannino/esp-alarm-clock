use std::fmt;
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

const TYPE_U8: u8 = 0x01;
const TYPE_I8: u8 = 0x11;
const TYPE_U16: u8 = 0x02;
const TYPE_I16: u8 = 0x12;
const TYPE_U32: u8 = 0x04;
const TYPE_I32: u8 = 0x14;
const TYPE_U64: u8 = 0x08;
const TYPE_I64: u8 = 0x18;
const TYPE_STR: u8 = 0x21;
/// Pre-chunked blobs, still written by older NVS versions.
const TYPE_BLOB: u8 = 0x41;
const TYPE_BLOB_DATA: u8 = 0x42;
const TYPE_BLOB_IDX: u8 = 0x48;

/// The decoded payload of an entry, resolved from its type byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryValue {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    Str(String),
    Blob(Vec<u8>),
    /// The index entry that describes how many `Blob` chunks make up one blob.
    BlobIndex {
        size: u32,
        chunk_count: u8,
        chunk_start: u8,
    },
    Unknown {
        typ: u8,
        data: [u8; DATA_SIZE],
    },
}

impl fmt::Display for EntryValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::U8(value) => write!(formatter, "u8({value})"),
            Self::I8(value) => write!(formatter, "i8({value})"),
            Self::U16(value) => write!(formatter, "u16({value})"),
            Self::I16(value) => write!(formatter, "i16({value})"),
            Self::U32(value) => write!(formatter, "u32({value})"),
            Self::I32(value) => write!(formatter, "i32({value})"),
            Self::U64(value) => write!(formatter, "u64({value})"),
            Self::I64(value) => write!(formatter, "i64({value})"),
            Self::Str(value) => write!(formatter, "str({value:?})"),
            Self::Blob(value) => write!(formatter, "blob({} bytes, {value:02X?})", value.len()),
            Self::BlobIndex {
                size,
                chunk_count,
                chunk_start,
            } => write!(formatter, "blob_index(size={size}, chunks={chunk_count}, start={chunk_start})"),
            Self::Unknown { typ, data } => write!(formatter, "unknown(type={typ:#04X}, data={data:02X?})"),
        }
    }
}

/// An entry exactly as it is laid out on flash, before its value is resolved.
struct RawEntry {
    ns: u8,
    typ: u8,
    span: u8,
    chunk_index: u8,
    crc32: u32,
    key: String,
    data: [u8; DATA_SIZE],
}

pub struct PageEntry {
    ns: u8,
    span: u8,
    chunk_index: u8,
    crc32: u32,
    key: String,
    value: EntryValue,
}

impl PageEntry {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &EntryValue {
        &self.value
    }
}

pub struct PageHeader {
    state: u32,
    sequence: u32,
    /// This is reversed so `0xFF` is version 1, `0xFE` is version 2, etc.
    version: u8,
    crc32: u32,
}

pub struct Page {
    header: PageHeader,
    entries: Vec<PageEntry>,
}

impl Page {
    pub fn entries(&self) -> &Vec<PageEntry> {
        &self.entries
    }
}

impl TryFrom<&[u8]> for RawEntry {
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

impl PageEntry {
    /// `payload` holds the bytes of the entries this one spans, which is empty for fixed-size types.
    fn from_raw(raw: RawEntry, payload: &[u8]) -> Result<Self> {
        let value =
            decode_value(raw.typ, raw.data, payload).with_context(|| format!("failed to decode the value of NVS key {:?}", raw.key))?;

        Ok(Self {
            ns: raw.ns,
            span: raw.span,
            chunk_index: raw.chunk_index,
            crc32: raw.crc32,
            key: raw.key,
            value,
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
        let mut index = 0;
        while let Some(bytes) = entry_chunks.get(index) {
            let next = index.checked_add(1).context("NVS entry index overflowed")?;
            if !entry_is_written(entry_states, index)? {
                index = next;
                continue;
            }

            let raw = RawEntry::try_from(bytes.as_slice()).with_context(|| format!("failed to decode NVS entry {index}"))?;
            // A span of zero would never advance the loop, so treat it the same as a lone entry.
            let span = usize::from(raw.span.max(1));
            let end = index.checked_add(span).context("NVS entry span overflowed")?;
            let payload = entry_chunks
                .get(next..end)
                .with_context(|| format!("NVS entry {index} spans past the end of the page"))?
                .concat();

            entries.push(PageEntry::from_raw(raw, &payload).with_context(|| format!("failed to decode NVS entry {index}"))?);
            index = end;
        }

        Ok(Self { header, entries })
    }
}

fn decode_value(typ: u8, data: [u8; DATA_SIZE], payload: &[u8]) -> Result<EntryValue> {
    match typ {
        TYPE_U8 => Ok(EntryValue::U8(byte_at(&data, 0, "u8 value")?)),
        TYPE_I8 => Ok(EntryValue::I8(i8::from_le_bytes([byte_at(&data, 0, "i8 value")?]))),
        TYPE_U16 => Ok(EntryValue::U16(u16::from_le_bytes(fixed_bytes(data, "u16 value")?))),
        TYPE_I16 => Ok(EntryValue::I16(i16::from_le_bytes(fixed_bytes(data, "i16 value")?))),
        TYPE_U32 => Ok(EntryValue::U32(u32::from_le_bytes(fixed_bytes(data, "u32 value")?))),
        TYPE_I32 => Ok(EntryValue::I32(i32::from_le_bytes(fixed_bytes(data, "i32 value")?))),
        TYPE_U64 => Ok(EntryValue::U64(u64::from_le_bytes(fixed_bytes(data, "u64 value")?))),
        TYPE_I64 => Ok(EntryValue::I64(i64::from_le_bytes(fixed_bytes(data, "i64 value")?))),
        TYPE_STR => {
            let bytes = variable_payload(data, payload, "string")?;
            // The stored length counts the terminator, which does not belong in the decoded string.
            let end = bytes.iter().position(|&byte| byte == 0).unwrap_or(bytes.len());
            let text = bytes.get(..end).context("NVS string has an invalid length")?;
            Ok(EntryValue::Str(String::from_utf8_lossy(text).into_owned()))
        }
        TYPE_BLOB | TYPE_BLOB_DATA => Ok(EntryValue::Blob(variable_payload(data, payload, "blob")?.to_vec())),
        TYPE_BLOB_IDX => Ok(EntryValue::BlobIndex {
            size: read_u32_le(&data, 0, "blob size")?,
            chunk_count: byte_at(&data, 4, "blob chunk count")?,
            chunk_start: byte_at(&data, 5, "blob chunk start")?,
        }),
        _ => Ok(EntryValue::Unknown { typ, data }),
    }
}

fn fixed_bytes<const N: usize>(data: [u8; DATA_SIZE], field: &str) -> Result<[u8; N]> {
    data.get(..N)
        .with_context(|| format!("NVS entry is missing its {field}"))?
        .try_into()
        .with_context(|| format!("NVS {field} has an invalid length"))
}

/// Strings and blob chunks store their length up front and their bytes in the entries they span.
fn variable_payload<'a>(data: [u8; DATA_SIZE], payload: &'a [u8], field: &str) -> Result<&'a [u8]> {
    let size = usize::from(u16::from_le_bytes(fixed_bytes(data, field)?));
    payload
        .get(..size)
        .with_context(|| format!("NVS {field} of {size} bytes does not fit in the {} bytes it spans", payload.len()))
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

pub(super) fn decode_flash(nvs_path: &Path) -> Result<Vec<Page>> {
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

    let pages = page_chunks
        .iter()
        .filter_map(|chunk| Page::try_from(chunk.as_slice()).ok())
        .collect();

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(typ: u8, span: u8, key: &str, data: &[u8]) -> Result<[u8; ENTRY_SIZE]> {
        let mut entry = [0_u8; ENTRY_SIZE];
        *entry.get_mut(1).context("test entry type is missing")? = typ;
        *entry.get_mut(2).context("test entry span is missing")? = span;
        let key_end = 8_usize.checked_add(key.len()).context("test entry key is too long")?;
        entry
            .get_mut(8..key_end)
            .context("test entry key range is invalid")?
            .copy_from_slice(key.as_bytes());
        let data_end = 24_usize.checked_add(data.len()).context("test entry data is too long")?;
        entry
            .get_mut(24..data_end)
            .context("test entry data range is invalid")?
            .copy_from_slice(data);
        Ok(entry)
    }

    fn page_with_entries(entries: &[[u8; ENTRY_SIZE]]) -> Result<[u8; PAGE_SIZE]> {
        let mut page = [0xFF; PAGE_SIZE];
        page.get_mut(28..32)
            .context("test page CRC range is invalid")?
            .copy_from_slice(&0x1234_5678_u32.to_le_bytes());
        for (index, bytes) in entries.iter().enumerate() {
            // Clear the low bit of the entry's state, turning the erased `0b11` into written `0b10`.
            let state_index = HEADER_SIZE.checked_add(index / 4).context("test page bitmap index overflowed")?;
            let shift = u32::try_from(index % 4)?
                .checked_mul(2)
                .context("test page bitmap shift overflowed")?;
            let mask = 0b01_u8.checked_shl(shift).context("test page bitmap shift is invalid")?;
            *page.get_mut(state_index).context("test page bitmap is missing")? &= !mask;
            let offset = index
                .checked_mul(ENTRY_SIZE)
                .and_then(|offset| offset.checked_add(ENTRY_TABLE_OFFSET))
                .context("test page entry offset overflowed")?;
            let end = offset.checked_add(ENTRY_SIZE).context("test page entry end overflowed")?;
            page.get_mut(offset..end)
                .context("test page entry range is invalid")?
                .copy_from_slice(bytes);
        }
        Ok(page)
    }

    fn page_with_first_entry(entry: &[u8; ENTRY_SIZE]) -> Result<[u8; PAGE_SIZE]> {
        page_with_entries(std::slice::from_ref(entry))
    }

    fn size_field(size: usize) -> Result<[u8; DATA_SIZE]> {
        let mut data = [0_u8; DATA_SIZE];
        data.get_mut(..2)
            .context("test size field range is invalid")?
            .copy_from_slice(&u16::try_from(size)?.to_le_bytes());
        Ok(data)
    }

    #[test]
    fn decodes_full_key_and_page_crc() -> Result<()> {
        let page = Page::try_from(page_with_first_entry(&entry(TYPE_U8, 1, "sixteen-byte-key", &[7])?)?.as_slice())?;

        anyhow::ensure!(page.header.crc32 == 0x1234_5678, "decoded the wrong page CRC");
        anyhow::ensure!(page.entries.len() == 1, "decoded the wrong number of entries");
        let first = page.entries.first().context("decoded page has no entries")?;
        anyhow::ensure!(first.key == "sixteen-byte-key", "decoded the wrong entry key");
        anyhow::ensure!(first.value == EntryValue::U8(7), "decoded the wrong entry value");
        Ok(())
    }

    #[test]
    fn decodes_fixed_width_numbers() -> Result<()> {
        let page = Page::try_from(
            page_with_entries(&[
                entry(TYPE_I8, 1, "i8", &(-3_i8).to_le_bytes())?,
                entry(TYPE_U16, 1, "u16", &4_000_u16.to_le_bytes())?,
                entry(TYPE_I32, 1, "i32", &(-70_000_i32).to_le_bytes())?,
                entry(TYPE_U64, 1, "u64", &u64::MAX.to_le_bytes())?,
            ])?
            .as_slice(),
        )?;

        let values: Vec<_> = page.entries.iter().map(|entry| entry.value.clone()).collect();
        anyhow::ensure!(
            values
                == [
                    EntryValue::I8(-3),
                    EntryValue::U16(4_000),
                    EntryValue::I32(-70_000),
                    EntryValue::U64(u64::MAX),
                ],
            "decoded the wrong values: {values:?}"
        );
        Ok(())
    }

    #[test]
    fn decodes_a_string_from_the_entries_it_spans() -> Result<()> {
        let text = "a string that is longer than one entry can hold";
        // The stored size counts the terminating NUL byte.
        let size = size_field(text.len().checked_add(1).context("test string is too long")?)?;
        let mut entries = vec![entry(TYPE_STR, 3, "text", &size)?];
        for chunk in text.as_bytes().chunks(ENTRY_SIZE) {
            let mut payload = [0_u8; ENTRY_SIZE];
            payload
                .get_mut(..chunk.len())
                .context("test payload range is invalid")?
                .copy_from_slice(chunk);
            entries.push(payload);
        }

        let page = Page::try_from(page_with_entries(&entries)?.as_slice())?;

        anyhow::ensure!(page.entries.len() == 1, "the spanned entries were decoded separately");
        let first = page.entries.first().context("decoded page has no entries")?;
        anyhow::ensure!(first.value == EntryValue::Str(text.to_owned()), "decoded the wrong string");
        Ok(())
    }

    #[test]
    fn decodes_a_blob_chunk() -> Result<()> {
        let blob = [0xDE_u8, 0xAD, 0xBE, 0xEF];
        let mut payload = [0_u8; ENTRY_SIZE];
        payload
            .get_mut(..blob.len())
            .context("test payload range is invalid")?
            .copy_from_slice(&blob);
        let page = Page::try_from(page_with_entries(&[entry(TYPE_BLOB_DATA, 2, "blob", &size_field(blob.len())?)?, payload])?.as_slice())?;

        anyhow::ensure!(
            page.entries.first().context("decoded page has no entries")?.value == EntryValue::Blob(blob.to_vec()),
            "decoded the wrong blob"
        );
        Ok(())
    }

    #[test]
    fn decodes_a_blob_index() -> Result<()> {
        let mut data = [0_u8; DATA_SIZE];
        data.get_mut(..4)
            .context("test blob size range is invalid")?
            .copy_from_slice(&600_u32.to_le_bytes());
        *data.get_mut(4).context("test blob chunk count is missing")? = 2;
        *data.get_mut(5).context("test blob chunk start is missing")? = 1;
        let page = Page::try_from(page_with_first_entry(&entry(TYPE_BLOB_IDX, 1, "blob", &data)?)?.as_slice())?;

        anyhow::ensure!(
            page.entries.first().context("decoded page has no entries")?.value
                == EntryValue::BlobIndex {
                    size: 600,
                    chunk_count: 2,
                    chunk_start: 1,
                },
            "decoded the wrong blob index"
        );
        Ok(())
    }

    #[test]
    fn rejects_a_string_longer_than_its_span() -> Result<()> {
        let page = page_with_first_entry(&entry(TYPE_STR, 2, "text", &size_field(100)?)?)?;

        let result = Page::try_from(page.as_slice());

        anyhow::ensure!(
            result.is_err_and(|error| format!("{error:#}").contains("does not fit")),
            "accepted a string longer than its span"
        );
        Ok(())
    }

    #[test]
    fn ignores_entries_not_marked_written() -> Result<()> {
        let page = Page::try_from([0xFF; PAGE_SIZE].as_slice())?;

        anyhow::ensure!(page.entries.is_empty(), "decoded an unwritten entry");
        Ok(())
    }

    #[test]
    fn rejects_incorrect_page_size() {
        let result = Page::try_from([0_u8; PAGE_SIZE - 1].as_slice());

        assert!(result.is_err_and(|error| error.to_string().contains("must be 4096 bytes")));
    }
}
