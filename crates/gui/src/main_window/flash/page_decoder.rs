use std::path::Path;

use anyhow::Result;

#[derive(Debug)]
pub struct Entry {
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
    // entries: [Entry; 126],
}

impl From<&[u8]> for Page {
    fn from(value: &[u8]) -> Self {
        let header = PageHeader {
            state: value[0..3].into(),
            sequence: 0,
            version: value[8],
            crc32: 0,
        };

        Page { header }
    }
}

pub(super) fn decode_flash(nvs_path: &Path) -> Result<()> {
    /// Page size in bytes
    const PAGE_SIZE: usize = 4096;

    let nvs_data = std::fs::read(nvs_path)?;
    println!("nvs_data byte count: {}", nvs_data.len());

    let pages = nvs_data.chunks_exact(PAGE_SIZE).map(Page::from);
    println!("page count: {}", pages.len());

    for (index, page) in pages.enumerate() {
        println!("{index}.");
        println!("  {:?}", page.header);
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
