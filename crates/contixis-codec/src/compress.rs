use anyhow::Result;

/// Compress raw pixel data with zstd for clipboard / small payloads.
pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>> {
    Ok(zstd::bulk::compress(data, level)?)
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    Ok(zstd::bulk::decompress(data, 64 * 1024 * 1024)?)
}
