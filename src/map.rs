use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, fs, path::Path};

use crate::resources::find_data_file;

const MAP_LIMIT: usize = 64;
const MAP_HEADER_SIZE: usize = 38;
pub const EXIT_TILE: u16 = 99;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Map {
    pub name: String,
    pub number: usize,
    pub width: usize,
    pub height: usize,
    /// Plane 0 is the wall/floor map and plane 1 contains actor starts.
    pub planes: [Vec<u16>; 2],
}

impl Map {
    pub fn demo() -> Self {
        let rows = [
            "############",
            "#..........#",
            "#..##......#",
            "#..#.......#",
            "#..#..###..#",
            "#.....#....#",
            "###...#....#",
            "#.....#....#",
            "#..####....#",
            "#..........#",
            "#..........#",
            "############",
        ];
        let width = rows[0].len();
        let height = rows.len();
        let mut walls = vec![0; MAP_LIMIT * MAP_LIMIT];
        for (y, row) in rows.iter().enumerate() {
            for (x, tile) in row.bytes().enumerate() {
                walls[y * MAP_LIMIT + x] = u16::from(tile == b'#');
            }
        }
        let mut objects = vec![0; MAP_LIMIT * MAP_LIMIT];
        objects[2 * MAP_LIMIT + 2] = 19;
        Self {
            name: "Development map".into(),
            number: 0,
            width,
            height,
            planes: [walls, objects],
        }
    }

    pub fn tile(&self, x: usize, y: usize) -> u16 {
        self.planes[0][y * MAP_LIMIT + x]
    }

    pub(crate) fn set_tile(&mut self, x: usize, y: usize, tile: u16) {
        self.planes[0][y * MAP_LIMIT + x] = tile;
    }

    pub fn info_tile(&self, x: usize, y: usize) -> u16 {
        self.planes[1][y * MAP_LIMIT + x]
    }

    pub fn player_start(&self) -> Option<(usize, usize, u16)> {
        (0..self.height).find_map(|y| {
            (0..self.width).find_map(|x| {
                let direction = self.planes[1][y * MAP_LIMIT + x];
                (19..=22).contains(&direction).then_some((x, y, direction))
            })
        })
    }
}

#[derive(Debug)]
struct MapError(String);

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for MapError {}

fn invalid(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(MapError(message.into()))
}

/// Reads the Carmackized map archive used by the original WL6 release.
pub fn load(data_dir: &Path, extension: &str, map_number: usize) -> Result<Map, Box<dyn Error>> {
    let maphead = fs::read(find_data_file(data_dir, &format!("MAPHEAD.{extension}"))?)?;
    let game_maps = fs::read(find_data_file(data_dir, &format!("GAMEMAPS.{extension}"))?)?;
    let rlew_tag = u16_at(&maphead, 0)?;
    let map_entry_start = map_number
        .checked_mul(4)
        .and_then(|offset| offset.checked_add(2))
        .ok_or_else(|| MapError("map index overflows the MAPHEAD offset".into()))?;
    let map_entry_end = map_entry_start
        .checked_add(4)
        .ok_or_else(|| MapError("MAPHEAD entry range overflows".into()))?;
    let map_entry = maphead.get(map_entry_start..map_entry_end).ok_or_else(|| {
        MapError(format!(
            "MAPHEAD.{extension} does not contain map index {map_number}"
        ))
    })?;
    let header_offset = u32_at(map_entry, 0)? as usize;
    if header_offset == u32::MAX as usize {
        return Err(invalid(format!(
            "map {map_number} is not present in this data set"
        )));
    }
    let header_end = header_offset
        .checked_add(MAP_HEADER_SIZE)
        .ok_or_else(|| MapError("map header range overflow".into()))?;
    let header = game_maps.get(header_offset..header_end).ok_or_else(|| {
        MapError(format!(
            "GAMEMAPS.{extension} contains a truncated map header"
        ))
    })?;

    let mut starts = [0usize; 2];
    let mut lengths = [0usize; 2];
    for plane in 0..2 {
        starts[plane] = u32_at(header, plane * 4)? as usize;
        lengths[plane] = u16_at(header, 12 + plane * 2)? as usize;
    }
    let width = u16_at(header, 18)? as usize;
    let height = u16_at(header, 20)? as usize;
    if width == 0 || height == 0 || width > MAP_LIMIT || height > MAP_LIMIT {
        return Err(invalid(format!(
            "map dimensions {width}x{height} are outside the supported 1..={MAP_LIMIT} range"
        )));
    }
    let name_bytes = &header[22..38];
    let name_end = name_bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name_bytes.len());
    let name = String::from_utf8_lossy(&name_bytes[..name_end])
        .trim()
        .to_owned();
    // Original map planes always occupy a 64x64 buffer and use a fixed row
    // stride, even when a map's declared playable dimensions are smaller.
    let plane_words = MAP_LIMIT * MAP_LIMIT;
    let decode = |plane: usize| {
        let end = starts[plane]
            .checked_add(lengths[plane])
            .ok_or_else(|| MapError("map plane range overflow".into()))?;
        let compressed = game_maps.get(starts[plane]..end).ok_or_else(|| {
            MapError(format!(
                "map plane {plane} extends beyond GAMEMAPS.{extension}"
            ))
        })?;
        decode_plane(compressed, rlew_tag, plane_words)
    };
    let planes = [decode(0)?, decode(1)?];
    Ok(Map {
        name,
        number: map_number,
        width,
        height,
        planes,
    })
}

