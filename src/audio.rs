use std::{error::Error, fmt, fs, path::Path};

use crate::resources::find_data_file;

const SOUND_COUNT: usize = 87;
const MUSIC_TRACK_COUNT: usize = 27;
const SOUND_HEADER_BYTES: usize = 6;
const ADLIB_HEADER_BYTES: usize = SOUND_HEADER_BYTES + 16 + 1;
#[cfg(test)]
const DIGITIZED_HEADER_BYTES: usize = 10;
const PC_TICK_RATE: usize = 140;
const MAX_PC_SOUND_SAMPLES: usize = 1_000_000;
const MAX_ADLIB_SOUND_TICS: usize = 14_000;
const MAX_DIGITIZED_SOUND_BYTES: usize = 16_000_000;
pub const OUTPUT_RATE: usize = 22_050;

#[derive(Debug)]
struct AudioError(String);

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for AudioError {}

/// Reads original PC-speaker sounds, mapped digitized effects, and IMF music
/// sequences from the AUDIOHED/AUDIOT archive.
pub struct AudioArchive {
    chunks: Vec<Vec<u8>>,
    priorities: Vec<u16>,
    adlib_sounds: Vec<Option<AdLibSound>>,
    digitized: Vec<Option<Vec<i16>>>,
    music: Vec<Option<Vec<u8>>>,
    wl1_sound_layout: bool,
}

#[derive(Clone, Debug)]
pub struct AdLibSound {
    pub priority: u16,
    pub instrument: [u8; 16],
    pub block: u8,
    pub pitches: Vec<u8>,
}

impl AudioArchive {
    pub fn load(
        directory: &Path,
        extension: &str,
        textures: &crate::vswap::Vswap,
    ) -> Result<Self, Box<dyn Error>> {
        let headers = fs::read(find_data_file(directory, &format!("AUDIOHED.{extension}"))?)?;
        let data = fs::read(find_data_file(directory, &format!("AUDIOT.{extension}"))?)?;
        let wl1_sound_layout = extension.eq_ignore_ascii_case("WL1")
            && headers.len() == (69 * 3 + MUSIC_TRACK_COUNT + 1) * 4;
        let sound_count = if wl1_sound_layout { 69 } else { SOUND_COUNT };
        let digitized_start = sound_count * 2;
        if headers.len() < (digitized_start + sound_count + 1) * 4 || headers.len() % 4 != 0 {
            return Err(AudioError(
                "AUDIOHED does not contain complete PC and digitized sound directories".into(),
            )
            .into());
        }
        let offsets: Vec<usize> = headers
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
            .collect();
        let mut chunks = Vec::with_capacity(sound_count);
        let mut priorities = Vec::with_capacity(sound_count);
        for index in 0..sound_count {
            let start = offsets[index];
            let end = offsets[index + 1];
            if start == u32::MAX as usize || end == u32::MAX as usize {
                chunks.push(Vec::new());
                priorities.push(0);
                continue;
            }
            let chunk = data
                .get(start..end)
                .ok_or_else(|| AudioError(format!("audio chunk {index} has invalid offsets")))?;
            if chunk.len() < SOUND_HEADER_BYTES {
                return Err(AudioError(format!("audio chunk {index} is truncated")).into());
            }
            let sample_count =
                u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize;
            priorities.push(u16::from_le_bytes([chunk[4], chunk[5]]));
            if sample_count > MAX_PC_SOUND_SAMPLES {
                return Err(AudioError(format!(
                    "audio chunk {index} exceeds the supported size limit"
                ))
                .into());
            }
            let sample_end = SOUND_HEADER_BYTES
                .checked_add(sample_count)
                .ok_or_else(|| AudioError("audio sample size overflow".into()))?;
            if sample_end > chunk.len() {
                return Err(
                    AudioError(format!("audio chunk {index} has a truncated sample")).into(),
                );
            }
            chunks.push(chunk[SOUND_HEADER_BYTES..sample_end].to_vec());
        }

        let mut adlib_sounds = Vec::with_capacity(sound_count);
        for index in 0..sound_count {
            let chunk_index = sound_count + index;
            let start = offsets[chunk_index];
            let end = offsets[chunk_index + 1];
            if start == u32::MAX as usize || end == u32::MAX as usize || start == end {
                adlib_sounds.push(None);
                continue;
            }
            let chunk = data.get(start..end).ok_or_else(|| {
                AudioError(format!(
                    "AdLib audio chunk {chunk_index} has invalid offsets"
                ))
            })?;
            if chunk.len() < ADLIB_HEADER_BYTES {
                return Err(
                    AudioError(format!("AdLib audio chunk {chunk_index} is truncated")).into(),
                );
            }
            let pitch_count = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize;
            if pitch_count > MAX_ADLIB_SOUND_TICS {
                return Err(AudioError(format!(
                    "AdLib audio chunk {chunk_index} exceeds the supported sequence size"
                ))
                .into());
            }
            let pitch_end = ADLIB_HEADER_BYTES
                .checked_add(pitch_count)
                .ok_or_else(|| AudioError("AdLib pitch sequence size overflow".into()))?;
            if pitch_end > chunk.len() {
                return Err(AudioError(format!(
                    "AdLib audio chunk {chunk_index} has a truncated pitch sequence"
                ))
                .into());
            }
            let mut instrument = [0u8; 16];
            instrument.copy_from_slice(&chunk[SOUND_HEADER_BYTES..ADLIB_HEADER_BYTES - 1]);
            adlib_sounds.push(Some(AdLibSound {
                priority: u16::from_le_bytes([chunk[4], chunk[5]]),
                instrument,
                block: chunk[ADLIB_HEADER_BYTES - 1],
                pitches: chunk[ADLIB_HEADER_BYTES..pitch_end].to_vec(),
            }));
        }

        let mut digitized = vec![None; sound_count];
        for &(sound, sample) in digitized_sound_map(extension) {
            if let (Some(slot), Some(source)) =
                (digitized.get_mut(sound), textures.digitized_sample(sample))
            {
                let samples: Vec<i16> = source
                    .into_iter()
                    .map(|byte| (i16::from(byte) - 128) << 8)
                    .collect();
                *slot = resample_digitized(&samples, 7_000);
            }
        }
        let music_start = sound_count * 3;
        let available_music_tracks = offsets
            .len()
            .saturating_sub(music_start + 1)
            .min(MUSIC_TRACK_COUNT);
        let mut music = vec![None; MUSIC_TRACK_COUNT];
        for (track, slot) in music.iter_mut().enumerate().take(available_music_tracks) {
            let chunk_index = music_start + track;
            let start = offsets[chunk_index];
            let end = offsets[chunk_index + 1];
            if start == u32::MAX as usize || end == u32::MAX as usize || start == end {
                continue;
            }
            let chunk = data.get(start..end).ok_or_else(|| {
                AudioError(format!("music chunk {chunk_index} has invalid offsets"))
            })?;
            if chunk.len() < 2 {
                return Err(AudioError(format!("music chunk {chunk_index} is truncated")).into());
            }
            let event_bytes = u16::from_le_bytes([chunk[0], chunk[1]]) as usize;
            let event_end = 2usize
                .checked_add(event_bytes)
                .ok_or_else(|| AudioError("music event size overflow".into()))?;
            if event_bytes % 4 != 0 || event_end > chunk.len() {
                return Err(AudioError(format!(
                    "music chunk {chunk_index} has an invalid IMF event stream"
                ))
                .into());
            }
            *slot = Some(chunk[2..event_end].to_vec());
        }
        Ok(Self {
            chunks,
            priorities,
            adlib_sounds,
            digitized,
            music,
            wl1_sound_layout,
        })
    }

