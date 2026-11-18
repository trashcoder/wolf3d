use std::{error::Error, fmt, fs, path::Path};

use crate::resources::find_data_file;

const HUFFMAN_NODES: usize = 255;
const HUFFMAN_ROOT: usize = 254;
const SPARSE_OFFSET: u32 = 0x00FF_FFFF;
const MAX_CHUNK_BYTES: usize = 16 * 1024 * 1024;
const START_PICS: usize = 3;
const MAX_PICTURE_PIXELS: usize = 1024 * 1024;
const WL1_SOURCE_PICTURE_COUNT: usize = 136;
const WL1_EXTENDED_PICTURE_COUNT: usize = 144;

#[derive(Debug)]
struct VgaError(String);

impl fmt::Display for VgaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for VgaError {}

#[derive(Clone, Copy)]
struct Node {
    bit0: u16,
    bit1: u16,
}

#[derive(Clone, Debug)]
struct Glyph {
    width: usize,
    bitmap: Vec<u8>,
}

/// Decoded proportional BIOS-style font stored in VGA graphics chunk 1.
#[derive(Clone, Debug)]
pub struct BitmapFont {
    height: usize,
    glyphs: Vec<Glyph>,
}

#[derive(Clone, Debug)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl BitmapFont {
    pub fn decode(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        const FONT_HEADER: usize = 2 + 256 * 2 + 256;
        if data.len() < FONT_HEADER {
            return Err(VgaError("VGA font chunk is truncated".into()).into());
        }
        let height = u16::from_le_bytes([data[0], data[1]]) as usize;
        if height == 0 || height > 64 {
            return Err(VgaError("VGA font has an invalid glyph height".into()).into());
        }
        let mut glyphs = Vec::with_capacity(256);
        for ch in 0..256 {
            let location_offset = 2 + ch * 2;
            let location =
                u16::from_le_bytes([data[location_offset], data[location_offset + 1]]) as usize;
            let signed_width = data[2 + 256 * 2 + ch] as i8;
            let width = signed_width.max(0) as usize;
            if width == 0 {
                glyphs.push(Glyph {
                    width: 0,
                    bitmap: Vec::new(),
                });
                continue;
            }
            let length = width
                .checked_mul(height)
                .ok_or_else(|| VgaError("VGA font glyph size overflow".into()))?;
            let end = location
                .checked_add(length)
                .ok_or_else(|| VgaError("VGA font glyph range overflow".into()))?;
            let bitmap = data
                .get(location..end)
                .ok_or_else(|| VgaError(format!("VGA font glyph {ch} is out of bounds")))?
                .to_vec();
            glyphs.push(Glyph { width, bitmap });
        }
        Ok(Self { height, glyphs })
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn text_width(&self, text: &str) -> usize {
        text.bytes()
            .filter_map(|byte| self.glyphs.get(byte as usize))
            .map(|glyph| glyph.width)
            .sum()
    }

    pub fn draw_text(
        &self,
        pixels: &mut [u32],
        canvas_width: usize,
        canvas_height: usize,
        x: usize,
        y: usize,
        text: &str,
        color: u32,
    ) {
        let mut cursor_x = x;
        let mut cursor_y = y;
        for byte in text.bytes() {
            if byte == b'\n' {
                cursor_x = x;
                cursor_y = cursor_y.saturating_add(self.height);
                if cursor_y >= canvas_height {
                    break;
                }
                continue;
            }
            if cursor_x >= canvas_width {
                continue;
            }
            let Some(glyph) = self.glyphs.get(byte as usize) else {
                continue;
            };
            for row in 0..self.height {
                for column in 0..glyph.width {
                    if glyph.bitmap[row * glyph.width + column] != 0 {
                        let px = cursor_x + column;
                        let py = cursor_y + row;
                        if px < canvas_width && py < canvas_height {
                            pixels[py * canvas_width + px] = color;
                        }
                    }
                }
            }
            cursor_x = cursor_x.saturating_add(glyph.width);
        }
    }
}

/// The three original VGA resource files: Huffman dictionary, 24-bit chunk
/// offsets, and compressed graphics data.
pub struct VgaArchive {
    dictionary: Vec<Node>,
    offsets: Vec<u32>,
    graph: Vec<u8>,
    picture_sizes: Vec<(usize, usize)>,
    wl1_extended_picture_layout: bool,
}

