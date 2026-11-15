use crate::config::{
    DEFAULT_PREVIEW_SPEED, EVENT_SIZE, FRAME_SIZE, MAX_SONG_COUNT, MAX_TOTAL_FRAMES,
    MUSIC_CHANNELS, SONG_LENGTH_TABLE_SIZE, UI_SECTION_SIZE, WAVE_TABLE_COUNT, WAVE_TABLE_SIZE,
};

pub type Wave = [u8; WAVE_TABLE_SIZE];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Event {
    pub param: u8,
    pub volume_timbre: u8,
    pub extra: u8,
}

impl Event {
    pub fn volume(self) -> u8 {
        self.volume_timbre >> 4
    }

    pub fn timbre(self) -> u8 {
        self.volume_timbre & 0x0F
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.volume_timbre = (self.volume_timbre & 0x0F) | ((volume & 0x0F) << 4);
    }

    pub fn set_timbre(&mut self, timbre: u8) {
        self.volume_timbre = (self.volume_timbre & 0xF0) | (timbre & 0x0F);
    }

    pub fn is_empty(self) -> bool {
        self.param == 0 && self.volume_timbre == 0 && self.extra == 0
    }

    pub fn to_bytes(self) -> [u8; EVENT_SIZE] {
        [self.param, self.volume_timbre, self.extra]
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < EVENT_SIZE {
            return Self::default();
        }
        Self {
            param: bytes[0],
            volume_timbre: bytes[1],
            extra: bytes[2],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Song {
    pub frames: Vec<[Event; MUSIC_CHANNELS]>,
    pub preview_speed: u8,
}

impl Song {
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            preview_speed: DEFAULT_PREVIEW_SPEED,
        }
    }

    pub fn length(&self) -> usize {
        self.frames.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundData {
    pub waves: [Wave; WAVE_TABLE_COUNT],
    pub songs: Vec<Song>,
}

impl Default for SoundData {
    fn default() -> Self {
        let mut waves = [[0u8; WAVE_TABLE_SIZE]; WAVE_TABLE_COUNT];
        for byte in &mut waves[0][..8] {
            *byte = 0xFF;
        }
        let mut songs: Vec<Song> = (0..MAX_SONG_COUNT).map(|_| Song::new()).collect();
        songs[0]
            .frames
            .resize(8, [Event::default(); MUSIC_CHANNELS]);
        Self { waves, songs }
    }
}

impl SoundData {
    pub fn used_frames(&self) -> usize {
        self.songs.iter().map(Song::length).sum()
    }

    pub fn unused_frames(&self) -> usize {
        MAX_TOTAL_FRAMES.saturating_sub(self.used_frames())
    }

    pub fn to_wave_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(WAVE_TABLE_COUNT * WAVE_TABLE_SIZE);
        for wave in &self.waves {
            bytes.extend_from_slice(wave);
        }
        bytes
    }

    pub fn set_wave_bytes(&mut self, bytes: &[u8]) {
        for (index, wave) in self.waves.iter_mut().enumerate() {
            let start = index * WAVE_TABLE_SIZE;
            if start + WAVE_TABLE_SIZE <= bytes.len() {
                wave.copy_from_slice(&bytes[start..start + WAVE_TABLE_SIZE]);
            }
        }
    }

    pub fn to_sfx_bytes(&self) -> Vec<u8> {
        let total_frames = self.used_frames().min(MAX_TOTAL_FRAMES);
        let mut bytes = vec![0u8; SONG_LENGTH_TABLE_SIZE + total_frames * FRAME_SIZE];
        for (index, song) in self.songs.iter().take(MAX_SONG_COUNT).enumerate() {
            bytes[index] = song.length().min(255) as u8;
        }
        let mut offset = SONG_LENGTH_TABLE_SIZE;
        for song in self.songs.iter().take(MAX_SONG_COUNT) {
            for frame in &song.frames {
                if offset + FRAME_SIZE > bytes.len() {
                    return bytes;
                }
                for event in frame {
                    bytes[offset..offset + EVENT_SIZE].copy_from_slice(&event.to_bytes());
                    offset += EVENT_SIZE;
                }
            }
        }
        bytes
    }

    pub fn set_sfx_bytes(&mut self, bytes: &[u8]) {
        for song in &mut self.songs {
            song.frames.clear();
        }
        if bytes.len() < SONG_LENGTH_TABLE_SIZE {
            return;
        }
        let mut offset = SONG_LENGTH_TABLE_SIZE;
        for (index, song) in self.songs.iter_mut().take(MAX_SONG_COUNT).enumerate() {
            let length = bytes[index] as usize;
            let byte_len = length * FRAME_SIZE;
            if offset + byte_len > bytes.len() {
                break;
            }
            song.frames.reserve(length);
            for _ in 0..length {
                let mut frame = [Event::default(); MUSIC_CHANNELS];
                for cell in &mut frame {
                    *cell = Event::from_bytes(&bytes[offset..offset + EVENT_SIZE]);
                    offset += EVENT_SIZE;
                }
                song.frames.push(frame);
            }
        }
    }

    pub fn to_ui_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![DEFAULT_PREVIEW_SPEED; UI_SECTION_SIZE];
        for (index, song) in self.songs.iter().take(UI_SECTION_SIZE).enumerate() {
            bytes[index] = song.preview_speed.max(1);
        }
        bytes
    }

    pub fn set_ui_bytes(&mut self, bytes: &[u8]) {
        for (index, song) in self.songs.iter_mut().take(UI_SECTION_SIZE).enumerate() {
            let speed = bytes.get(index).copied().unwrap_or(DEFAULT_PREVIEW_SPEED);
            song.preview_speed = if speed == 0 {
                DEFAULT_PREVIEW_SPEED
            } else {
                speed
            };
        }
    }

    pub fn append_to_source(&self, content: &mut String) {
        append_hex_section(content, "__wav__", &self.to_wave_bytes());
        append_hex_section(content, "__sfx__", &self.to_sfx_bytes());
        if self
            .songs
            .iter()
            .any(|song| song.preview_speed != DEFAULT_PREVIEW_SPEED)
        {
            append_hex_section(content, "__ui__", &self.to_ui_bytes());
        }
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

    #[test]
    fn default_song_count_and_capacity() {
        let data = SoundData::default();
        assert_eq!(data.songs.len(), MAX_SONG_COUNT);
        assert_eq!(data.songs[0].length(), 8);
        assert_eq!(data.used_frames(), 8);
        assert_eq!(data.unused_frames(), MAX_TOTAL_FRAMES - 8);
        assert_eq!(
            data.to_sfx_bytes().len(),
            SONG_LENGTH_TABLE_SIZE + 8 * FRAME_SIZE
        );
        assert_eq!(data.to_ui_bytes().len(), UI_SECTION_SIZE);
    }

    #[test]
    fn sfx_roundtrip() {
        let mut data = SoundData::default();
        data.songs[0].frames.clear();
        data.songs[2].frames.resize(
            3,
            [Event {
                param: 0,
                volume_timbre: 0,
                extra: 0,
            }; MUSIC_CHANNELS],
        );
        data.songs[2].frames[1][0] = Event {
            param: 34,
            volume_timbre: 0xC2,
            extra: 0x13,
        };
        data.songs[7]
            .frames
            .resize(2, [Event::default(); MUSIC_CHANNELS]);
        let bytes = data.to_sfx_bytes();
        assert_eq!(bytes.len(), SONG_LENGTH_TABLE_SIZE + 5 * FRAME_SIZE);
        let mut decoded = SoundData::default();
        decoded.set_sfx_bytes(&bytes);
        assert_eq!(decoded.songs[2].frames[1][0], data.songs[2].frames[1][0]);
        assert_eq!(decoded.songs[2].length(), 3);
        assert_eq!(decoded.songs[7].length(), 2);
        assert_eq!(decoded.used_frames(), 5);
    }

    #[test]
    fn default_song_section_and_empty_section() {
        let data = SoundData::default();
        assert_eq!(
            data.to_sfx_bytes().len(),
            SONG_LENGTH_TABLE_SIZE + 8 * FRAME_SIZE
        );
        let mut content = String::new();
        data.append_to_source(&mut content);
        assert!(content.contains("__sfx__"));

        let mut empty = SoundData::default();
        for song in &mut empty.songs {
            song.frames.clear();
        }
        let mut content = String::new();
        empty.append_to_source(&mut content);
        assert!(!content.contains("__sfx__"));

        let mut loaded = SoundData::default();
        assert_eq!(loaded.songs[0].length(), 8);
        loaded.set_sfx_bytes(&[]);
        assert_eq!(loaded.songs[0].length(), 0);
    }

    #[test]
    fn wave_and_ui_roundtrip() {
        let mut data = SoundData::default();
        data.waves[3][0] = 0xAB;
        data.songs[5].preview_speed = 120;
        let mut decoded = SoundData::default();
        decoded.set_wave_bytes(&data.to_wave_bytes());
        decoded.set_ui_bytes(&data.to_ui_bytes());
        assert_eq!(decoded.waves[3][0], 0xAB);
        assert_eq!(decoded.songs[5].preview_speed, 120);
        assert_eq!(decoded.songs[0].preview_speed, DEFAULT_PREVIEW_SPEED);
    }

    #[test]
    fn event_bytes_roundtrip() {
        let event = Event {
            param: 73,
            volume_timbre: 0xF3,
            extra: 0x81,
        };
        assert_eq!(Event::from_bytes(&event.to_bytes()), event);
        assert_eq!(event.volume(), 15);
        assert_eq!(event.timbre(), 3);
    }
}
