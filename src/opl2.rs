//! Software approximation for the two-operator OPL2 register writes in Wolf3D
//! IMF streams. It models nine melodic channels and YM3812 waveform curves, but
//! advances its operators and shared envelope timer at the YM3812's 49,716 Hz
//! sample clock, then averages chip samples into the SDL output rate.

use std::sync::OnceLock;

const CHANNEL_OPERATOR_OFFSETS: [u8; 9] = [0, 1, 2, 8, 9, 10, 16, 17, 18];
// The YM3812 phase input uses signed operator output units on a 10-bit scale.
const OPERATOR_OUTPUT_PEAK: f64 = 4084.0;
// Hardware total-level/envelope attenuation is quantized at this dB step;
// the YM3812 exponential ROM uses eight finer steps per hardware unit.
const OPERATOR_ATTENUATION_DB_PER_UNIT: f64 = 0.1875;
const MULTIPLIERS: [f64; 16] = [
    0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 10.0, 12.0, 12.0, 15.0, 15.0,
];
const CHIP_SAMPLE_RATE: f64 = 49_716.0;
const EG_INCSTEP: [[u8; 4]; 4] = [[0, 0, 0, 0], [1, 0, 0, 0], [1, 0, 1, 0], [1, 1, 1, 0]];
const EG_TIMER_MAX: u64 = 0x0f_ffff_ffff;
const KSL_3DB_TABLE: [[f64; 16]; 8] = [
    [0.0; 16],
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.75, 1.125, 1.5, 1.875, 2.25, 2.625, 3.0,
    ],
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 1.125, 1.875, 2.625, 3.0, 3.75, 4.125, 4.5, 4.875, 5.25, 5.625,
        6.0,
    ],
    [
        0.0, 0.0, 0.0, 1.875, 3.0, 4.125, 4.875, 5.625, 6.0, 6.75, 7.125, 7.5, 7.875, 8.25, 8.625,
        9.0,
    ],
    [
        0.0, 0.0, 3.0, 4.875, 6.0, 7.125, 7.875, 8.625, 9.0, 9.75, 10.125, 10.5, 10.875, 11.25,
        11.625, 12.0,
    ],
    [
        0.0, 3.0, 6.0, 7.875, 9.0, 10.125, 10.875, 11.625, 12.0, 12.75, 13.125, 13.5, 13.875,
        14.25, 14.625, 15.0,
    ],
    [
        0.0, 6.0, 9.0, 10.875, 12.0, 13.125, 13.875, 14.625, 15.0, 15.75, 16.125, 16.5, 16.875,
        17.25, 17.625, 18.0,
    ],
    [
        0.0, 9.0, 12.0, 13.875, 15.0, 16.125, 16.875, 17.625, 18.0, 18.75, 19.125, 19.5, 19.875,
        20.25, 20.625, 21.0,
    ],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EnvelopeStage {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy, Debug)]
struct Operator {
    phase: f64,
    attenuation: u16,
    stage: EnvelopeStage,
    multiplier: u8,
    total_level: u8,
    key_scale_level: u8,
    attack: u8,
    decay: u8,
    sustain: u8,
    release: u8,
    sustain_type: bool,
    key_scale_rate: bool,
    amplitude_modulation: bool,
    vibrato: bool,
    waveform: u8,
}