pub fn map_count(data_dir: &Path, extension: &str) -> Result<usize, Box<dyn Error>> {
    let maphead = fs::read(find_data_file(data_dir, &format!("MAPHEAD.{extension}"))?)?;
    if maphead.len() < 2 || (maphead.len() - 2) % 4 != 0 {
        return Err(invalid(format!(
            "MAPHEAD.{extension} has an invalid length"
        )));
    }
    Ok((maphead.len() - 2) / 4)
}

fn decode_plane(input: &[u8], rlew_tag: u16, output_words: usize) -> Result<Vec<u16>, MapError> {
    let expanded_bytes = u16_at(input, 0).map_err(|e| MapError(e.to_string()))? as usize;
    if expanded_bytes % 2 != 0 {
        return Err(MapError("Carmack expanded size is not word aligned".into()));
    }
    let words = carmack_expand(&input[2..], expanded_bytes / 2)?;
    let (&rlew_bytes, compressed_words) = words
        .split_first()
        .ok_or_else(|| MapError("Carmack data did not contain an RLEW size".into()))?;
    if rlew_bytes as usize != output_words.saturating_mul(2) {
        return Err(MapError(
            "RLEW output size does not match the fixed 64x64 map plane".into(),
        ));
    }
    rlew_expand(compressed_words, rlew_tag, output_words)
}

fn carmack_expand(input: &[u8], output_words: usize) -> Result<Vec<u16>, MapError> {
    const NEAR_TAG: u16 = 0xA7;
    const FAR_TAG: u16 = 0xA8;
    let mut cursor = 0usize;
    let mut output = Vec::with_capacity(output_words);
    while output.len() < output_words {
        let code = u16_at(input, cursor).map_err(|e| MapError(e.to_string()))?;
        cursor += 2;
        let tag = code >> 8;
        let count = (code & 0xff) as usize;
        match (tag, count) {
            (NEAR_TAG, 0) | (FAR_TAG, 0) => {
                let low = *input
                    .get(cursor)
                    .ok_or_else(|| MapError("truncated Carmack escaped tag".into()))?
                    as u16;
                cursor += 1;
                output.push(code | low);
            }
            (NEAR_TAG, count) => {
                let offset = *input
                    .get(cursor)
                    .ok_or_else(|| MapError("truncated Carmack near reference".into()))?
                    as usize;
                cursor += 1;
                let start = output
                    .len()
                    .checked_sub(offset)
                    .ok_or_else(|| MapError("invalid Carmack near reference".into()))?;
                copy_reference(&mut output, start, count, output_words)?;
            }
            (FAR_TAG, count) => {
                let offset = u16_at(input, cursor).map_err(|e| MapError(e.to_string()))? as usize;
                cursor += 2;
                copy_reference(&mut output, offset, count, output_words)?;
            }
            _ => output.push(code),
        }
    }
    Ok(output)
}

fn copy_reference(
    output: &mut Vec<u16>,
    start: usize,
    count: usize,
    limit: usize,
) -> Result<(), MapError> {
    if start >= output.len() || output.len().saturating_add(count) > limit {
        return Err(MapError(
            "Carmack reference is outside the decoded buffer".into(),
        ));
    }
    for index in 0..count {
        let value = *output
            .get(start + index)
            .ok_or_else(|| MapError("invalid overlapping Carmack reference".into()))?;
        output.push(value);
    }
    Ok(())
}