    /// Returns a level's original IMF sequence using the DOS campaign song
    /// table. The returned events are `(register, value, delay_le_u16)` tuples.
    pub fn music_for_map(&self, map_number: usize) -> Option<&[u8]> {
        let track = music_track_for_map(map_number, self.wl1_sound_layout)?;
        self.music.get(track)?.as_deref()
    }

    pub fn music_player_for_map(&self, map_number: usize) -> Option<crate::opl2::ImfPlayer> {
        crate::opl2::ImfPlayer::new(self.music_for_map(map_number)?, OUTPUT_RATE)
    }

    pub fn music_player_for_track(&self, track: usize) -> Option<crate::opl2::ImfPlayer> {
        crate::opl2::ImfPlayer::new(self.music.get(track)?.as_deref()?, OUTPUT_RATE)
    }

    pub fn has_music_track(&self, track: usize) -> bool {
        self.music.get(track).is_some_and(Option::is_some)
    }

    pub fn priority(&self, sound: usize) -> Option<u16> {
        self.priorities
            .get(self.source_sound_index(sound)?)
            .copied()
    }

    pub fn adlib_sound(&self, sound: usize) -> Option<&AdLibSound> {
        self.adlib_sounds
            .get(self.source_sound_index(sound)?)?
            .as_ref()
    }

    pub fn adlib_priority(&self, sound: usize) -> Option<u16> {
        Some(self.adlib_sound(sound)?.priority)
    }

    pub fn has_sound(&self, sound: usize) -> bool {
        self.source_sound_index(sound).is_some()
    }