impl Default for Operator {
    fn default() -> Self {
        Self {
            phase: 0.0,
            attenuation: 0x1ff,
            stage: EnvelopeStage::Off,
            multiplier: 0,
            total_level: 0,
            key_scale_level: 0,
            attack: 0,
            decay: 0,
            sustain: 0,
            release: 0,
            sustain_type: false,
            key_scale_rate: false,
            amplitude_modulation: false,
            vibrato: false,
            waveform: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Channel {
    modulator: Operator,
    carrier: Operator,
    f_number: u16,
    block: u8,
    key_on: bool,
    operator_keys: [bool; 2],
    additive: bool,
    feedback: u8,
    modulator_history: [f64; 2],
}

struct AmplitudeTables {
    log_sine: [u16; 256],
    exponential: [u16; 256],
}

static AMPLITUDE_TABLES: OnceLock<AmplitudeTables> = OnceLock::new();

fn amplitude_tables() -> &'static AmplitudeTables {
    AMPLITUDE_TABLES.get_or_init(|| AmplitudeTables {
        log_sine: std::array::from_fn(|index| {
            let angle = std::f64::consts::PI * (index as f64 + 0.5) / 512.0;
            (-angle.sin().log2() * 256.0).round() as u16
        }),
        exponential: std::array::from_fn(|index| {
            (2042.0 * 2.0f64.powf(-(index as f64) / 256.0)).round() as u16
        }),
    })
}

/// Generates mono OPL2 samples from register writes.
pub struct Opl2Synth {
    channels: [Channel; 9],
    sample_rate: f64,
    waveform_select_enabled: bool,
    note_select: bool,
    am_depth: bool,
    vibrato_depth: bool,
    am_phase: f64,
    vibrato_phase: f64,
    rhythm: u8,
    noise_lfsr: u32,
    output_clock_remainder: f64,
    last_chip_sample: i16,
    eg_state: bool,
    eg_timer: u64,
    eg_timer_rem: bool,
    eg_timer_lo: u8,
    eg_add: u8,
    rhythm_tc_bits: u8,
}

impl Opl2Synth {
    pub fn new(sample_rate: usize) -> Self {
        Self {
            channels: [Channel::default(); 9],
            sample_rate: sample_rate.max(1) as f64,
            waveform_select_enabled: false,
            note_select: false,
            am_depth: false,
            vibrato_depth: false,
            am_phase: 0.0,
            vibrato_phase: 0.0,
            rhythm: 0,
            noise_lfsr: 1,
            output_clock_remainder: 0.0,
            last_chip_sample: 0,
            eg_state: false,
            eg_timer: 0,
            eg_timer_rem: false,
            eg_timer_lo: 0,
            eg_add: 0,
            rhythm_tc_bits: 0,
        }
    }

    pub fn write_register(&mut self, register: u8, value: u8) {
        if register == 0x01 {
            self.waveform_select_enabled = value & 0x20 != 0;
            return;
        }
        if register == 0x08 {
            self.note_select = value & 0x40 != 0;
            return;
        }
        if register == 0xbd {
            self.am_depth = value & 0x80 != 0;
            self.vibrato_depth = value & 0x40 != 0;
            self.rhythm = value & 0x3f;
            for index in 6..9 {
                self.update_channel_keys(index);
            }
            return;
        }
        if (0xa0..=0xa8).contains(&register) {
            let channel = &mut self.channels[(register - 0xa0) as usize];
            channel.f_number = (channel.f_number & 0x300) | value as u16;
            return;
        }
        if (0xb0..=0xb8).contains(&register) {
            let index = (register - 0xb0) as usize;
            let channel = &mut self.channels[index];
            channel.f_number = (channel.f_number & 0xff) | (((value & 0x03) as u16) << 8);
            channel.block = (value >> 2) & 0x07;
            channel.key_on = value & 0x20 != 0;
            self.update_channel_keys(index);
            return;
        }
        if (0xc0..=0xc8).contains(&register) {
            let channel = &mut self.channels[(register - 0xc0) as usize];
            channel.feedback = (value >> 1) & 0x07;
            channel.additive = value & 0x01 != 0;
            return;
        }
        let Some((channel_index, carrier)) = operator_register(register) else {
            return;
        };
        let channel = &mut self.channels[channel_index];
        let operator = if carrier {
            &mut channel.carrier
        } else {
            &mut channel.modulator
        };
        match register & 0xe0 {
            0x20 => {
                operator.multiplier = value & 0x0f;
                operator.sustain_type = value & 0x20 != 0;
                operator.key_scale_rate = value & 0x10 != 0;
                operator.vibrato = value & 0x40 != 0;
                operator.amplitude_modulation = value & 0x80 != 0;
            }
            0x40 => {
                operator.total_level = value & 0x3f;
                operator.key_scale_level = value >> 6;
            }
            0x60 => {
                operator.attack = value >> 4;
                operator.decay = value & 0x0f;
            }
            0x80 => {
                operator.sustain = value >> 4;
                operator.release = value & 0x0f;
            }
            0xe0 => operator.waveform = value & 0x03,
            _ => {}
        }
    }

    fn update_channel_keys(&mut self, index: usize) {
        let channel = &mut self.channels[index];
        let mut keys = [channel.key_on; 2];
        if self.rhythm & 0x20 != 0 && index >= 6 {
            // Bass drum, hi-hat/snare, and tom/cymbal respectively.
            let masks = [[0x10, 0x10], [0x01, 0x08], [0x04, 0x02]][index - 6];
            for (key, mask) in keys.iter_mut().zip(masks) {
                *key |= self.rhythm & mask != 0;
            }
        }
        for (index, operator) in [&mut channel.modulator, &mut channel.carrier]
            .into_iter()
            .enumerate()
        {
            match (channel.operator_keys[index], keys[index]) {
                (false, true) => {
                    operator.stage = EnvelopeStage::Attack;
                    operator.phase = 0.0;
                }
                (true, false) if operator.stage != EnvelopeStage::Off => {
                    operator.stage = EnvelopeStage::Release;
                }
                _ => {}
            }
        }
        channel.operator_keys = keys;
    }

    pub(crate) fn channel_is_off(&self, channel: usize) -> bool {
        self.channels.get(channel).is_some_and(|channel| {
            channel.modulator.stage == EnvelopeStage::Off
                && channel.carrier.stage == EnvelopeStage::Off
        })
    }

    pub fn next_sample(&mut self) -> i16 {
        self.next_sample_masked(0x01ff)
    }

    /// Advances every operator while mixing only the requested channels.
    /// This lets the SDL layer keep its independent music and effects mutes
    /// when an AdLib effect shares the music chip state.
    pub fn next_sample_masked(&mut self, channel_mask: u16) -> i16 {
        self.output_clock_remainder += CHIP_SAMPLE_RATE / self.sample_rate;
        let chip_steps = self.output_clock_remainder.floor() as usize;
        self.output_clock_remainder -= chip_steps as f64;
        if chip_steps == 0 {
            return self.last_chip_sample;
        }

        let mut sample_sum = 0i64;
        for _ in 0..chip_steps {
            self.last_chip_sample = self.next_chip_sample_masked(channel_mask);
            sample_sum += i64::from(self.last_chip_sample);
        }
        (sample_sum / chip_steps as i64) as i16
    }

    fn next_chip_sample_masked(&mut self, channel_mask: u16) -> i16 {
        let mut mixed = 0.0;
        let rhythm_enabled = self.rhythm & 0x20 != 0;
        // OPL3_PhaseGenerate consumes noise bit 0 before advancing the LFSR.
        let noise_bit = (self.noise_lfsr & 1) as u16;
        let (phase_select, noise_state): (u16, u16) = if rhythm_enabled {
            let hi_hat_phase = phase_steps(self.channels[7].modulator.phase);
            let hh_bit2 = (hi_hat_phase >> 2) & 1;
            let hh_bit3 = (hi_hat_phase >> 3) & 1;
            let hh_bit7 = (hi_hat_phase >> 7) & 1;
            // Nuked processes the hi-hat slot before the cymbal slot, so the
            // cymbal phase bits here are the previous chip sample's values.
            let tc_bit3 = u16::from(self.rhythm_tc_bits & 1);
            let tc_bit5 = u16::from((self.rhythm_tc_bits >> 1) & 1);
            let select = ((hh_bit2 ^ hh_bit7) | (hh_bit3 ^ tc_bit5) | (tc_bit3 ^ tc_bit5)) & 1;
            (select, noise_bit)
        } else {
            (0, 0)
        };
        let mut next_rhythm_tc_bits = self.rhythm_tc_bits;
        for (index, channel) in self.channels.iter_mut().enumerate() {
            let independent_drums = rhythm_enabled && index >= 7;
            let key_scale_number =
                key_scaling_number(channel.block, channel.f_number, self.note_select);
            let modulator_ksl = key_scale_attenuation_db(
                channel.block,
                channel.f_number,
                channel.modulator.key_scale_level,
            );
            let carrier_ksl = key_scale_attenuation_db(
                channel.block,
                channel.f_number,
                channel.carrier.key_scale_level,
            );
            let frequency =
                channel.f_number as f64 * CHIP_SAMPLE_RATE / 2.0f64.powi(20 - channel.block as i32);
            let modulator_frequency = vibrato_frequency(
                frequency * MULTIPLIERS[channel.modulator.multiplier as usize],
                channel.modulator.vibrato,
                self.vibrato_phase,
                self.vibrato_depth,
                channel.f_number,
            );
            let modulator_am = amplitude_modulation_attenuation_db(
                channel.modulator.amplitude_modulation,
                self.am_phase,
                self.am_depth,
            );
            let [older_modulator, previous_modulator] = channel.modulator_history;
            let feedback_phase = if channel.feedback == 0 || independent_drums {
                0
            } else {
                let previous_units = (previous_modulator * OPERATOR_OUTPUT_PEAK).round() as i32;
                let older_units = (older_modulator * OPERATOR_OUTPUT_PEAK).round() as i32;
                (older_units + previous_units) >> (9 - i32::from(channel.feedback))
            };
            let hi_hat_phase = phase_steps(channel.modulator.phase);
            let modulator_phase = if rhythm_enabled && index == 7 {
                Some((phase_select << 9) | (0xd0 >> (2 * (noise_state ^ phase_select))))
            } else {
                None
            };
            let modulator = operator_sample_with_phase(
                &mut channel.modulator,
                modulator_frequency,
                CHIP_SAMPLE_RATE,
                key_scale_number,
                modulator_ksl,
                modulator_am,
                feedback_phase,
                self.waveform_select_enabled,
                modulator_phase,
            );
            channel.modulator_history = [previous_modulator, modulator];
            let carrier_frequency = vibrato_frequency(
                frequency * MULTIPLIERS[channel.carrier.multiplier as usize],
                channel.carrier.vibrato,
                self.vibrato_phase,
                self.vibrato_depth,
                channel.f_number,
            );
            let carrier_am = amplitude_modulation_attenuation_db(
                channel.carrier.amplitude_modulation,
                self.am_phase,
                self.am_depth,
            );
            let cymbal_phase = (index == 8).then(|| phase_steps(channel.carrier.phase));
            let phase_modulation = if channel.additive || independent_drums {
                0
            } else {
                (previous_modulator * OPERATOR_OUTPUT_PEAK).round() as i32
            };
            let carrier = if rhythm_enabled && index == 7 {
                let phase = (0x100 << ((hi_hat_phase >> 8) & 1)) ^ (noise_state << 8);
                operator_sample_with_phase(
                    &mut channel.carrier,
                    carrier_frequency,
                    CHIP_SAMPLE_RATE,
                    key_scale_number,
                    carrier_ksl,
                    carrier_am,
                    phase_modulation,
                    self.waveform_select_enabled,
                    Some(phase),
                )
            } else if rhythm_enabled && index == 8 {
                let phase = 0x100 | (phase_select << 9);
                operator_sample_with_phase(
                    &mut channel.carrier,
                    carrier_frequency,
                    CHIP_SAMPLE_RATE,
                    key_scale_number,
                    carrier_ksl,
                    carrier_am,
                    phase_modulation,
                    self.waveform_select_enabled,
                    Some(phase),
                )
            } else {
                operator_sample(
                    &mut channel.carrier,
                    carrier_frequency,
                    CHIP_SAMPLE_RATE,
                    key_scale_number,
                    carrier_ksl,
                    carrier_am,
                    phase_modulation,
                    self.waveform_select_enabled,
                )
            };
            if let Some(phase) = cymbal_phase {
                next_rhythm_tc_bits = (((phase >> 3) & 1) | (((phase >> 5) & 1) << 1)) as u8;
            }
            let output = if independent_drums {
                (carrier + modulator) * 2.0
            } else if rhythm_enabled && index == 6 {
                carrier * 2.0
            } else {
                carrier
                    + if channel.additive {
                        previous_modulator
                    } else {
                        0.0
                    }
            };
            if channel_mask & (1 << index) != 0 {
                mixed += output;
            }
        }
        self.rhythm_tc_bits = next_rhythm_tc_bits;
        // OPL3_EnvelopeCalc computes eg_out from the current route before
        // updating the route for the next chip sample. Mix this chip sample
        // first, then advance the shared EG clock.
        for channel in &mut self.channels {
            let key_scale_number =
                key_scaling_number(channel.block, channel.f_number, self.note_select);
            advance_operator_envelope(
                &mut channel.modulator,
                key_scale_number,
                self.eg_state,
                self.eg_add,
                self.eg_timer_lo,
            );
            advance_operator_envelope(
                &mut channel.carrier,
                key_scale_number,
                self.eg_state,
                self.eg_add,
                self.eg_timer_lo,
            );
        }
        if self.eg_state {
            let shift = self.eg_timer.trailing_zeros().min(13) as u8;
            self.eg_add = if shift > 12 { 0 } else { shift + 1 };
            self.eg_timer_lo = (self.eg_timer & 0x03) as u8;
        }
        if self.eg_timer_rem || self.eg_state {
            if self.eg_timer == EG_TIMER_MAX {
                self.eg_timer = 0;
                self.eg_timer_rem = true;
            } else {
                self.eg_timer += 1;
                self.eg_timer_rem = false;
            }
        }
        self.eg_state = !self.eg_state;
        self.am_phase = (self.am_phase + 1.0 / (210.0 * 64.0)).rem_euclid(1.0);
        self.vibrato_phase = (self.vibrato_phase + 1.0 / 8_192.0).rem_euclid(1.0);
        let noise = self.noise_lfsr & 0x007f_ffff;
        let feedback = ((noise >> 14) ^ noise) & 1;
        self.noise_lfsr = (noise >> 1) | (feedback << 22);
        (mixed * (12_000.0 / 3.0)).clamp(i16::MIN as f64, i16::MAX as f64) as i16
    }
}

/// Replays a looping Wolf3D IMF sequence through an OPL2 register model.
pub struct ImfPlayer {
    events: Vec<(u8, u8, u16)>,
    channel_mask: u16,
    cursor: usize,
    samples_until_event: f64,
    samples_per_tick: f64,
}

/// Replays the register instrument and 140 Hz pitch sequence from an AdLib
/// sound-effect chunk on OPL2 channel zero.
pub struct AdLibEffectPlayer {
    instrument: [u8; 16],
    pitches: Vec<u8>,
    cursor: usize,
    samples_until_pitch: f64,
    samples_per_pitch: f64,
    block: u8,
    keyed_off: bool,
    release_samples: usize,
    release_sample_limit: usize,
    started: bool,
}

impl AdLibEffectPlayer {
    pub fn new(
        instrument: [u8; 16],
        block: u8,
        pitches: &[u8],
        sample_rate: usize,
    ) -> Option<Self> {
        if pitches.is_empty() || (instrument[6] | instrument[7]) == 0 {
            return None;
        }
        let samples_per_pitch = sample_rate.max(1) as f64 / 140.0;
        Some(Self {
            instrument,
            pitches: pitches.to_vec(),
            cursor: 0,
            // DOS installs the sound first, then consumes its first pitch on
            // the next 140 Hz sound-service tick.
            samples_until_pitch: samples_per_pitch,
            samples_per_pitch,
            block: (block & 0x07) << 2,
            keyed_off: false,
            release_samples: 0,
            release_sample_limit: sample_rate.max(1).saturating_mul(2),
            started: false,
        })
    }

    /// DOS reports the effect as finished as soon as its final pitch byte is
    /// consumed, even though the OPL channel can still have an audible release
    /// tail.
    pub fn is_playing(&self) -> bool {
        self.cursor < self.pitches.len()
    }

    /// Applies one due 140 Hz effect-service update to the shared OPL state.
    /// Returns false when the sound's release tail has finished.
    pub fn service(&mut self, synth: &mut Opl2Synth) -> bool {
        if !self.started {
            // SDL_DetectAdLib enables waveform selection before playback.
            synth.write_register(0x01, 0x20);
            // DOS stops the previous channel-zero effect before installing
            // the next instrument, which also restarts an active envelope.
            synth.write_register(0xb0, 0);
            for (register, value) in [
                (0x20, self.instrument[0]),
                (0x23, self.instrument[1]),
                (0x40, self.instrument[2]),
                (0x43, self.instrument[3]),
                (0x60, self.instrument[4]),
                (0x63, self.instrument[5]),
                (0x80, self.instrument[6]),
                (0x83, self.instrument[7]),
                (0xe0, self.instrument[8]),
                (0xe3, self.instrument[9]),
                // The DOS sound driver forces the effect voice into FM mode.
                (0xc0, 0),
            ] {
                synth.write_register(register, value);
            }
            self.started = true;
        }
        if self.samples_until_pitch <= 0.0 {
            if let Some(&pitch) = self.pitches.get(self.cursor) {
                self.cursor += 1;
                if pitch != 0 {
                    synth.write_register(0xa0, pitch);
                    synth.write_register(0xb0, self.block | 0x20);
                }
                // The DOS service clears the entire high register on a
                // rest and immediately after consuming the final byte.
                if pitch == 0 || self.cursor == self.pitches.len() {
                    synth.write_register(0xb0, 0);
                }
                self.keyed_off = self.cursor == self.pitches.len();
                self.samples_until_pitch += self.samples_per_pitch;
            } else if synth.channel_is_off(0) || self.release_samples >= self.release_sample_limit {
                return false;
            }
        }
        self.samples_until_pitch -= 1.0;
        if self.keyed_off {
            self.release_samples = self.release_samples.saturating_add(1);
        }
        true
    }
}

impl ImfPlayer {
    pub fn new(events: &[u8], sample_rate: usize) -> Option<Self> {
        if events.is_empty() || events.len() % 4 != 0 {
            return None;
        }
        let events = events
            .chunks_exact(4)
            .map(|event| (event[0], event[1], u16::from_le_bytes([event[2], event[3]])))
            .collect::<Vec<_>>();
        let channel_mask = events.iter().fold(0u16, |mask, (register, _, _)| {
            if (0xb0..=0xb8).contains(register) {
                mask | (1u16 << usize::from(*register - 0xb0))
            } else {
                mask
            }
        });
        Some(Self {
            events,
            channel_mask,
            cursor: 0,
            samples_until_event: 0.0,
            samples_per_tick: sample_rate.max(1) as f64 / 700.0,
        })
    }

    pub fn channel_mask(&self) -> u16 {
        self.channel_mask
    }

    pub fn service(&mut self, synth: &mut Opl2Synth) {
        let mut immediate_events = 0;
        while self.samples_until_event <= 0.0 && immediate_events < self.events.len() {
            let (register, value, delay) = self.events[self.cursor];
            synth.write_register(register, value);
            self.cursor += 1;
            let end_of_track = self.cursor == self.events.len();
            if end_of_track {
                self.cursor = 0;
                // DOS ends this service pass when it consumes the last
                // event, resets the sequence clock, and emits event 0 on
                // the next 700 Hz service tick.
                self.samples_until_event += self.samples_per_tick;
            } else {
                self.samples_until_event += delay as f64 * self.samples_per_tick;
            }
            immediate_events += 1;
        }
        self.samples_until_event -= 1.0;
    }
}

fn operator_register(register: u8) -> Option<(usize, bool)> {
    let group = register & 0xe0;
    if !matches!(group, 0x20 | 0x40 | 0x60 | 0x80 | 0xe0) {
        return None;
    }
    let offset = register & 0x1f;
    CHANNEL_OPERATOR_OFFSETS
        .iter()
        .enumerate()
        .find_map(|(channel, &base)| {
            if offset == base {
                Some((channel, false))
            } else if offset == base + 3 {
                Some((channel, true))
            } else {
                None
            }
        })
}

fn operator_sample(
    operator: &mut Operator,
    frequency: f64,
    sample_rate: f64,
    _key_scale_number: u8,
    key_scale_attenuation_db: f64,
    amplitude_modulation_attenuation_db: f64,
    phase_modulation: i32,
    waveform_select_enabled: bool,
) -> f64 {
    operator_sample_with_phase(
        operator,
        frequency,
        sample_rate,
        _key_scale_number,
        key_scale_attenuation_db,
        amplitude_modulation_attenuation_db,
        phase_modulation,
        waveform_select_enabled,
        None,
    )
}

fn operator_sample_with_phase(
    operator: &mut Operator,
    frequency: f64,
    sample_rate: f64,
    _key_scale_number: u8,
    key_scale_attenuation_db: f64,
    amplitude_modulation_attenuation_db: f64,
    phase_modulation: i32,
    waveform_select_enabled: bool,
    fixed_phase: Option<u16>,
) -> f64 {
    // OPL3_PhaseGenerate outputs the old accumulator, then advances it. Keep
    // this order so key-on emits phase zero and every sample has the right step.
    let accumulator_phase = operator.phase;
    operator.phase = (operator.phase + frequency / sample_rate).rem_euclid(1.0);
    if operator.stage == EnvelopeStage::Off {
        return 0.0;
    }
    let phase = fixed_phase.map_or_else(
        || (i32::from(phase_steps(accumulator_phase)) + phase_modulation).rem_euclid(1024) as u16,
        |phase| phase & 0x03ff,
    );
    let waveform = if waveform_select_enabled {
        operator.waveform & 0x03
    } else {
        0
    };
    let attenuation_db = operator.total_level as f64 * 0.75
        + key_scale_attenuation_db
        + amplitude_modulation_attenuation_db;
    // exprom indexes 1/256-octave steps (about 0.0234 dB). Hardware EG/TL
    // units are 0.1875 dB, so each one contributes eight exprom units.
    let attenuation_units = ((attenuation_db / OPERATOR_ATTENUATION_DB_PER_UNIT).round() as u16)
        .saturating_add(operator.attenuation)
        .saturating_mul(8)
        .min(8191);
    ym3812_wave_sample(waveform, phase, attenuation_units)
}

fn advance_operator_envelope(
    operator: &mut Operator,
    key_scale_number: u8,
    eg_state: bool,
    eg_add: u8,
    eg_timer_lo: u8,
) {
    if operator.attenuation & 0x1f8 == 0x1f8 && operator.stage != EnvelopeStage::Attack {
        operator.attenuation = 0x1ff;
        operator.stage = EnvelopeStage::Off;
        return;
    }
    let rate = match operator.stage {
        EnvelopeStage::Off => return,
        EnvelopeStage::Attack => operator.attack,
        EnvelopeStage::Decay => operator.decay,
        EnvelopeStage::Sustain if !operator.sustain_type => operator.release,
        EnvelopeStage::Sustain => 0,
        EnvelopeStage::Release => operator.release,
    };
    let Some(rate_index) = envelope_rate_index(rate, operator.key_scale_rate, key_scale_number)
    else {
        return;
    };
    let rate_hi = (rate_index >> 2) as u8;
    let rate_lo = (rate_index & 0x03) as u8;
    let effective_rate_hi = rate_hi.min(15);
    let mut shift = 0u8;
    if effective_rate_hi < 12 {
        if eg_state {
            shift = match effective_rate_hi + eg_add {
                12 => 1,
                13 => (rate_lo >> 1) & 1,
                14 => rate_lo & 1,
                _ => 0,
            };
        }
    } else {
        shift = (effective_rate_hi & 0x03) + EG_INCSTEP[rate_lo as usize][eg_timer_lo as usize];
        if shift & 0x04 != 0 {
            shift = 3;
        }
        if shift == 0 {
            shift = u8::from(eg_state);
        }
    }

    match operator.stage {
        EnvelopeStage::Off => {}
        EnvelopeStage::Attack => {
            if operator.attenuation == 0 {
                operator.stage = EnvelopeStage::Decay;
            } else if shift > 0 && effective_rate_hi != 15 {
                let inverse_attenuation = !i32::from(operator.attenuation);
                let increment = inverse_attenuation >> (4 - shift);
                operator.attenuation =
                    (i32::from(operator.attenuation) + increment).clamp(0, 0x1ff) as u16;
            } else if effective_rate_hi == 15 {
                // The reset reaches full output now; the next EG step moves
                // the attack state to decay, as in OPL3_EnvelopeCalc.
                operator.attenuation = 0;
            }
        }
        EnvelopeStage::Decay => {
            let sustain_level = if operator.sustain == 15 {
                31
            } else {
                operator.sustain
            };
            if operator.attenuation >> 4 == u16::from(sustain_level) {
                operator.stage = EnvelopeStage::Sustain;
            } else if shift > 0 {
                operator.attenuation = operator
                    .attenuation
                    .saturating_add(1 << (shift - 1))
                    .min(0x1ff);
            }
        }
        EnvelopeStage::Sustain | EnvelopeStage::Release => {
            if shift > 0 {
                operator.attenuation = operator
                    .attenuation
                    .saturating_add(1 << (shift - 1))
                    .min(0x1ff);
            }
        }
    }
}

fn ym3812_wave_sample(waveform: u8, phase: u16, attenuation_units: u16) -> f64 {
    let tables = amplitude_tables();
    let (sine_attenuation, negative) = match waveform {
        1 if phase & 0x0200 != 0 => (0x1000, false),
        3 if phase & 0x0100 != 0 => (0x1000, false),
        3 => (tables.log_sine[(phase & 0x00ff) as usize], false),
        _ => {
            let index = if phase & 0x0100 != 0 {
                ((phase & 0x00ff) ^ 0x00ff) as usize
            } else {
                (phase & 0x00ff) as usize
            };
            (tables.log_sine[index], waveform != 2 && phase & 0x0200 != 0)
        }
    };
    let level = sine_attenuation.saturating_add(attenuation_units).min(8191);
    let shift = u32::from(level >> 8);
    let magnitude = if shift >= 16 {
        0
    } else {
        (u32::from(tables.exponential[(level & 0x00ff) as usize]) << 1) >> shift
    } as i16;
    let output = if negative { !magnitude } else { magnitude };
    f64::from(output) / OPERATOR_OUTPUT_PEAK
}

fn phase_steps(phase: f64) -> u16 {
    (phase.rem_euclid(1.0) * 1024.0) as u16
}

fn vibrato_frequency(frequency: f64, enabled: bool, phase: f64, deep: bool, f_number: u16) -> f64 {
    if !enabled || f_number == 0 {
        return frequency;
    }
    const PM_SCALE: [i32; 8] = [0, 4, 8, 4, 0, -4, -8, -4];
    let phase_step = (phase.rem_euclid(1.0) * PM_SCALE.len() as f64) as usize;
    let raw_pm = PM_SCALE[phase_step] >> if deep { 0 } else { 1 };
    let base_f_number = i32::from(f_number) << 2;
    let f_number_scale = i32::from((f_number >> 7) & 0x07);
    let adjusted_f_number = (base_f_number + ((raw_pm * f_number_scale) >> 1)) & 0x0fff;
    frequency * f64::from(adjusted_f_number) / f64::from(base_f_number)
}

fn amplitude_modulation_attenuation_db(enabled: bool, phase: f64, deep: bool) -> f64 {
    if !enabled {
        return 0.0;
    }
    // YM3812 advances one of 210 triangle positions every 64 chip clocks.
    // Deep tremolo steps every four positions, shallow every sixteen; each
    // attenuation unit contributes 0.1875 dB.
    let position = (phase.rem_euclid(1.0) * 210.0).floor() as u16;
    let triangle = if position < 105 {
        position
    } else {
        210 - position
    };
    let attenuation_units = triangle >> if deep { 2 } else { 4 };
    f64::from(attenuation_units) * 0.1875
}

fn key_scaling_number(block: u8, f_number: u16, note_select: bool) -> u8 {
    let selected_f_number_bit = if note_select {
        (f_number >> 9) & 1
    } else {
        (f_number >> 8) & 1
    };
    ((block & 0x07) << 1) | selected_f_number_bit as u8
}

fn envelope_rate_index(rate: u8, key_scale_rate: bool, key_scale_number: u8) -> Option<usize> {
    if rate == 0 {
        return None;
    }
    let key_scale_offset = if key_scale_rate {
        key_scale_number
    } else {
        key_scale_number >> 2
    };
    Some((rate as usize * 4 + key_scale_offset as usize).min(63))
}

fn key_scale_attenuation_db(block: u8, f_number: u16, key_scale_level: u8) -> f64 {
    let table_value = KSL_3DB_TABLE[(block & 0x07) as usize][((f_number >> 6) & 0x0f) as usize];
    match key_scale_level & 0x03 {
        1 => table_value,
        2 => table_value / 2.0,
        3 => table_value * 2.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_scale_number_uses_the_selected_f_number_split_bit() {
        assert_eq!(key_scaling_number(3, 0x200, false), 6);
        assert_eq!(key_scaling_number(3, 0x200, true), 7);
    }

    #[test]
    fn key_scale_rate_adds_the_rate_offset_from_the_ym3812_table() {
        assert_eq!(envelope_rate_index(10, false, 14), Some(43));
        assert_eq!(envelope_rate_index(10, true, 14), Some(54));
        assert_eq!(envelope_rate_index(0, true, 15), None);
        assert_eq!(envelope_rate_index(15, true, 15), Some(63));
    }

    #[test]
    fn operator_register_decodes_ksr_and_note_select_bits() {
        let mut synth = Opl2Synth::new(48_000);
        synth.write_register(0x08, 0x40);
        synth.write_register(0x20, 0x30);

        assert!(synth.note_select);
        assert!(synth.channels[0].modulator.key_scale_rate);
        assert!(synth.channels[0].modulator.sustain_type);
    }

    #[test]
    fn zero_attack_rate_holds_the_envelope_at_its_current_level() {
        let mut operator = Operator {
            stage: EnvelopeStage::Attack,
            attack: 0,
            ..Operator::default()
        };

        advance_operator_envelope(&mut operator, 0, false, 0, 0);

        assert_eq!(operator.attenuation, 0x1ff);
        assert_eq!(operator.stage, EnvelopeStage::Attack);
    }

    #[test]
    fn zero_attack_rate_does_not_advance_an_already_full_envelope() {
        let mut operator = Operator {
            attenuation: 0,
            stage: EnvelopeStage::Attack,
            attack: 0,
            ..Operator::default()
        };

        advance_operator_envelope(&mut operator, 0, false, 0, 0);

        assert_eq!(operator.attenuation, 0);
        assert_eq!(operator.stage, EnvelopeStage::Attack);
    }

    #[test]
    fn zero_decay_and_release_rates_hold_the_current_envelope() {
        let mut decay = Operator {
            attenuation: 0,
            stage: EnvelopeStage::Decay,
            decay: 0,
            sustain: 8,
            ..Operator::default()
        };
        let mut release = Operator {
            attenuation: 128,
            stage: EnvelopeStage::Release,
            release: 0,
            ..Operator::default()
        };

        advance_operator_envelope(&mut decay, 0, false, 0, 0);
        advance_operator_envelope(&mut release, 0, false, 0, 0);

        assert_eq!(decay.attenuation, 0);
        assert_eq!(decay.stage, EnvelopeStage::Decay);
        assert_eq!(release.attenuation, 128);
        assert_eq!(release.stage, EnvelopeStage::Release);
    }

    #[test]
    fn silent_release_envelope_turns_off_even_with_zero_rate() {
        let mut operator = Operator {
            attenuation: 0x1ff,
            stage: EnvelopeStage::Release,
            release: 0,
            ..Operator::default()
        };

        advance_operator_envelope(&mut operator, 0, false, 0, 0);

        assert_eq!(operator.attenuation, 0x1ff);
        assert_eq!(operator.stage, EnvelopeStage::Off);
    }

    #[test]
    fn percussive_envelope_enters_release_after_reaching_sustain_level() {
        let mut operator = Operator {
            attenuation: 128,
            stage: EnvelopeStage::Decay,
            decay: 15,
            sustain: 8,
            release: 15,
            sustain_type: false,
            ..Operator::default()
        };

        advance_operator_envelope(&mut operator, 0, false, 0, 0);

        assert_eq!(operator.stage, EnvelopeStage::Sustain);
        assert_eq!(operator.attenuation, 128);
    }

    #[test]
    fn sustaining_envelope_holds_at_sustain_level_while_key_is_on() {
        let mut operator = Operator {
            attenuation: 128,
            stage: EnvelopeStage::Decay,
            decay: 15,
            sustain: 8,
            release: 15,
            sustain_type: true,
            ..Operator::default()
        };

        advance_operator_envelope(&mut operator, 0, false, 0, 0);

        assert_eq!(operator.stage, EnvelopeStage::Sustain);
        assert_eq!(operator.attenuation, 128);
    }

    #[test]
    fn key_scale_level_applies_the_ym3812_attenuation_slopes() {
        let f_number = 15 << 6;
        assert_eq!(key_scale_attenuation_db(1, f_number, 0), 0.0);
        assert_eq!(key_scale_attenuation_db(1, f_number, 1), 3.0);
        assert_eq!(key_scale_attenuation_db(1, f_number, 2), 1.5);
        assert_eq!(key_scale_attenuation_db(1, f_number, 3), 6.0);
    }

    #[test]
    fn operator_register_splits_key_scale_and_total_level() {
        let mut synth = Opl2Synth::new(48_000);
        synth.write_register(0x40, 0xA5);

        assert_eq!(synth.channels[0].modulator.key_scale_level, 2);
        assert_eq!(synth.channels[0].modulator.total_level, 0x25);
    }

    #[test]
    fn operator_and_bd_registers_enable_am_and_vibrato_depths() {
        let mut synth = Opl2Synth::new(48_000);
        synth.write_register(0xbd, 0xc0);
        synth.write_register(0x20, 0xd0);

        assert!(synth.am_depth);
        assert!(synth.vibrato_depth);
        assert!(synth.channels[0].modulator.amplitude_modulation);
        assert!(synth.channels[0].modulator.vibrato);
        assert!(synth.channels[0].modulator.key_scale_rate);
    }

    #[test]
    fn amplitude_modulation_uses_discrete_ym3812_depths() {
        assert_eq!(amplitude_modulation_attenuation_db(true, 0.5, false), 1.125);
        assert_eq!(amplitude_modulation_attenuation_db(true, 0.5, true), 4.875);
        assert_eq!(amplitude_modulation_attenuation_db(false, 0.5, true), 0.0);
        assert!(vibrato_frequency(440.0, true, 0.25, true, 0x01ff) > 440.0);
        assert_eq!(vibrato_frequency(440.0, false, 0.0, true, 0x01ff), 440.0);
    }
}