impl VgaArchive {
    pub fn load(directory: &Path, extension: &str) -> Result<Self, Box<dyn Error>> {
        let dict_bytes = fs::read(find_data_file(directory, &format!("VGADICT.{extension}"))?)?;
        let head = fs::read(find_data_file(directory, &format!("VGAHEAD.{extension}"))?)?;
        let graph = fs::read(find_data_file(directory, &format!("VGAGRAPH.{extension}"))?)?;
        if dict_bytes.len() < HUFFMAN_NODES * 4 || head.len() < 6 || head.len() % 3 != 0 {
            return Err(VgaError("VGA dictionary or chunk directory is truncated".into()).into());
        }
        let dictionary = (0..HUFFMAN_NODES)
            .map(|index| Node {
                bit0: u16::from_le_bytes([dict_bytes[index * 4], dict_bytes[index * 4 + 1]]),
                bit1: u16::from_le_bytes([dict_bytes[index * 4 + 2], dict_bytes[index * 4 + 3]]),
            })
            .collect();
        let offsets = head
            .chunks_exact(3)
            .map(|bytes| bytes[0] as u32 | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16))
            .collect();
        let mut archive = Self {
            dictionary,
            offsets,
            graph,
            picture_sizes: Vec::new(),
            wl1_extended_picture_layout: false,
        };
        let picture_table = archive
            .chunk(0)?
            .ok_or_else(|| VgaError("VGA picture table chunk is sparse".into()))?;
        if picture_table.is_empty() || picture_table.len() % 4 != 0 {
            return Err(VgaError("VGA picture table has an invalid length".into()).into());
        }
        for dimensions in picture_table.chunks_exact(4) {
            let width = u16::from_le_bytes([dimensions[0], dimensions[1]]) as usize;
            let height = u16::from_le_bytes([dimensions[2], dimensions[3]]) as usize;
            let pixels = width
                .checked_mul(height)
                .ok_or_else(|| VgaError("VGA picture dimensions overflow".into()))?;
            if width == 0 || height == 0 || pixels > MAX_PICTURE_PIXELS {
                return Err(VgaError("VGA picture has invalid dimensions".into()).into());
            }
            archive.picture_sizes.push((width, height));
        }
        archive.wl1_extended_picture_layout = extension.eq_ignore_ascii_case("WL1")
            && archive.picture_sizes.len() == WL1_EXTENDED_PICTURE_COUNT;
        Ok(archive)
    }

    pub fn chunk_count(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    pub fn font(&self, index: usize) -> Result<BitmapFont, Box<dyn Error>> {
        let chunk = self
            .chunk(index)?
            .ok_or_else(|| VgaError(format!("VGA font chunk {index} is sparse")))?;
        BitmapFont::decode(&chunk)
    }

    pub fn picture(&self, chunk_index: usize) -> Result<Option<Picture>, Box<dyn Error>> {
        let archive_chunk_index = self.picture_archive_chunk(chunk_index);
        let Some(picture_index) = archive_chunk_index.checked_sub(START_PICS) else {
            return Err(VgaError(format!("VGA chunk {chunk_index} is not a picture")).into());
        };
        let (width, height) = *self
            .picture_sizes
            .get(picture_index)
            .ok_or_else(|| VgaError(format!("VGA picture chunk {chunk_index} is out of range")))?;
        let Some(pixels) = self.chunk(archive_chunk_index)? else {
            return Ok(None);
        };
        let required = width * height;
        if pixels.len() < required {
            return Err(VgaError(format!(
                "VGA picture chunk {chunk_index} is shorter than its dimensions"
            ))
            .into());
        }
        if width % 4 != 0 {
            return Err(VgaError(format!(
                "VGA picture chunk {chunk_index} width is not divisible by four"
            ))
            .into());
        }
        let plane_width = width / 4;
        let plane_size = plane_width * height;
        let mut chunky = vec![0; required];
        for y in 0..height {
            for x in 0..width {
                let plane = x & 3;
                let plane_x = x >> 2;
                let planar_offset = plane * plane_size + y * plane_width + plane_x;
                chunky[y * width + x] = pixels[planar_offset];
            }
        }
        Ok(Some(Picture {
            width,
            height,
            pixels: chunky,
        }))
    }

    fn picture_archive_chunk(&self, source_chunk: usize) -> usize {
        if !self.wl1_extended_picture_layout {
            return source_chunk;
        }

        // This WL1 archive contains eight more pictures than the source
        // GFXV_WL1.EQU table. One was inserted after C_OPTIONSPIC and seven
        // before STATUSBARPIC, shifting the original picture IDs in those
        // ranges while leaving the opening menu artwork untouched.
        match source_chunk {
            0..=21 => source_chunk,
            22..=89 => source_chunk + 1,
            _ => source_chunk + (WL1_EXTENDED_PICTURE_COUNT - WL1_SOURCE_PICTURE_COUNT),
        }
    }

    pub fn chunk(&self, index: usize) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        let start = *self
            .offsets
            .get(index)
            .ok_or_else(|| VgaError(format!("VGA chunk {index} is outside the directory")))?;
        let end = *self
            .offsets
            .get(index + 1)
            .ok_or_else(|| VgaError(format!("VGA chunk {index} has no following offset")))?;
        if start == SPARSE_OFFSET || end == SPARSE_OFFSET {
            return Ok(None);
        }
        if start > end || end as usize > self.graph.len() || end as usize > u32::MAX as usize {
            return Err(VgaError(format!("VGA chunk {index} has invalid offsets")).into());
        }
        let compressed = &self.graph[start as usize..end as usize];
        let expanded_length = u32_at(compressed, 0)? as usize;
        if expanded_length > MAX_CHUNK_BYTES {
            return Err(VgaError(format!(
                "VGA chunk {index} exceeds the supported size limit"
            ))
            .into());
        }
        let decoded = huffman_expand(&compressed[4..], expanded_length, &self.dictionary)?;
        Ok(Some(decoded))
    }
}