fn rlew_expand(input: &[u16], tag: u16, output_words: usize) -> Result<Vec<u16>, MapError> {
    let mut cursor = 0;
    let mut output = Vec::with_capacity(output_words);
    while output.len() < output_words {
        let value = *input
            .get(cursor)
            .ok_or_else(|| MapError("truncated RLEW map plane".into()))?;
        cursor += 1;
        if value != tag {
            output.push(value);
            continue;
        }
        let count = *input
            .get(cursor)
            .ok_or_else(|| MapError("truncated RLEW run length".into()))?
            as usize;
        let run_value = *input
            .get(cursor + 1)
            .ok_or_else(|| MapError("truncated RLEW run value".into()))?;
        cursor += 2;
        if count == 0 || output.len().saturating_add(count) > output_words {
            return Err(MapError("invalid RLEW run length".into()));
        }
        output.resize(output.len() + count, run_value);
    }
    Ok(output)
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16, MapError> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| MapError("map-data offset overflows".into()))?;
    let bytes = data
        .get(offset..end)
        .ok_or_else(|| MapError("unexpected end of map data".into()))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32, MapError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| MapError("map-data offset overflows".into()))?;
    let bytes = data
        .get(offset..end)
        .ok_or_else(|| MapError("unexpected end of map data".into()))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rlew_expands_runs_and_literals() {
        let decoded = rlew_expand(&[0x1234, 0xABCD, 3, 0x5678, 0x9ABC], 0xABCD, 5).unwrap();
        assert_eq!(decoded, [0x1234, 0x5678, 0x5678, 0x5678, 0x9ABC]);
    }

    #[test]
    fn carmack_expands_overlapping_near_reference() {
        let mut data = Vec::new();
        data.extend_from_slice(&0x1111u16.to_le_bytes());
        data.extend_from_slice(&0xA703u16.to_le_bytes());
        data.push(1);
        let decoded = carmack_expand(&data, 4).unwrap();
        assert_eq!(decoded, [0x1111; 4]);
    }

    #[test]
    fn demo_map_has_a_player_start_and_wall_boundary() {
        let map = Map::demo();
        assert_eq!(map.player_start(), Some((2, 2, 19)));
        assert_ne!(map.tile(0, 0), 0);
    }

    #[test]
    fn loads_map_header_and_both_decoded_planes() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wolf3d-map-{unique}"));
        fs::create_dir_all(&dir).unwrap();

        let rlew_tag = 0xABCDu16;
        let mut first_plane = vec![0; MAP_LIMIT * MAP_LIMIT];
        first_plane[0] = 1;
        first_plane[3] = 2;
        let mut second_plane = vec![0; MAP_LIMIT * MAP_LIMIT];
        second_plane[0] = 19;
        let first = packed_plane(rlew_tag, &first_plane);
        let second = packed_plane(rlew_tag, &second_plane);
        let first_start = MAP_HEADER_SIZE as u32;
        let second_start = first_start + first.len() as u32;
        let mut header = vec![0u8; MAP_HEADER_SIZE];
        put_u32(&mut header, 0, first_start);
        put_u32(&mut header, 4, second_start);
        put_u16(&mut header, 12, first.len() as u16);
        put_u16(&mut header, 14, second.len() as u16);
        put_u16(&mut header, 18, 2);
        put_u16(&mut header, 20, 2);
        header[22..29].copy_from_slice(b"E1M1\0\0\0");
        header.extend(first);
        header.extend(second);

        let mut maphead = vec![0u8; 2 + 100 * 4];
        put_u16(&mut maphead, 0, rlew_tag);
        put_u32(&mut maphead, 2, 0);
        for map in 1..100 {
            put_u32(&mut maphead, 2 + map * 4, u32::MAX);
        }
        fs::write(dir.join("MAPHEAD.WL6"), maphead).unwrap();
        fs::write(dir.join("GAMEMAPS.WL6"), header).unwrap();

        let map = load(&dir, "WL6", 0).unwrap();
        assert_eq!(map.name, "E1M1");
        assert_eq!(&map.planes[0][..4], [1, 0, 0, 2]);
        assert_eq!(map.player_start(), Some((0, 0, 19)));
        fs::remove_dir_all(dir).unwrap();
    }

    fn packed_plane(tag: u16, tiles: &[u16]) -> Vec<u8> {
        let mut words = vec![(tiles.len() * 2) as u16];
        let mut index = 0;
        while index < tiles.len() {
            let mut count = 1;
            while index + count < tiles.len()
                && tiles[index + count] == tiles[index]
                && count < u16::MAX as usize
            {
                count += 1;
            }
            if count > 3 || tiles[index] == tag {
                words.extend([tag, count as u16, tiles[index]]);
            } else {
                words.extend(std::iter::repeat_n(tiles[index], count));
            }
            index += count;
        }
        let expanded_len = (words.len() * 2) as u16;
        let mut bytes = expanded_len.to_le_bytes().to_vec();
        for word in words {
            bytes.extend(word.to_le_bytes());
        }
        bytes
    }

    fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}
