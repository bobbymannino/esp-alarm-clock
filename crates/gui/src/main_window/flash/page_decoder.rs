use std::path::Path;

use anyhow::Result;

const PAGE_SIZE: usize = 4096;
const ENTRY_SIZE: usize = 32;
const ENTRIES_PER_PAGE: usize = 126;
const ENTRY_TABLE_OFFSET: usize = 64;

#[derive(Debug)]
pub struct PageEntry {
    ns: u8,
    typ: u8,
    span: u8,
    chunk_index: u8,
    crc32: u32,
    /// 16 bytes.
    key: String,
    data: [u8; 8],
}

#[derive(Debug)]
pub struct PageHeader {
    state: u32,
    sequence: u32,
    /// This is reversed so 0xFF is version 1, 0xFE is version 2, etc.
    version: u8,
    crc32: u32,
}

#[derive(Debug)]
pub struct Page {
    header: PageHeader,
    entries: [PageEntry; ENTRIES_PER_PAGE],
}

impl From<&[u8]> for PageEntry {
    fn from(value: &[u8]) -> Self {
        Self {
            ns: value[0],
            typ: value[1],
            span: value[2],
            chunk_index: value[3],
            crc32: u32::from_le_bytes([value[4], value[5], value[6], value[7]]),
            key: String::from_utf8_lossy(&value[8..23]).to_string(),
            data: value[24..].try_into().expect("Page entry data is invalid length"),
        }
    }
}

impl TryFrom<&[u8]> for Page {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> std::prelude::v1::Result<Self, Self::Error> {
        let header = PageHeader {
            state: u32::from_le_bytes([value[0], value[1], value[2], value[3]]),
            sequence: u32::from_le_bytes([value[4], value[5], value[6], value[7]]),
            version: value[8],
            crc32: u32::from_le_bytes([value[9], value[10], value[11], value[12]]),
        };

        let entries: Vec<PageEntry> = value[ENTRY_TABLE_OFFSET..].chunks_exact(ENTRY_SIZE).map(PageEntry::from).collect();
        let Ok(entries) = entries.try_into() else {
            anyhow::bail!("Failed to convert pages");
        };

        Ok(Page { header, entries })
    }
}

pub(super) fn decode_flash(nvs_path: &Path) -> Result<()> {
    let nvs_data = std::fs::read(nvs_path)?;
    println!("nvs_data byte count: {}", nvs_data.len());

    let pages = nvs_data
        .chunks_exact(PAGE_SIZE)
        .filter_map(|chunk| Page::try_from(chunk).ok())
        .collect::<Vec<_>>();
    println!("page count: {}", pages.len());

    for (index, page) in pages.iter().enumerate() {
        println!("{index}.");
        println!("  {:?}", page.header);
        for entry in page.entries.iter().filter(|entry| entry.data.iter().any(|b| !b.eq(&0))) {
            println!("  {:?}", entry);
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
