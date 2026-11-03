use crate::config::{
    AUDIO_INSTRUMENT_COUNT, AUDIO_INSTRUMENT_OFFSET, AUDIO_INSTRUMENT_SIZE, AUDIO_REGION_SIZE,
    AUDIO_SFX_COUNT, AUDIO_SFX_OFFSET, AUDIO_SFX_SIZE, AUDIO_TABLE_COUNT, AUDIO_TABLE_HEADER_BYTES,
    AUDIO_TABLE_OFFSET, AUDIO_TABLE_SIZE, AUDIO_TABLE_STEPS, AUDIO_TABLE_STEP_SIZE,
    AUDIO_WAVE_COUNT, AUDIO_WAVE_REGION_SIZE, AUDIO_WAVE_SIZE, MUSIC_CHANNELS, MUSIC_ROWS,
    SONG_COUNT, SONG_END_MASK, SONG_HEADER_BYTES, SONG_HEADER_START, SONG_LOOP_ENABLED,
    SONG_ORDER_COUNT, SONG_ORDER_MASK, SONG_ORDER_REGION_SIZE, SONG_PATTERN_COUNT,
    SONG_PATTERN_REGION_SIZE, SONG_REGION_SIZE, SONG_REGION_START, SONG_SIZE, SONG_TABLE_START,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ChannelType {
    #[default]
    Pulse = 0,
    Wave = 1,
    Noise = 2,
}

impl ChannelType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Pulse),
            1 => Some(Self::Wave),
            2 => Some(Self::Noise),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pulse => "PU",
            Self::Wave => "WV",
            Self::Noise => "NS",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instrument {
    pub channel: ChannelType,
    pub volume: u8,
    pub envelope_dir_up: bool,
    pub envelope_period: u8,
    pub duty: u8,
    pub length: u8,
    pub length_enabled: bool,
    pub sweep_pace: u8,
    pub sweep_dir_up: bool,
    pub sweep_step: u8,
    pub wave_slot: u8,
    pub wave_volume: u8,
    pub noise_width_7bit: bool,
    pub noise_divider: u8,
    pub noise_shift: u8,
    pub table: u8,
    pub panning: u8,
    pub finetune: i8,
}

impl Default for Instrument {
    fn default() -> Self {
        Self {
            channel: ChannelType::Pulse,
            volume: 0,
            envelope_dir_up: false,
            envelope_period: 0,
            duty: 2,
            length: 0,
            length_enabled: false,
            sweep_pace: 0,
            sweep_dir_up: false,
            sweep_step: 0,
            wave_slot: 0,
            wave_volume: 3,
            noise_width_7bit: false,
            noise_divider: 0,
            noise_shift: 0,
            table: 0,
            panning: 0b11,
            finetune: 0,
        }
    }
}