    /// Converts a DOS PIT frequency stream to queued SDL signed 16-bit PCM.
    /// Each source byte is one 140 Hz speaker update; zero denotes silence.
    pub fn pc_sound_pcm(&self, sound: usize) -> Option<Vec<i16>> {
        let source = self.chunks.get(self.source_sound_index(sound)?)?;
        if source.is_empty() {
            return None;
        }
        let estimated_frames = source.len().saturating_mul(OUTPUT_RATE / PC_TICK_RATE);
        let mut pcm = Vec::with_capacity(estimated_frames);
        let mut phase = 0.0f64;
        let mut previous_tone = None;
        let mut frame_remainder = 0usize;
        for &tone in source {
            // SDL_PCService reloads PIT channel 2 only when the byte changes.
            // Mode 3 begins a new high half-cycle when the counter is loaded.
            if previous_tone != Some(tone) {
                phase = 0.0;
                previous_tone = Some(tone);
            }
            let frequency = if tone == 0 {
                0.0
            } else {
                1_193_180.0 / (tone as f64 * 60.0)
            };
            frame_remainder += OUTPUT_RATE;
            let frames = frame_remainder / PC_TICK_RATE;
            frame_remainder %= PC_TICK_RATE;
            for _ in 0..frames {
                let sample = if frequency == 0.0 {
                    0
                } else {
                    if phase >= 1.0 {
                        phase -= phase.floor();
                    }
                    let value = if phase < 0.5 { 4_000 } else { -4_000 };
                    phase += frequency / OUTPUT_RATE as f64;
                    value
                };
                pcm.push(sample);
            }
        }
        Some(pcm)
    }

    pub fn digitized_sound_pcm(&self, sound: usize) -> Option<&[i16]> {
        let sound = self.wl1_sound_alias(sound);
        self.digitized.get(sound)?.as_deref()
    }

    fn source_sound_index(&self, sound: usize) -> Option<usize> {
        let sound = self.wl1_sound_alias(sound);
        if self.wl1_sound_layout && sound == 50 {
            return Some(57);
        }
        if self.wl1_sound_layout && sound == 57 {
            return None;
        }
        (sound < self.chunks.len()).then_some(sound)
    }

    fn wl1_sound_alias(&self, sound: usize) -> usize {
        if !self.wl1_sound_layout {
            return sound;
        }
        match sound {
            73 | 76 | 78 => 29,
            74 | 77 => 22,
            _ => sound,
        }
    }
}

fn music_track_for_map(map_number: usize, wl1: bool) -> Option<usize> {
    const SONGS: [usize; 60] = [
        3, 11, 9, 12, 3, 11, 9, 12, 2, 0, 8, 18, 17, 4, 8, 18, 4, 17, 2, 1, 6, 20, 22, 21, 6, 20,
        22, 21, 19, 26, 3, 11, 9, 12, 3, 11, 9, 12, 2, 0, 8, 18, 17, 4, 8, 18, 4, 17, 2, 1, 6, 20,
        22, 21, 6, 20, 22, 21, 19, 15,
    ];
    let track = *SONGS.get(map_number)?;
    // WL1's audio enum replaces the WL6 boss theme at index 2 with
    // WARMRCH1_MUS at index 26. The campaign table itself is shared.
    Some(if wl1 && track == 2 { 26 } else { track })
}

#[cfg(test)]
fn decode_digitized_chunk(chunk: &[u8]) -> Result<Option<Vec<i16>>, Box<dyn Error>> {
    if chunk.len() < DIGITIZED_HEADER_BYTES {
        return Err(AudioError("digitized audio chunk is truncated".into()).into());
    }
    let byte_count = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize;
    let rate = u16::from_le_bytes([chunk[6], chunk[7]]) as usize;
    let bits = chunk[8];
    if byte_count > MAX_DIGITIZED_SOUND_BYTES {
        return Err(AudioError("digitized audio chunk exceeds the size limit".into()).into());
    }
    let end = DIGITIZED_HEADER_BYTES
        .checked_add(byte_count)
        .ok_or_else(|| AudioError("digitized audio size overflow".into()))?;
    let source = chunk
        .get(DIGITIZED_HEADER_BYTES..end)
        .ok_or_else(|| AudioError("digitized audio sample is truncated".into()))?;
    if rate == 0 {
        return Ok(None);
    }
    if bits == 16 && byte_count % 2 != 0 {
        return Err(AudioError("16-bit digitized sample has an odd byte count".into()).into());
    }
    let samples: Vec<i16> = match bits {
        8 => source
            .iter()
            .map(|sample| ((*sample as i16) - 128) << 8)
            .collect(),
        16 => source
            .chunks_exact(2)
            .map(|sample| i16::from_le_bytes([sample[0], sample[1]]))
            .collect(),
        _ => return Err(AudioError(format!("unsupported digitized sample depth: {bits}")).into()),
    };
    Ok(resample_digitized(&samples, rate))
}

