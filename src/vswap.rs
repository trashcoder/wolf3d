use std::{error::Error, fmt, fs, path::Path};

use crate::resources::find_data_file;

const CHUNK_HEADER_SIZE: usize = 6;
pub const WALL_TEXTURE_BYTES: usize = 64 * 64;

#[derive(Debug, Clone, Copy)]
struct Chunk {
    offset: usize,
    length: usize,
}

#[derive(Debug)]
struct VswapError(String);

impl fmt::Display for VswapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for VswapError {}

/// Parsed VSWAP page directory. Wall pages are indexed as in the original
/// renderer: each map wall has a horizontal and a vertical page.
pub struct Vswap {
    bytes: Vec<u8>,
    chunks: Vec<Chunk>,
    wall_page_count: usize,
    sound_page_start: usize,
}

impl Vswap {
    pub fn load(data_dir: &Path, extension: &str) -> Result<Self, Box<dyn Error>> {
        let bytes = fs::read(find_data_file(data_dir, &format!("VSWAP.{extension}"))?)?;
        if bytes.len() < CHUNK_HEADER_SIZE {
            return Err(VswapError("VSWAP file is shorter than its header".into()).into());
        }
        let chunk_count = read_u16(&bytes, 0)? as usize;
        let wall_page_count = read_u16(&bytes, 2)? as usize;
        let sound_start = read_u16(&bytes, 4)? as usize;
        if chunk_count == 0 || wall_page_count > sound_start || sound_start > chunk_count {
            return Err(VswapError("VSWAP has inconsistent page counts".into()).into());
        }
        let offsets_start = CHUNK_HEADER_SIZE;
        let lengths_start = offsets_start
            .checked_add(
                chunk_count
                    .checked_mul(4)
                    .ok_or_else(|| VswapError("VSWAP offset table size overflow".into()))?,
            )
            .ok_or_else(|| VswapError("VSWAP table offset overflow".into()))?;
        let data_start = lengths_start
            .checked_add(
                chunk_count
                    .checked_mul(2)
                    .ok_or_else(|| VswapError("VSWAP length table size overflow".into()))?,
            )
            .ok_or_else(|| VswapError("VSWAP table size overflow".into()))?;
        if data_start > bytes.len() {
            return Err(VswapError("VSWAP page directory is truncated".into()).into());
        }
        let mut chunks = Vec::with_capacity(chunk_count);
        for index in 0..chunk_count {
            let offset = read_u32(&bytes, offsets_start + index * 4)? as usize;
            let length = read_u16(&bytes, lengths_start + index * 2)? as usize;
            if length > 0
                && (offset < data_start
                    || offset
                        .checked_add(length)
                        .is_none_or(|end| end > bytes.len()))
            {
                return Err(
                    VswapError(format!("VSWAP page {index} extends beyond the file")).into(),
                );
            }
            chunks.push(Chunk { offset, length });
        }
        Ok(Self {
            bytes,
            chunks,
            wall_page_count,
            sound_page_start: sound_start,
        })
    }

    pub fn wall_texture(&self, page: usize) -> Option<&[u8]> {
        if page >= self.wall_page_count {
            return None;
        }
        let chunk = *self.chunks.get(page)?;
        if chunk.length < WALL_TEXTURE_BYTES {
            return None;
        }
        self.bytes
            .get(chunk.offset..chunk.offset + WALL_TEXTURE_BYTES)
    }

    /// The last VSWAP page contains (relative start page, PCM byte count) pairs.
    pub(crate) fn digitized_sample(&self, sample: usize) -> Option<Vec<u8>> {
        let directory_index = self.chunks.len().checked_sub(1)?;
        let directory = self.chunks[directory_index];
        let entry = sample.checked_mul(4)?;
        if entry.checked_add(4)? > directory.length {
            return None;
        }
        let offset = directory.offset + entry;
        let start = usize::from(read_u16(&self.bytes, offset).ok()?);
        let length = usize::from(read_u16(&self.bytes, offset + 2).ok()?);
        if length == 0 {
            return None;
        }
        let mut page = self.sound_page_start.checked_add(start)?;
        let mut pcm = Vec::with_capacity(length);
        while pcm.len() < length && page < directory_index {
            let chunk = self.chunks[page];
            if chunk.length == 0 {
                return None;
            }
            let count = chunk.length.min(length - pcm.len());
            pcm.extend_from_slice(self.bytes.get(chunk.offset..chunk.offset + count)?);
            page += 1;
        }
        (pcm.len() == length).then_some(pcm)
    }

    pub fn sprite_start(&self) -> usize {
        self.wall_page_count
    }

    pub fn sprite_page(&self, sprite: usize) -> Option<&[u8]> {
        let page = self.wall_page_count.checked_add(sprite)?;
        if page >= self.sound_page_start {
            return None;
        }
        let chunk = *self.chunks.get(page)?;
        if chunk.length == 0 {
            return None;
        }
        self.bytes.get(chunk.offset..chunk.offset + chunk.length)
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, VswapError> {
    let pair = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| VswapError("truncated VSWAP directory".into()))?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, VswapError> {
    let quad = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| VswapError("truncated VSWAP directory".into()))?;
    Ok(u32::from_le_bytes([quad[0], quad[1], quad[2], quad[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reads_a_wall_page_from_vswap() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wolf3d-vswap-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let data_start = 6 + 4 + 2;
        let mut bytes = Vec::new();
        bytes.extend(1u16.to_le_bytes()); // chunks
        bytes.extend(1u16.to_le_bytes()); // sprite start
        bytes.extend(1u16.to_le_bytes()); // sound start
        bytes.extend((data_start as u32).to_le_bytes());
        bytes.extend((WALL_TEXTURE_BYTES as u16).to_le_bytes());
        bytes.extend((0..WALL_TEXTURE_BYTES).map(|n| n as u8));
        fs::write(dir.join("VSWAP.WL6"), bytes).unwrap();
        let archive = Vswap::load(&dir, "WL6").unwrap();
        let texture = archive.wall_texture(0).unwrap();
        assert_eq!(texture.len(), WALL_TEXTURE_BYTES);
        assert_eq!(texture[255], 255);
        fs::remove_dir_all(dir).unwrap();
    }
}
