//! Decoder for the original DOS demo stream embedded in VGA chunks.
//!
//! The four-byte header stores the absolute map number and the total stream
//! length. Each input record is three bytes: the eight DOS button states,
//! signed horizontal control, and signed vertical control. `PollControls`
//! holds every record for `DEMOTICS` (four) simulation tics.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DemoCommand {
    pub buttons: u8,
    pub control_x: i8,
    pub control_y: i8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemoRecording {
    pub map_number: usize,
    commands: Vec<DemoCommand>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemoPlayback {
    recording: DemoRecording,
    command_index: usize,
    tics_remaining: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DemoError {
    TruncatedHeader,
    InvalidLength,
    MisalignedCommands,
}

impl DemoRecording {
    pub fn parse(bytes: &[u8]) -> Result<Self, DemoError> {
        if bytes.len() < 4 {
            return Err(DemoError::TruncatedHeader);
        }
        let stream_length = u16::from_le_bytes([bytes[1], bytes[2]]) as usize;
        if stream_length < 4 || stream_length > bytes.len() {
            return Err(DemoError::InvalidLength);
        }
        let payload = &bytes[4..stream_length];
        if payload.len() % 3 != 0 {
            return Err(DemoError::MisalignedCommands);
        }
        let commands = payload
            .chunks_exact(3)
            .map(|record| DemoCommand {
                buttons: record[0],
                control_x: record[1] as i8,
                control_y: record[2] as i8,
            })
            .collect();
        Ok(Self {
            map_number: bytes[0] as usize,
            commands,
        })
    }

    pub fn playback(self) -> DemoPlayback {
        DemoPlayback {
            recording: self,
            command_index: 0,
            tics_remaining: 0,
        }
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
}

impl DemoPlayback {
    pub fn next_tic(&mut self) -> Option<DemoCommand> {
        if self.tics_remaining == 0 {
            let command = *self.recording.commands.get(self.command_index)?;
            self.command_index += 1;
            self.tics_remaining = 4;
            self.tics_remaining -= 1;
            Some(command)
        } else {
            self.tics_remaining -= 1;
            self.recording.commands.get(self.command_index - 1).copied()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DemoCommand, DemoError, DemoRecording};

    #[test]
    fn parses_original_header_and_signed_control_records() {
        let bytes = [7, 7, 0, 0, 0b0000_1011, 0xEC, 0x32];
        let demo = DemoRecording::parse(&bytes).unwrap();
        assert_eq!(demo.map_number, 7);
        assert_eq!(demo.command_count(), 1);
        let mut playback = demo.playback();
        let command = DemoCommand {
            buttons: 0b0000_1011,
            control_x: -20,
            control_y: 50,
        };
        for _ in 0..4 {
            assert_eq!(playback.next_tic(), Some(command));
        }
        assert_eq!(playback.next_tic(), None);
    }

    #[test]
    fn rejects_truncated_and_misaligned_demo_streams() {
        assert_eq!(
            DemoRecording::parse(&[0, 0, 0]),
            Err(DemoError::TruncatedHeader)
        );
        assert_eq!(
            DemoRecording::parse(&[0, 8, 0, 0, 1, 2, 3, 4]),
            Err(DemoError::MisalignedCommands)
        );
        assert_eq!(
            DemoRecording::parse(&[0, 9, 0, 0, 1, 2, 3]),
            Err(DemoError::InvalidLength)
        );
    }
}