fn resample_digitized(samples: &[i16], rate: usize) -> Option<Vec<i16>> {
    if samples.is_empty() {
        return None;
    }
    let output_count = samples
        .len()
        .saturating_mul(OUTPUT_RATE)
        .div_ceil(rate)
        .min(MAX_DIGITIZED_SOUND_BYTES);
    let mut pcm = Vec::with_capacity(output_count);
    for frame in 0..output_count {
        let source_position = frame as f64 * rate as f64 / OUTPUT_RATE as f64;
        let first = (source_position.floor() as usize).min(samples.len() - 1);
        let second = (first + 1).min(samples.len() - 1);
        let fraction = source_position - first as f64;
        let value = samples[first] as f64 * (1.0 - fraction) + samples[second] as f64 * fraction;
        pcm.push(value.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
    }
    Some(pcm)
}

fn digitized_sound_map(extension: &str) -> &'static [(usize, usize)] {
    const WL1: &[(usize, usize)] = &[
        (21, 0),
        (41, 1),
        (19, 2),
        (18, 3),
        (26, 4),
        (24, 5),
        (11, 6),
        (51, 7),
        (55, 8),
        (50, 9),
        (59, 10),
        (60, 11),
        (29, 12),
        (22, 13),
        (25, 13),
        (16, 14),
        (46, 15),
        (56, 20),
        (58, 21),
        (61, 22),
    ];
    const WL6: &[(usize, usize)] = &[
        (21, 0),
        (41, 1),
        (19, 2),
        (18, 3),
        (26, 4),
        (24, 5),
        (11, 6),
        (51, 7),
        (55, 8),
        (50, 9),
        (59, 10),
        (60, 11),
        (29, 12),
        (22, 13),
        (25, 13),
        (16, 14),
        (46, 15),
        (56, 20),
        (58, 21),
        (61, 22),
        (72, 32),
        (10, 16),
        (52, 17),
        (53, 18),
        (54, 19),
        (62, 23),
        (63, 24),
        (64, 25),
        (65, 26),
        (66, 27),
        (67, 28),
        (68, 29),
        (40, 30),
        (70, 31),
        (57, 33),
        (73, 34),
        (74, 35),
        (79, 36),
        (80, 37),
        (81, 38),
        (75, 39),
        (76, 40),
        (77, 41),
        (78, 42),
        (82, 43),
        (83, 44),
        (84, 45),
    ];
    if extension.eq_ignore_ascii_case("WL1") {
        WL1
    } else {
        WL6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_original_unsigned_eight_bit_samples_to_sdl_pcm() {
        let mut chunk = Vec::from(4u32.to_le_bytes());
        chunk.extend_from_slice(&10u16.to_le_bytes());
        chunk.extend_from_slice(&(OUTPUT_RATE as u16).to_le_bytes());
        chunk.extend_from_slice(&[8, 128, 0, 128, 255, 128]);
        let decoded = decode_digitized_chunk(&chunk).unwrap().unwrap();
        assert_eq!(decoded.as_slice(), &[-32768i16, 0, 32512, 0]);
    }

    #[test]
    fn wl1_and_wl6_use_their_original_muttisnd_indexes() {
        assert!(digitized_sound_map("WL1").contains(&(50, 9)));
        assert!(digitized_sound_map("WL6").contains(&(50, 9)));
        assert!(!digitized_sound_map("WL1").contains(&(57, 33)));
    }

    #[test]
    fn wl1_maps_mutti_to_the_shareware_chunk() {
        let archive = AudioArchive {
            chunks: vec![Vec::new(); 69],
            priorities: (0..69).map(|index| index as u16).collect(),
            adlib_sounds: vec![None; 69],
            digitized: vec![None; 69],
            music: Vec::new(),
            wl1_sound_layout: true,
        };
        assert_eq!(archive.source_sound_index(50), Some(57));
        assert_eq!(archive.priority(50), Some(57));
        assert_eq!(archive.source_sound_index(57), None);
    }

    #[test]
    fn wl1_maps_unavailable_guard_screams_to_available_screams() {
        let archive = AudioArchive {
            chunks: vec![Vec::new(); 69],
            priorities: (0..69).map(|index| index as u16).collect(),
            adlib_sounds: vec![None; 69],
            digitized: vec![None; 69],
            music: Vec::new(),
            wl1_sound_layout: true,
        };
        assert_eq!(archive.source_sound_index(73), Some(29));
        assert_eq!(archive.source_sound_index(74), Some(22));
        assert_eq!(archive.source_sound_index(76), Some(29));
        assert_eq!(archive.source_sound_index(77), Some(22));
    }
}
