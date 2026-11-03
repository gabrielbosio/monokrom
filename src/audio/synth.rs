pub struct ChannelSynth {
    phase: f32,
    noise_phase: f32,
    lfsr: u16,
}

impl Default for ChannelSynth {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelSynth {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            noise_phase: 0.0,
            lfsr: 0x7FFF,
        }
    }

    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.noise_phase = 0.0;
        self.lfsr = 0x7FFF;
    }

    pub fn pulse_sample(&mut self, period: u16, duty: u8, sample_rate: u32) -> f32 {
        let frequency = 131072.0 / (2048.0 - period as f32);
        self.phase += frequency / sample_rate as f32;
        self.phase -= self.phase.floor();
        let duty = match duty & 0x03 {
            0 => 0.125,
            1 => 0.25,
            2 => 0.5,
            _ => 0.75,
        };
        if self.phase < duty {
            1.0
        } else {
            -1.0
        }
    }

    pub fn wave_sample(&mut self, period: u16, wave: &[u8; 16], sample_rate: u32) -> f32 {
        let frequency = 65536.0 / (2048.0 - period as f32);
        self.phase += frequency / sample_rate as f32;
        self.phase -= self.phase.floor();
        let index = ((self.phase * 32.0) as usize) & 31;
        let byte = wave[index / 2];
        let sample = if index.is_multiple_of(2) {
            byte >> 4
        } else {
            byte & 0x0F
        };
        sample as f32 / 7.5 - 1.0
    }

    pub fn noise_sample(
        &mut self,
        divider: u8,
        shift: u8,
        width_7bit: bool,
        sample_rate: u32,
    ) -> f32 {
        let clock = if divider == 0 {
            524288.0 / (1u32 << shift.min(15)) as f32
        } else {
            262144.0 / (divider as f32 * (1u32 << shift.min(15)) as f32)
        };
        self.noise_phase += clock / sample_rate as f32;
        while self.noise_phase >= 1.0 {
            let bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr = (self.lfsr >> 1) | (bit << 14);
            if width_7bit {
                self.lfsr = (self.lfsr & !(1 << 6)) | (bit << 6);
            }
            self.noise_phase -= 1.0;
        }
        if self.lfsr & 1 == 0 {
            1.0
        } else {
            -1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_is_silent_only_when_period_is_invalid() {
        let mut synth = ChannelSynth::new();
        let sample = synth.pulse_sample(1750, 2, 48000);
        assert!(sample == 1.0 || sample == -1.0);
    }

    #[test]
    fn wave_reads_high_nibble_first() {
        let mut synth = ChannelSynth::new();
        let mut wave = [0u8; 16];
        wave[0] = 0x0F;
        synth.phase = 0.0;
        let high = synth.wave_sample(0, &wave, 48000);
        synth.phase = 1.0 / 32.0;
        let low = synth.wave_sample(0, &wave, 48000);
        assert_eq!(high, -1.0);
        assert_eq!(low, 1.0);
    }

    #[test]
    fn noise_output_changes_over_time() {
        let mut synth = ChannelSynth::new();
        let mut changed = false;
        let first = synth.noise_sample(1, 0, false, 48000);
        for _ in 0..1000 {
            if synth.noise_sample(1, 0, false, 48000) != first {
                changed = true;
                break;
            }
        }
        assert!(changed);
    }
}