fn huffman_expand(
    input: &[u8],
    output_len: usize,
    dictionary: &[Node],
) -> Result<Vec<u8>, VgaError> {
    if dictionary.len() != HUFFMAN_NODES {
        return Err(VgaError(
            "Huffman dictionary has the wrong node count".into(),
        ));
    }
    let mut output = Vec::with_capacity(output_len);
    let mut byte_index = 0usize;
    let mut bit_mask = 1u8;
    let mut node_index = HUFFMAN_ROOT;
    let mut depth = 0usize;
    while output.len() < output_len {
        let source_byte = *input
            .get(byte_index)
            .ok_or_else(|| VgaError("compressed VGA chunk ended early".into()))?;
        let branch = if source_byte & bit_mask == 0 {
            dictionary[node_index].bit0
        } else {
            dictionary[node_index].bit1
        };
        bit_mask <<= 1;
        if bit_mask == 0 {
            bit_mask = 1;
            byte_index += 1;
        }
        if branch < 256 {
            output.push(branch as u8);
            node_index = HUFFMAN_ROOT;
            depth = 0;
        } else {
            node_index = (branch - 256) as usize;
            if node_index >= HUFFMAN_NODES || depth >= HUFFMAN_NODES {
                return Err(VgaError("invalid or cyclic VGA Huffman dictionary".into()));
            }
            depth += 1;
        }
    }
    Ok(output)
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, VgaError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| VgaError("VGA chunk is missing its expanded length".into()))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huffman_reads_bits_least_significant_first_and_restarts_at_root() {
        let mut nodes = vec![Node { bit0: 0, bit1: 0 }; HUFFMAN_NODES];
        nodes[HUFFMAN_ROOT] = Node { bit0: 17, bit1: 29 };
        let decoded = huffman_expand(&[0b0000_0010], 2, &nodes).unwrap();
        assert_eq!(decoded, [17, 29]);
    }

    #[test]
    fn rejects_child_node_outside_dictionary() {
        let mut nodes = vec![Node { bit0: 0, bit1: 0 }; HUFFMAN_NODES];
        nodes[HUFFMAN_ROOT] = Node { bit0: 511, bit1: 0 };
        assert!(huffman_expand(&[0], 1, &nodes).is_err());
    }

    #[test]
    fn reads_and_expands_a_vga_chunk() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wolf3d-vga-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let mut dictionary = vec![0u8; HUFFMAN_NODES * 4];
        dictionary[HUFFMAN_ROOT * 4..HUFFMAN_ROOT * 4 + 2].copy_from_slice(&42u16.to_le_bytes());
        fs::write(dir.join("VGADICT.WL6"), dictionary).unwrap();
        fs::write(dir.join("VGAHEAD.WL6"), [0, 0, 0, 5, 0, 0]).unwrap();
        fs::write(dir.join("VGAGRAPH.WL6"), [1, 0, 0, 0, 0]).unwrap();
        let archive = VgaArchive::load(&dir, "WL6").unwrap();
        assert_eq!(archive.chunk(0).unwrap(), Some(vec![42]));
        fs::remove_dir_all(dir).unwrap();
    }
}