impl Instrument {
    pub fn to_bytes(self) -> [u8; AUDIO_INSTRUMENT_SIZE] {
        let mut bytes = [0u8; AUDIO_INSTRUMENT_SIZE];
        bytes[0] = self.channel as u8;
        bytes[1] = (self.volume & 0x0F)
            | ((self.envelope_dir_up as u8) << 4)
            | ((self.envelope_period & 0x07) << 5);
        bytes[2] = (self.duty & 0x03)
            | ((self.wave_volume & 0x03) << 2)
            | ((self.noise_width_7bit as u8) << 4)
            | ((self.length_enabled as u8) << 5);
        bytes[3] = self.length;
        bytes[4] = (self.sweep_pace & 0x07)
            | ((self.sweep_dir_up as u8) << 3)
            | ((self.sweep_step & 0x07) << 4);
        bytes[5] = (self.wave_slot & 0x0F) | ((self.noise_divider & 0x07) << 4);
        bytes[6] = (self.noise_shift & 0x0F) | ((self.table & 0x0F) << 4);
        bytes[7] = self.panning & 0x03;
        bytes[8] = self.finetune as u8;
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut instrument = Self::default();
        if bytes.len() < AUDIO_INSTRUMENT_SIZE {
            return instrument;
        }
        instrument.channel = ChannelType::from_u8(bytes[0] & 0x03).unwrap_or(ChannelType::Pulse);
        instrument.volume = bytes[1] & 0x0F;
        instrument.envelope_dir_up = bytes[1] & 0x10 != 0;
        instrument.envelope_period = (bytes[1] >> 5) & 0x07;
        instrument.duty = bytes[2] & 0x03;
        instrument.wave_volume = (bytes[2] >> 2) & 0x03;
        instrument.noise_width_7bit = bytes[2] & 0x10 != 0;
        instrument.length_enabled = bytes[2] & 0x20 != 0;
        instrument.length = bytes[3];
        instrument.sweep_pace = bytes[4] & 0x07;
        instrument.sweep_dir_up = bytes[4] & 0x08 != 0;
        instrument.sweep_step = (bytes[4] >> 4) & 0x07;
        instrument.wave_slot = bytes[5] & 0x0F;
        instrument.noise_divider = (bytes[5] >> 4) & 0x07;
        instrument.noise_shift = bytes[6] & 0x0F;
        instrument.table = (bytes[6] >> 4) & 0x0F;
        instrument.panning = bytes[7] & 0x03;
        instrument.finetune = bytes[8] as i8;
        instrument
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TableStep {
    pub pitch: i8,
    pub volume: u8,
    pub shape: u8,
    pub detune: u8,
    pub command: u8,
}

impl TableStep {
    pub fn to_bytes(self) -> [u8; AUDIO_TABLE_STEP_SIZE] {
        let packed = (self.pitch as u8 as u32)
            | ((self.volume as u32 & 0x0F) << 8)
            | ((self.shape as u32 & 0x0F) << 12)
            | ((self.detune as u32 & 0x0F) << 16)
            | ((self.command as u32 & 0x0F) << 20);
        [packed as u8, (packed >> 8) as u8, (packed >> 16) as u8]
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < AUDIO_TABLE_STEP_SIZE {
            return Self::default();
        }
        let packed = bytes[0] as u32 | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16);
        Self {
            pitch: packed as u8 as i8,
            volume: ((packed >> 8) & 0x0F) as u8,
            shape: ((packed >> 12) & 0x0F) as u8,
            detune: ((packed >> 16) & 0x0F) as u8,
            command: ((packed >> 20) & 0x0F) as u8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Table {
    pub speed: u8,
    pub loop_enabled: bool,
    pub loop_start: u8,
    pub loop_end: u8,
    pub steps: [TableStep; AUDIO_TABLE_STEPS],
}

impl Default for Table {
    fn default() -> Self {
        Self {
            speed: 1,
            loop_enabled: false,
            loop_start: 0,
            loop_end: 0,
            steps: [TableStep::default(); AUDIO_TABLE_STEPS],
        }
    }
}

impl Table {
    pub fn to_bytes(self) -> [u8; AUDIO_TABLE_SIZE] {
        let mut bytes = [0u8; AUDIO_TABLE_SIZE];
        bytes[0] = self.speed;
        bytes[1] = self.loop_start;
        bytes[2] = self.loop_end;
        bytes[3] = self.loop_enabled as u8;
        for (index, step) in self.steps.iter().enumerate() {
            let offset = AUDIO_TABLE_HEADER_BYTES + index * AUDIO_TABLE_STEP_SIZE;
            let step_bytes = step.to_bytes();
            bytes[offset..offset + AUDIO_TABLE_STEP_SIZE].copy_from_slice(&step_bytes);
        }
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut table = Self::default();
        if bytes.len() < AUDIO_TABLE_SIZE {
            return table;
        }
        table.speed = bytes[0];
        table.loop_start = bytes[1];
        table.loop_end = bytes[2];
        table.loop_enabled = bytes[3] & 1 != 0;
        for index in 0..AUDIO_TABLE_STEPS {
            let offset = AUDIO_TABLE_HEADER_BYTES + index * AUDIO_TABLE_STEP_SIZE;
            table.steps[index] =
                TableStep::from_bytes(&bytes[offset..offset + AUDIO_TABLE_STEP_SIZE]);
        }
        table
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SfxEntry {
    pub channel: ChannelType,
    pub instrument: u8,
    pub table: u8,
    pub note: u8,
    pub priority: u8,
    pub duration: u8,
}

impl SfxEntry {
    pub fn to_bytes(self) -> [u8; AUDIO_SFX_SIZE] {
        [
            self.channel as u8,
            self.instrument,
            self.table,
            self.note,
            self.priority & 0x03,
            self.duration,
        ]
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < AUDIO_SFX_SIZE {
            return Self::default();
        }
        Self {
            channel: ChannelType::from_u8(bytes[0] & 0x03).unwrap_or(ChannelType::Pulse),
            instrument: bytes[1],
            table: bytes[2],
            note: bytes[3],
            priority: bytes[4] & 0x03,
            duration: bytes[5],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MusicCell {
    pub note: u8,
    pub instrument: u8,
    pub effect: u8,
    pub value: u8,
}

impl MusicCell {
    pub fn pack(self) -> u32 {
        (self.note as u32 & 0x7F)
            | ((self.instrument as u32 & 0x1F) << 7)
            | ((self.effect as u32 & 0x0F) << 12)
            | ((self.value as u32) << 16)
    }

    pub fn unpack(value: u32) -> Self {
        Self {
            note: (value & 0x7F) as u8,
            instrument: ((value >> 7) & 0x1F) as u8,
            effect: ((value >> 12) & 0x0F) as u8,
            value: ((value >> 16) & 0xFF) as u8,
        }
    }

    pub fn to_bytes(self) -> [u8; 3] {
        let packed = self.pack();
        [packed as u8, (packed >> 8) as u8, (packed >> 16) as u8]
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < 3 {
            return Self::default();
        }
        Self::unpack(bytes[0] as u32 | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Order {
    pub patterns: [u8; MUSIC_CHANNELS],
}

impl Order {
    pub fn to_bytes(self) -> [u8; 4] {
        self.patterns
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut patterns = [0u8; MUSIC_CHANNELS];
        for (index, slot) in patterns.iter_mut().enumerate() {
            *slot = bytes.get(index).copied().unwrap_or(0) & 0x0F;
        }
        Self { patterns }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Song {
    pub order_start: u8,
    pub loop_start: u8,
    pub loop_end: u8,
    pub loop_enabled: bool,
}

impl Song {
    pub fn to_bytes(self) -> [u8; SONG_SIZE] {
        [
            self.order_start & SONG_ORDER_MASK,
            (self.loop_start & SONG_ORDER_MASK)
                | if self.loop_enabled {
                    SONG_LOOP_ENABLED
                } else {
                    0
                },
            self.loop_end & SONG_END_MASK,
        ]
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < SONG_SIZE {
            return Self::default();
        }
        Self {
            order_start: bytes[0] & SONG_ORDER_MASK,
            loop_start: bytes[1] & SONG_ORDER_MASK,
            loop_enabled: bytes[1] & SONG_LOOP_ENABLED != 0,
            loop_end: bytes[2] & SONG_END_MASK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MusicHeader {
    pub ticks_per_row: u8,
    pub order_count: u8,
}

impl Default for MusicHeader {
    fn default() -> Self {
        Self {
            ticks_per_row: 6,
            order_count: 0,
        }
    }
}

impl MusicHeader {
    pub fn to_bytes(self) -> [u8; SONG_HEADER_BYTES] {
        let mut bytes = [0u8; SONG_HEADER_BYTES];
        bytes[0] = self.ticks_per_row.max(1);
        bytes[1] = self.order_count;
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < SONG_HEADER_BYTES {
            return Self::default();
        }
        Self {
            ticks_per_row: bytes[0].max(1),
            order_count: bytes[1],
        }
    }
}

pub type Wave = [u8; AUDIO_WAVE_SIZE];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundData {
    pub waves: [Wave; AUDIO_WAVE_COUNT],
    pub instruments: [Instrument; AUDIO_INSTRUMENT_COUNT],
    pub tables: [Table; AUDIO_TABLE_COUNT],
    pub sfx: [SfxEntry; AUDIO_SFX_COUNT],
    pub patterns: [[[MusicCell; MUSIC_ROWS]; SONG_PATTERN_COUNT]; MUSIC_CHANNELS],
    pub orders: [Order; SONG_ORDER_COUNT],
    pub songs: [Song; SONG_COUNT],
    pub header: MusicHeader,
}

impl Default for SoundData {
    fn default() -> Self {
        let mut waves = [[0u8; AUDIO_WAVE_SIZE]; AUDIO_WAVE_COUNT];
        // A square wave in the first slot gives new wave instruments
        // something audible to start from.
        for byte in &mut waves[0][..8] {
            *byte = 0xFF;
        }
        let mut instruments = [Instrument::default(); AUDIO_INSTRUMENT_COUNT];
        instruments[1] = Instrument {
            channel: ChannelType::Pulse,
            volume: 15,
            duty: 2,
            panning: 0b11,
            ..Instrument::default()
        };
        let mut songs = [Song::default(); SONG_COUNT];
        songs[0] = Song {
            order_start: 0,
            loop_start: 0,
            loop_end: 1,
            loop_enabled: false,
        };
        Self {
            waves,
            instruments,
            tables: [Table::default(); AUDIO_TABLE_COUNT],
            sfx: [SfxEntry::default(); AUDIO_SFX_COUNT],
            patterns: [[[MusicCell::default(); MUSIC_ROWS]; SONG_PATTERN_COUNT]; MUSIC_CHANNELS],
            orders: [Order::default(); SONG_ORDER_COUNT],
            songs,
            header: MusicHeader {
                ticks_per_row: 6,
                order_count: 1,
            },
        }
    }
}

impl SoundData {
    pub fn to_audio_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0u8; AUDIO_REGION_SIZE];
        for (index, wave) in self.waves.iter().enumerate() {
            let offset = index * AUDIO_WAVE_SIZE;
            bytes[offset..offset + AUDIO_WAVE_SIZE].copy_from_slice(wave);
        }
        for (index, instrument) in self.instruments.iter().enumerate() {
            let offset = AUDIO_INSTRUMENT_OFFSET + index * AUDIO_INSTRUMENT_SIZE;
            bytes[offset..offset + AUDIO_INSTRUMENT_SIZE].copy_from_slice(&instrument.to_bytes());
        }
        for (index, table) in self.tables.iter().enumerate() {
            let offset = AUDIO_TABLE_OFFSET + index * AUDIO_TABLE_SIZE;
            bytes[offset..offset + AUDIO_TABLE_SIZE].copy_from_slice(&table.to_bytes());
        }
        for (index, sfx) in self.sfx.iter().enumerate() {
            let offset = AUDIO_SFX_OFFSET + index * AUDIO_SFX_SIZE;
            bytes[offset..offset + AUDIO_SFX_SIZE].copy_from_slice(&sfx.to_bytes());
        }
        bytes
    }

    pub fn from_audio_bytes(bytes: &[u8]) -> Self {
        let mut data = Self::default();
        if bytes.len() < AUDIO_REGION_SIZE {
            return data;
        }
        for (index, wave) in data.waves.iter_mut().enumerate() {
            let offset = index * AUDIO_WAVE_SIZE;
            wave.copy_from_slice(&bytes[offset..offset + AUDIO_WAVE_SIZE]);
        }
        for (index, instrument) in data.instruments.iter_mut().enumerate() {
            let offset = AUDIO_INSTRUMENT_OFFSET + index * AUDIO_INSTRUMENT_SIZE;
            *instrument = Instrument::from_bytes(&bytes[offset..offset + AUDIO_INSTRUMENT_SIZE]);
        }
        for (index, table) in data.tables.iter_mut().enumerate() {
            let offset = AUDIO_TABLE_OFFSET + index * AUDIO_TABLE_SIZE;
            *table = Table::from_bytes(&bytes[offset..offset + AUDIO_TABLE_SIZE]);
        }
        for (index, sfx) in data.sfx.iter_mut().enumerate() {
            let offset = AUDIO_SFX_OFFSET + index * AUDIO_SFX_SIZE;
            *sfx = SfxEntry::from_bytes(&bytes[offset..offset + AUDIO_SFX_SIZE]);
        }
        data
    }

    pub fn to_song_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0u8; SONG_REGION_SIZE];
        let mut offset = 0;
        for channel in 0..MUSIC_CHANNELS {
            for pattern in 0..SONG_PATTERN_COUNT {
                for row in 0..MUSIC_ROWS {
                    let cell = self.patterns[channel][pattern][row];
                    bytes[offset..offset + 3].copy_from_slice(&cell.to_bytes());
                    offset += 3;
                }
            }
        }
        debug_assert_eq!(offset, SONG_PATTERN_REGION_SIZE);
        for order in &self.orders {
            bytes[offset..offset + 4].copy_from_slice(&order.to_bytes());
            offset += 4;
        }
        debug_assert_eq!(offset, SONG_PATTERN_REGION_SIZE + SONG_ORDER_REGION_SIZE);
        let header_offset = SONG_HEADER_START - SONG_REGION_START;
        bytes[header_offset..header_offset + SONG_HEADER_BYTES]
            .copy_from_slice(&self.header.to_bytes());
        let table_offset = SONG_TABLE_START - SONG_REGION_START;
        for (index, song) in self.songs.iter().enumerate() {
            let offset = table_offset + index * SONG_SIZE;
            bytes[offset..offset + SONG_SIZE].copy_from_slice(&song.to_bytes());
        }
        debug_assert!(table_offset + crate::config::SONG_TABLE_SIZE <= SONG_REGION_SIZE);
        bytes
    }

    pub fn from_song_bytes(bytes: &[u8]) -> Self {
        let mut data = Self::default();
        if bytes.len() < SONG_REGION_SIZE {
            return data;
        }
        let mut offset = 0;
        for channel in 0..MUSIC_CHANNELS {
            for pattern in 0..SONG_PATTERN_COUNT {
                for row in 0..MUSIC_ROWS {
                    data.patterns[channel][pattern][row] =
                        MusicCell::from_bytes(&bytes[offset..offset + 3]);
                    offset += 3;
                }
            }
        }
        for order in data.orders.iter_mut() {
            *order = Order::from_bytes(&bytes[offset..offset + 4]);
            offset += 4;
        }
        let header_offset = SONG_HEADER_START - SONG_REGION_START;
        data.header =
            MusicHeader::from_bytes(&bytes[header_offset..header_offset + SONG_HEADER_BYTES]);
        let table_offset = SONG_TABLE_START - SONG_REGION_START;
        for (index, song) in data.songs.iter_mut().enumerate() {
            let offset = table_offset + index * SONG_SIZE;
            *song = Song::from_bytes(&bytes[offset..offset + SONG_SIZE]);
        }
        data
    }

    pub fn append_to_source(&self, content: &mut String) {
        let audio = self.to_audio_bytes();
        append_hex_section(content, "__wav__", &audio[..AUDIO_WAVE_REGION_SIZE]);
        append_hex_section(
            content,
            "__ins__",
            &audio[AUDIO_INSTRUMENT_OFFSET
                ..AUDIO_INSTRUMENT_OFFSET + AUDIO_INSTRUMENT_COUNT * AUDIO_INSTRUMENT_SIZE],
        );
        append_hex_section(
            content,
            "__tbl__",
            &audio[AUDIO_TABLE_OFFSET..AUDIO_TABLE_OFFSET + AUDIO_TABLE_COUNT * AUDIO_TABLE_SIZE],
        );
        append_hex_section(
            content,
            "__sfx__",
            &audio[AUDIO_SFX_OFFSET..AUDIO_SFX_OFFSET + AUDIO_SFX_COUNT * AUDIO_SFX_SIZE],
        );
        let song = self.to_song_bytes();
        append_hex_section(content, "__mus__", &song);
    }
}

fn append_hex_section(content: &mut String, marker: &str, bytes: &[u8]) {
    if bytes.iter().all(|&byte| byte == 0) {
        return;
    }
    content.push('\n');
    content.push_str(marker);
    content.push('\n');
    for chunk in bytes.chunks(16) {
        for byte in chunk {
            content.push_str(&format!("{byte:02x}"));
        }
        content.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AUDIO_INSTRUMENT_OFFSET, AUDIO_SFX_OFFSET, AUDIO_TABLE_OFFSET, SONG_HEADER_START,
        SONG_TABLE_START,
    };

    #[test]
    fn default_instrument_is_one() {
        let data = SoundData::default();
        assert_eq!(data.instruments[0].volume, 0);
        assert_eq!(data.instruments[1].volume, 15);
    }

    #[test]
    fn instrument_roundtrip() {
        let instrument = Instrument {
            channel: ChannelType::Noise,
            volume: 12,
            envelope_dir_up: true,
            envelope_period: 5,
            duty: 3,
            length: 42,
            length_enabled: true,
            sweep_pace: 6,
            sweep_dir_up: true,
            sweep_step: 2,
            wave_slot: 9,
            wave_volume: 2,
            noise_width_7bit: true,
            noise_divider: 5,
            noise_shift: 11,
            table: 7,
            panning: 0b10,
            finetune: -12,
        };
        assert_eq!(Instrument::from_bytes(&instrument.to_bytes()), instrument);
    }

    #[test]
    fn table_roundtrip() {
        let mut table = Table {
            speed: 3,
            loop_enabled: true,
            loop_start: 4,
            loop_end: 12,
            ..Table::default()
        };
        table.steps[0] = TableStep {
            pitch: -12,
            volume: 15,
            shape: 3,
            detune: 7,
            command: 2,
        };
        table.steps[31] = TableStep {
            pitch: 9,
            volume: 1,
            shape: 15,
            detune: 15,
            command: 1,
        };
        assert_eq!(Table::from_bytes(&table.to_bytes()), table);
    }

    #[test]
    fn sfx_roundtrip() {
        let sfx = SfxEntry {
            channel: ChannelType::Wave,
            instrument: 5,
            table: 3,
            note: 36,
            priority: 2,
            duration: 17,
        };
        assert_eq!(SfxEntry::from_bytes(&sfx.to_bytes()), sfx);
    }

    #[test]
    fn music_cell_roundtrip() {
        let cell = MusicCell {
            note: 72,
            instrument: 31,
            effect: 14,
            value: 255,
        };
        assert_eq!(MusicCell::from_bytes(&cell.to_bytes()), cell);
    }

    #[test]
    fn song_loop_flag_roundtrip() {
        let song = Song {
            order_start: 3,
            loop_start: 7,
            loop_end: 64,
            loop_enabled: true,
        };
        let bytes = song.to_bytes();
        assert_eq!(bytes[1] & SONG_LOOP_ENABLED, SONG_LOOP_ENABLED);
        assert_eq!(Song::from_bytes(&bytes), song);
    }

    #[test]
    fn song_no_loop_preserves_loop_start() {
        let song = Song {
            order_start: 0,
            loop_start: 12,
            loop_end: 20,
            loop_enabled: false,
        };
        let bytes = song.to_bytes();
        assert_eq!(bytes[1] & SONG_LOOP_ENABLED, 0);
        assert_eq!(Song::from_bytes(&bytes).loop_start, 12);
    }

    #[test]
    fn sound_data_audio_roundtrip() {
        let mut data = SoundData::default();
        data.waves[3][0] = 0xAB;
        data.instruments[4].volume = 15;
        data.tables[2].loop_end = 8;
        data.sfx[1].note = 42;
        let bytes = data.to_audio_bytes();
        assert_eq!(bytes[AUDIO_INSTRUMENT_OFFSET], 0);
        assert_eq!(
            SoundData::from_audio_bytes(&bytes).instruments[4].volume,
            15
        );
        assert_eq!(SoundData::from_audio_bytes(&bytes).sfx[1].note, 42);
        assert_eq!(bytes.len(), AUDIO_REGION_SIZE);
        assert!(AUDIO_SFX_OFFSET > AUDIO_TABLE_OFFSET);
    }

    #[test]
    fn sound_data_song_roundtrip() {
        let mut data = SoundData::default();
        data.header = MusicHeader {
            ticks_per_row: 8,
            order_count: 24,
        };
        data.patterns[2][5][17] = MusicCell {
            note: 40,
            instrument: 3,
            effect: 9,
            value: 12,
        };
        data.orders[6].patterns = [1, 2, 3, 4];
        data.songs[2] = Song {
            order_start: 4,
            loop_start: 8,
            loop_end: 20,
            loop_enabled: true,
        };
        let bytes = data.to_song_bytes();
        let decoded = SoundData::from_song_bytes(&bytes);
        assert_eq!(decoded.patterns[2][5][17], data.patterns[2][5][17]);
        assert_eq!(decoded.orders[6], data.orders[6]);
        assert_eq!(decoded.songs[2], data.songs[2]);
        assert_eq!(decoded.header, data.header);
        assert_eq!(bytes.len(), SONG_REGION_SIZE);
        assert!(SONG_HEADER_START > 0);
        assert!(SONG_TABLE_START > SONG_HEADER_START);
    }
}
