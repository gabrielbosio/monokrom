use std::collections::VecDeque;

use macroquad::prelude::*;

use crate::audio::format::{Event, SoundData};
use crate::audio::notes::{note_parts, NOTE_MAX, NOTE_MIN, NOTE_OFF};
use crate::config::{
    COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, MAX_SONG_COUNT, MUSIC_CHANNELS, TILE_WIDTH,
};
use crate::input::{
    is_arrow_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed, should_key_fire,
};
use crate::render::DrawHelpers;

const UNDO_LIMIT: usize = 16;
const VISIBLE_FRAMES: usize = 12;
const HEADER_Y: f32 = 0.0;
const ROWS_Y: f32 = 12.0;
const ROW_HEIGHT: f32 = 6.0;
const CELL_WIDTH: f32 = 12.0;
const CELLS_X: f32 = 12.0;
const SCROLLBAR_Y: f32 = 37.0;
const INFO_Y: f32 = 42.0;
const CH_NAMES: [&str; MUSIC_CHANNELS] = ["P1", "P2", "WV", "NS"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeaderField {
    Sfx,
    Speed,
    Len,
}

impl HeaderField {
    fn next(self) -> Self {
        match self {
            Self::Sfx => Self::Speed,
            Self::Speed => Self::Len,
            Self::Len => Self::Sfx,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Sfx => Self::Len,
            Self::Speed => Self::Sfx,
            Self::Len => Self::Speed,
        }
    }
}

pub enum SfxEditorAction {
    None,
    ExitToTerminal,
    Save,
    Run,
    Preview { song: u8, frame: u16, toggle: bool },
    Stop,
}

pub struct SfxEditorOutput {
    pub action: SfxEditorAction,
    pub modified: bool,
}

impl SfxEditorOutput {
    fn none() -> Self {
        Self {
            action: SfxEditorAction::None,
            modified: false,
        }
    }

    fn action(action: SfxEditorAction) -> Self {
        Self {
            action,
            modified: false,
        }
    }

    fn modified() -> Self {
        Self {
            action: SfxEditorAction::None,
            modified: true,
        }
    }
}

pub struct SfxEditor {
    song: u8,
    channel: usize,
    frame: u16,
    scroll_frame: u16,
    header_focus: bool,
    header_field: HeaderField,
    param: usize,
    undo: VecDeque<SoundData>,
    redo: VecDeque<SoundData>,
    clipboard: Option<Event>,
    note_octave: u8,
    note_octave_pending: bool,
}

impl Default for SfxEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl SfxEditor {
    pub fn new() -> Self {
        Self {
            song: 0,
            channel: 0,
            frame: 0,
            scroll_frame: 0,
            header_focus: true,
            header_field: HeaderField::Sfx,
            param: 0,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            clipboard: None,
            note_octave: 4,
            note_octave_pending: false,
        }
    }

    pub fn update(&mut self, data: &mut SoundData) -> SfxEditorOutput {
        self.clamp_state(data);
        if is_key_pressed(KeyCode::Escape) {
            return SfxEditorOutput::action(SfxEditorAction::ExitToTerminal);
        }

        let modifier = is_shortcut_modifier_pressed();
        let shift = is_shift_pressed();

        if modifier && !shift && is_key_pressed(KeyCode::S) {
            return SfxEditorOutput::action(SfxEditorAction::Save);
        }
        if modifier
            && !shift
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
        {
            return SfxEditorOutput::action(SfxEditorAction::Run);
        }
        if modifier && !shift && is_key_pressed(KeyCode::Z) {
            return self.do_undo(data);
        }
        if (modifier && is_key_pressed(KeyCode::Y))
            || (modifier && shift && is_key_pressed(KeyCode::Z))
        {
            return self.do_redo(data);
        }

        if modifier && !shift && is_key_pressed(KeyCode::C) {
            if let Some(event) = self.current_event(data) {
                self.clipboard = Some(event);
            }
            return SfxEditorOutput::none();
        }
        if modifier && !shift && is_key_pressed(KeyCode::V) {
            if let Some(event) = self.clipboard {
                if self.current_event(data).is_some() {
                    self.push_undo(data);
                    self.set_current_event(data, event);
                    return SfxEditorOutput::modified();
                }
            }
            return SfxEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Tab) {
            self.note_octave_pending = false;
            if self.header_focus {
                if self.length(data) > 0 {
                    self.header_focus = false;
                    self.frame = self.frame.min(self.length(data) as u16 - 1);
                    self.ensure_cursor_visible(data);
                }
            } else {
                self.header_focus = true;
            }
            return SfxEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Space) {
            return SfxEditorOutput::action(SfxEditorAction::Preview {
                song: self.song,
                frame: if shift { self.frame } else { 0 },
                toggle: !shift,
            });
        }

        if self.header_focus {
            return self.update_header(data);
        }
        self.update_body(data)
    }

    fn update_header(&mut self, data: &mut SoundData) -> SfxEditorOutput {
        self.note_octave_pending = false;
        let shift = is_shift_pressed();
        if should_key_fire(KeyCode::Left, false) {
            self.header_field = self.header_field.prev();
            return SfxEditorOutput::none();
        }
        if should_key_fire(KeyCode::Right, false) {
            self.header_field = self.header_field.next();
            return SfxEditorOutput::none();
        }

        let up = should_key_fire(KeyCode::Up, false);
        let down = should_key_fire(KeyCode::Down, false);
        if !up && !down {
            return SfxEditorOutput::none();
        }
        let sign = if up { 1 } else { -1 };
        let step = if shift { 10 } else { 1 };
        let len_step = if shift { 8 } else { 1 };

        match self.header_field {
            HeaderField::Sfx => {
                self.song = next_song(self.song, sign * step);
                self.clamp_after_song_change(data);
                SfxEditorOutput::action(SfxEditorAction::Stop)
            }
            HeaderField::Speed => {
                let song = &mut data.songs[self.song as usize];
                let next = (song.preview_speed as i32 + sign * step).clamp(1, 255) as u8;
                if next == song.preview_speed {
                    return SfxEditorOutput::none();
                }
                self.push_undo(data);
                data.songs[self.song as usize].preview_speed = next;
                SfxEditorOutput::modified()
            }
            HeaderField::Len => {
                let length = self.length(data);
                let next = next_length(length, data.unused_frames(), len_step as usize, up);
                if next == length {
                    return SfxEditorOutput::none();
                }
                self.push_undo(data);
                data.songs[self.song as usize]
                    .frames
                    .resize(next, [Event::default(); MUSIC_CHANNELS]);
                self.frame = self.frame.min(next.saturating_sub(1) as u16);
                self.ensure_cursor_visible(data);
                SfxEditorOutput::modified()
            }
        }
    }

    fn update_body(&mut self, data: &mut SoundData) -> SfxEditorOutput {
        let shift = is_shift_pressed();
        let arrow_mod = is_arrow_modifier_pressed();
        let length = self.length(data);
        if length == 0 {
            self.header_focus = true;
            return SfxEditorOutput::none();
        }

        if should_key_fire(KeyCode::Left, false) && !arrow_mod {
            let step = if shift { 8 } else { 1 };
            self.frame = (self.frame as i32 - step).clamp(0, length as i32 - 1) as u16;
            self.ensure_cursor_visible(data);
            self.note_octave_pending = false;
        }
        if should_key_fire(KeyCode::Right, false) && !arrow_mod {
            let step = if shift { 8 } else { 1 };
            self.frame = (self.frame as i32 + step).clamp(0, length as i32 - 1) as u16;
            self.ensure_cursor_visible(data);
            self.note_octave_pending = false;
        }
        if should_key_fire(KeyCode::Up, false) && !arrow_mod {
            self.channel = (self.channel + MUSIC_CHANNELS - 1) % MUSIC_CHANNELS;
            self.param = self.param.min(self.param_count() - 1);
        }
        if should_key_fire(KeyCode::Down, false) && !arrow_mod {
            self.channel = (self.channel + 1) % MUSIC_CHANNELS;
            self.param = self.param.min(self.param_count() - 1);
        }

        if arrow_mod {
            let up = should_key_fire(KeyCode::Up, false);
            let down = should_key_fire(KeyCode::Down, false);
            let left = should_key_fire(KeyCode::Left, false);
            let right = should_key_fire(KeyCode::Right, false);
            if up || down {
                let sign = if up { -1 } else { 1 };
                self.param = move_param(self.param, sign, self.param_count());
                return SfxEditorOutput::none();
            }
            if left || right {
                let sign = if right { 1 } else { -1 };
                let coarse = shift;
                return self.bump_param(data, sign, coarse);
            }
        }

        if self.channel == 3 {
            if pressed_noise_gate_key() {
                let mut event = self.current_or_default_event(data);
                event.param = 1;
                self.push_undo(data);
                self.set_current_event(data, event);
                return SfxEditorOutput::modified();
            }
        } else {
            if let Some(semitone) = pressed_note_letter(shift) {
                let note = note_number(self.note_octave, semitone);
                let mut event = match self.current_event(data) {
                    Some(event) if is_off_event(self.channel, event) => self.default_event(),
                    _ => self.current_or_default_event(data),
                };
                event.param = note;
                self.push_undo(data);
                self.set_current_event(data, event);
                self.note_octave_pending = true;
                return SfxEditorOutput::modified();
            }
            if let Some(octave) = pressed_octave_digit() {
                self.note_octave = octave;
                if let Some(mut event) = self.current_event(data) {
                    if (NOTE_MIN..=NOTE_MAX).contains(&event.param) {
                        let semitone = (event.param - NOTE_MIN) % 12;
                        let note = note_number(octave, semitone);
                        if note != event.param {
                            if !self.note_octave_pending {
                                self.push_undo(data);
                            }
                            event.param = note;
                            self.set_current_event(data, event);
                            self.note_octave_pending = true;
                            return SfxEditorOutput::modified();
                        }
                    }
                }
                return SfxEditorOutput::none();
            }
        }

        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            self.note_octave_pending = false;
            let current = self.current_event(data).unwrap_or_default();
            let active_before = self.is_active_before(data, self.channel, self.frame);
            if let Some(event) = delete_replacement(self.channel, current, active_before) {
                self.push_undo(data);
                self.set_current_event(data, event);
                return SfxEditorOutput::modified();
            }
        }

        SfxEditorOutput::none()
    }

    fn bump_param(&mut self, data: &mut SoundData, sign: i32, coarse: bool) -> SfxEditorOutput {
        let current = self.current_event(data).unwrap_or_default();
        let active = self.is_active_at(data, self.channel, self.frame);
        if (current.is_empty() && !active)
            || (!current.is_empty() && is_off_event(self.channel, current))
        {
            return SfxEditorOutput::none();
        }
        let mut event = self.current_or_default_event(data);
        let step = if coarse { 4 } else { 1 };
        match self.channel {
            0 => match self.param {
                0 => {
                    let next = (event.volume() as i32 + sign * step).clamp(0, 15) as u8;
                    event.set_volume(next);
                }
                1 => {
                    let next = (event.timbre() as i32 + sign * step).rem_euclid(4) as u8;
                    event.set_timbre(next);
                }
                2 => {
                    let pace = (event.extra & 0x07) as i32;
                    let next = (pace + sign * step).rem_euclid(8) as u8;
                    event.extra = (event.extra & !0x07) | next;
                }
                3 => {
                    event.extra ^= 0x08;
                }
                _ => {
                    let step_bits = ((event.extra >> 4) & 0x07) as i32;
                    let next = (step_bits + sign * step).rem_euclid(8) as u8;
                    event.extra = (event.extra & 0x8F) | (next << 4);
                }
            },
            1 => match self.param {
                0 => {
                    let next = (event.volume() as i32 + sign * step).clamp(0, 15) as u8;
                    event.set_volume(next);
                }
                _ => {
                    let next = (event.timbre() as i32 + sign * step).rem_euclid(4) as u8;
                    event.set_timbre(next);
                }
            },
            2 => match self.param {
                0 => {
                    let next = (event.volume() as i32 + sign * step).clamp(0, 15) as u8;
                    event.set_volume(next);
                }
                _ => {
                    let current = if event.extra == 0xFF {
                        16
                    } else {
                        (event.extra & 0x0F) as i32
                    };
                    let next = (current + sign * step).rem_euclid(17);
                    event.extra = if next == 16 { 0xFF } else { next as u8 };
                }
            },
            _ => match self.param {
                0 => {
                    let next = (event.volume() as i32 + sign * step).clamp(0, 15) as u8;
                    event.set_volume(next);
                }
                1 => {
                    let divider = (event.extra & 0x07) as i32;
                    let next = (divider + sign * step).rem_euclid(8) as u8;
                    event.extra = (event.extra & !0x07) | next;
                }
                2 => {
                    let shift_bits = ((event.extra >> 3) & 0x0F) as i32;
                    let next = (shift_bits + sign * step).rem_euclid(16) as u8;
                    event.extra = (event.extra & !0x78) | (next << 3);
                }
                _ => {
                    event.extra ^= 0x80;
                }
            },
        }
        self.push_undo(data);
        self.set_current_event(data, event);
        SfxEditorOutput::modified()
    }

    fn param_count(&self) -> usize {
        match self.channel {
            0 => 5,
            1 => 2,
            2 => 2,
            _ => 4,
        }
    }

    fn length(&self, data: &SoundData) -> usize {
        data.songs
            .get(self.song as usize)
            .map(|song| song.frames.len())
            .unwrap_or(0)
    }

    fn current_event(&self, data: &SoundData) -> Option<Event> {
        let song = data.songs.get(self.song as usize)?;
        let frame = song.frames.get(self.frame as usize)?;
        Some(frame[self.channel])
    }

    fn set_current_event(&mut self, data: &mut SoundData, event: Event) {
        if let Some(song) = data.songs.get_mut(self.song as usize) {
            if let Some(frame) = song.frames.get_mut(self.frame as usize) {
                frame[self.channel] = event;
            }
        }
    }

    fn current_or_default_event(&self, data: &SoundData) -> Event {
        let current = self.current_event(data).unwrap_or_default();
        if !current.is_empty() {
            return current;
        }
        let previous = self.previous_param_event(data, self.channel, self.frame);
        let mut event = previous.unwrap_or_else(|| self.default_event());
        event.param = 0;
        event
    }

    fn default_event(&self) -> Event {
        let mut event = Event::default();
        event.set_volume(15);
        if self.channel <= 1 {
            event.set_timbre(2);
        }
        event
    }

    fn effective_event(&self, data: &SoundData, channel: usize, frame: u16) -> Event {
        let song = match data.songs.get(self.song as usize) {
            Some(song) => song,
            None => return Event::default(),
        };
        if let Some(cell) = song.frames.get(frame as usize) {
            let event = cell[channel];
            if !event.is_empty() {
                return event;
            }
        }
        self.previous_param_event(data, channel, frame)
            .unwrap_or_else(|| self.default_event())
    }

    fn previous_param_event(&self, data: &SoundData, channel: usize, frame: u16) -> Option<Event> {
        let song = data.songs.get(self.song as usize)?;
        for index in (0..frame as usize).rev() {
            let event = song.frames.get(index)?.get(channel)?;
            if event.is_empty() || is_off_event(channel, *event) {
                continue;
            }
            return Some(*event);
        }
        None
    }

    fn is_active_before(&self, data: &SoundData, channel: usize, frame: u16) -> bool {
        let Some(song) = data.songs.get(self.song as usize) else {
            return false;
        };
        let mut active = false;
        for index in 0..frame as usize {
            let Some(cell) = song.frames.get(index) else {
                break;
            };
            let event = cell[channel];
            if is_start_event(channel, event) {
                active = true;
            } else if is_off_event(channel, event) {
                active = false;
            }
        }
        active
    }

    fn is_active_at(&self, data: &SoundData, channel: usize, frame: u16) -> bool {
        let mut active = self.is_active_before(data, channel, frame);
        if let Some(event) = self.current_event_at(data, channel, frame) {
            if is_start_event(channel, event) {
                active = true;
            } else if is_off_event(channel, event) {
                active = false;
            }
        }
        active
    }

    fn display_cell(&self, data: &SoundData, channel: usize, frame: u16) -> String {
        let event = self
            .current_event_at(data, channel, frame)
            .unwrap_or_default();
        let active = self.is_active_at(data, channel, frame);
        if event.is_empty() {
            return if active { "---" } else { "..." }.to_string();
        }
        match channel {
            0..=2 => {
                if (NOTE_MIN..=NOTE_MAX).contains(&event.param) {
                    return note_parts(event.param)
                        .map(|(name, octave)| format!("{name}{octave}"))
                        .unwrap_or_else(|| "...".to_string());
                }
                if event.param == NOTE_OFF {
                    return "...".to_string();
                }
                if active { "---" } else { "..." }.to_string()
            }
            _ => {
                if event.param == 2 {
                    "...".to_string()
                } else if event.param == 1 || active {
                    "---".to_string()
                } else {
                    "...".to_string()
                }
            }
        }
    }

    fn current_event_at(&self, data: &SoundData, channel: usize, frame: u16) -> Option<Event> {
        let song = data.songs.get(self.song as usize)?;
        song.frames.get(frame as usize).map(|cell| cell[channel])
    }

    fn has_param_change(&self, data: &SoundData, channel: usize, frame: u16) -> bool {
        let Some(event) = self.current_event_at(data, channel, frame) else {
            return false;
        };
        if event.is_empty() || is_off_event(channel, event) {
            return false;
        }
        let Some(previous) = self.previous_param_event(data, channel, frame) else {
            return false;
        };
        event.volume_timbre != previous.volume_timbre || event.extra != previous.extra
    }

    fn ensure_cursor_visible(&mut self, data: &SoundData) {
        let length = self.length(data);
        if length <= VISIBLE_FRAMES {
            self.scroll_frame = 0;
            return;
        }
        let max_scroll = (length - VISIBLE_FRAMES) as u16;
        if self.frame < self.scroll_frame {
            self.scroll_frame = self.frame;
        } else if self.frame >= self.scroll_frame + VISIBLE_FRAMES as u16 {
            self.scroll_frame = self.frame + 1 - VISIBLE_FRAMES as u16;
        }
        self.scroll_frame = self.scroll_frame.min(max_scroll);
    }

    fn clamp_state(&mut self, data: &SoundData) {
        self.song = self.song.min((MAX_SONG_COUNT - 1) as u8);
        let length = self.length(data);
        if length == 0 {
            self.frame = 0;
            self.scroll_frame = 0;
        } else {
            self.frame = self.frame.min(length as u16 - 1);
            self.ensure_cursor_visible(data);
        }
        self.param = self.param.min(self.param_count() - 1);
    }

    fn clamp_after_song_change(&mut self, data: &SoundData) {
        self.scroll_frame = 0;
        self.clamp_state(data);
    }

    fn push_undo(&mut self, data: &SoundData) {
        self.undo.push_back(data.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    fn do_undo(&mut self, data: &mut SoundData) -> SfxEditorOutput {
        if let Some(snapshot) = self.undo.pop_back() {
            self.redo.push_back(data.clone());
            *data = snapshot;
            self.clamp_after_song_change(data);
            return SfxEditorOutput::modified();
        }
        SfxEditorOutput::none()
    }

    fn do_redo(&mut self, data: &mut SoundData) -> SfxEditorOutput {
        if let Some(snapshot) = self.redo.pop_back() {
            self.undo.push_back(data.clone());
            *data = snapshot;
            self.clamp_after_song_change(data);
            return SfxEditorOutput::modified();
        }
        SfxEditorOutput::none()
    }

    pub fn draw(&self, data: &SoundData, helpers: &DrawHelpers) {
        self.draw_header(data, helpers);
        self.draw_body(data, helpers);
        self.draw_info(data, helpers);
    }

    fn draw_header(&self, data: &SoundData, helpers: &DrawHelpers) {
        let song = &data.songs[self.song as usize];
        let free = data.unused_frames().min(999);
        let sfx = format!("SFX {:03}", self.song);
        let spd = format!("SPD {:03}", song.preview_speed);
        let len = format!("LEN {:03}", song.frames.len());
        let frm = format!("FRM {:03}", self.frame);
        let fre = format!("FREE {:03}", free);
        let tw = TILE_WIDTH as f32;
        let segments: [(&str, f32, bool, HeaderField); 5] = [
            (&sfx, 0.0, true, HeaderField::Sfx),
            (&spd, 8.0 * tw, true, HeaderField::Speed),
            (&len, 16.0 * tw, true, HeaderField::Len),
            (&frm, 24.0 * tw, false, HeaderField::Sfx),
            (&fre, 32.0 * tw, false, HeaderField::Sfx),
        ];
        for (text, x, editable, field) in segments {
            let color = if editable {
                COLOR_WHITE
            } else {
                COLOR_LIGHT_GRAY
            };
            draw_text(helpers, x, HEADER_Y, text, color);
            if self.header_focus && editable && field == self.header_field {
                helpers.draw_rect(
                    x,
                    HEADER_Y + 5.0,
                    text.chars().count() as f32 * tw,
                    1.0,
                    COLOR_WHITE,
                );
            }
        }
    }

    fn draw_body(&self, data: &SoundData, helpers: &DrawHelpers) {
        let length = self.length(data);
        let tw = TILE_WIDTH as f32;
        for (channel, name) in CH_NAMES.iter().enumerate() {
            let y = ROWS_Y + channel as f32 * ROW_HEIGHT;
            let name_color = if channel == self.channel {
                COLOR_WHITE
            } else {
                COLOR_LIGHT_GRAY
            };
            draw_text(helpers, 0.0, y, name, name_color);
            for visible in 0..VISIBLE_FRAMES {
                let frame = self.scroll_frame as usize + visible;
                if frame >= length {
                    break;
                }
                let x = CELLS_X + visible as f32 * CELL_WIDTH;
                let color = if channel == self.channel {
                    if frame == self.frame as usize {
                        COLOR_WHITE
                    } else {
                        COLOR_LIGHT_GRAY
                    }
                } else {
                    COLOR_DARK_GRAY
                };
                let text = self.display_cell(data, channel, frame as u16);
                for (index, c) in text.chars().enumerate() {
                    helpers.draw_char(c, x + index as f32 * tw, y, color);
                }
                if self.has_param_change(data, channel, frame as u16) {
                    helpers.draw_rect(x, y, 1.0, 1.0, COLOR_WHITE);
                }
            }
        }

        if length > VISIBLE_FRAMES {
            let track_w = VISIBLE_FRAMES as f32 * CELL_WIDTH;
            let thumb_w = (track_w * VISIBLE_FRAMES as f32 / length as f32)
                .round()
                .max(1.0);
            let max_scroll = (length - VISIBLE_FRAMES) as f32;
            let thumb_x = if max_scroll > 0.0 {
                (track_w - thumb_w) * self.scroll_frame as f32 / max_scroll
            } else {
                0.0
            };
            helpers.draw_rect(CELLS_X, SCROLLBAR_Y, track_w, 1.0, COLOR_DARK_GRAY);
            helpers.draw_rect(
                CELLS_X + thumb_x.round(),
                SCROLLBAR_Y,
                thumb_w,
                1.0,
                COLOR_WHITE,
            );
        }
    }

    fn draw_info(&self, data: &SoundData, helpers: &DrawHelpers) {
        if self.length(data) == 0 {
            return;
        }
        let event = self.current_event(data).unwrap_or_default();
        let active = self.is_active_at(data, self.channel, self.frame);
        if !event.is_empty() && is_off_event(self.channel, event) {
            return;
        }
        if event.is_empty() && !active {
            return;
        }
        let effective = self.effective_event(data, self.channel, self.frame);
        let tw = TILE_WIDTH as f32;
        for index in 0..self.param_count() {
            let color = if index == self.param {
                COLOR_WHITE
            } else {
                COLOR_LIGHT_GRAY
            };
            let label = self.param_label(index);
            let value = self.param_value(&effective, index);
            draw_text(helpers, 0.0, INFO_Y + index as f32 * 6.0, label, color);
            draw_text(
                helpers,
                12.0 * tw,
                INFO_Y + index as f32 * 6.0,
                &value,
                color,
            );
        }
    }

    fn param_label(&self, index: usize) -> &'static str {
        match self.channel {
            0 => match index {
                0 => "Volume",
                1 => "Duty",
                2 => "Sweep Pace",
                3 => "Sweep Dir",
                _ => "Sweep Step",
            },
            1 => match index {
                0 => "Volume",
                _ => "Duty",
            },
            2 => match index {
                0 => "Volume",
                _ => "Wave",
            },
            _ => match index {
                0 => "Volume",
                1 => "Divider",
                2 => "Shift",
                _ => "Width",
            },
        }
    }

    fn param_value(&self, event: &Event, index: usize) -> String {
        match self.channel {
            0 => match index {
                0 => format!("{:02}", event.volume() as u16 + 1),
                1 => format!("{}", event.timbre() & 0x03),
                2 => format!("{}", event.extra & 0x07),
                3 => {
                    if event.extra & 0x08 != 0 {
                        "-".to_string()
                    } else {
                        "+".to_string()
                    }
                }
                _ => format!("{}", (event.extra >> 4) & 0x07),
            },
            1 => match index {
                0 => format!("{:02}", event.volume() as u16 + 1),
                _ => format!("{}", event.timbre() & 0x03),
            },
            2 => match index {
                0 => format!("{:02}", event.volume() as u16 + 1),
                _ => {
                    if event.extra == 0xFF {
                        "LIVE".to_string()
                    } else {
                        format!("{:02}", event.extra & 0x0F)
                    }
                }
            },
            _ => match index {
                0 => format!("{:02}", event.volume() as u16 + 1),
                1 => format!("{}", event.extra & 0x07),
                2 => format!("{:02X}", (event.extra >> 3) & 0x0F),
                _ => {
                    if event.extra & 0x80 != 0 {
                        "7-bit".to_string()
                    } else {
                        "15-bit".to_string()
                    }
                }
            },
        }
    }
}

fn rest_event(channel: usize) -> Event {
    Event {
        param: if channel == 3 { 2 } else { NOTE_OFF },
        ..Event::default()
    }
}

fn delete_replacement(channel: usize, current: Event, active_before: bool) -> Option<Event> {
    if !current.is_empty() && is_off_event(channel, current) {
        return Some(Event::default());
    }
    if active_before {
        return Some(rest_event(channel));
    }
    if current.is_empty() {
        None
    } else {
        Some(Event::default())
    }
}

fn move_param(param: usize, delta: i32, count: usize) -> usize {
    (param as i32 + delta).rem_euclid(count as i32) as usize
}

fn next_song(song: u8, delta: i32) -> u8 {
    (song as i32 + delta).rem_euclid(MAX_SONG_COUNT as i32) as u8
}

fn next_length(length: usize, unused: usize, step: usize, up: bool) -> usize {
    if up {
        (length + step).min(length + unused).min(255)
    } else {
        length.saturating_sub(step)
    }
}

fn is_start_event(channel: usize, event: Event) -> bool {
    if channel == 3 {
        event.param == 1
    } else {
        (NOTE_MIN..=NOTE_MAX).contains(&event.param)
    }
}

fn is_off_event(channel: usize, event: Event) -> bool {
    if channel == 3 {
        event.param == 2
    } else {
        event.param == NOTE_OFF
    }
}

fn pressed_noise_gate_key() -> bool {
    const KEYS: [KeyCode; 7] = [
        KeyCode::A,
        KeyCode::B,
        KeyCode::C,
        KeyCode::D,
        KeyCode::E,
        KeyCode::F,
        KeyCode::G,
    ];
    KEYS.iter().any(|&key| is_key_pressed(key))
}

fn pressed_note_letter(sharp: bool) -> Option<u8> {
    const KEYS: [(KeyCode, u8); 7] = [
        (KeyCode::C, 0),
        (KeyCode::D, 2),
        (KeyCode::E, 4),
        (KeyCode::F, 5),
        (KeyCode::G, 7),
        (KeyCode::A, 9),
        (KeyCode::B, 11),
    ];
    for (key, semitone) in KEYS {
        if is_key_pressed(key) {
            return Some(semitone + sharp as u8);
        }
    }
    None
}

fn pressed_octave_digit() -> Option<u8> {
    const KEYS: [(KeyCode, u8); 12] = [
        (KeyCode::Key2, 2),
        (KeyCode::Key3, 3),
        (KeyCode::Key4, 4),
        (KeyCode::Key5, 5),
        (KeyCode::Key6, 6),
        (KeyCode::Key7, 7),
        (KeyCode::Kp2, 2),
        (KeyCode::Kp3, 3),
        (KeyCode::Kp4, 4),
        (KeyCode::Kp5, 5),
        (KeyCode::Kp6, 6),
        (KeyCode::Kp7, 7),
    ];
    for (key, octave) in KEYS {
        if is_key_pressed(key) {
            return Some(octave);
        }
    }
    None
}

fn note_number(octave: u8, semitone: u8) -> u8 {
    let note = (octave as i32 - 2) * 12 + semitone as i32 + NOTE_MIN as i32;
    note.clamp(NOTE_MIN as i32, NOTE_MAX as i32) as u8
}

fn draw_text(helpers: &DrawHelpers, x: f32, y: f32, text: &str, color: Color) {
    for (index, c) in text.chars().enumerate() {
        helpers.draw_char(c, x + index as f32 * TILE_WIDTH as f32, y, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song_with_note_and_off() -> SoundData {
        let mut data = SoundData::default();
        data.songs[0].frames = vec![[Event::default(); MUSIC_CHANNELS]; 4];
        let mut note = Event {
            param: 34,
            ..Event::default()
        };
        note.set_volume(15);
        note.set_timbre(2);
        data.songs[0].frames[0][0] = note;
        data.songs[0].frames[3][0] = Event {
            param: NOTE_OFF,
            ..Event::default()
        };
        data
    }

    #[test]
    fn sustain_bar_and_rest_are_derived() {
        let data = song_with_note_and_off();
        let editor = SfxEditor::new();
        assert_eq!(editor.display_cell(&data, 0, 0), "A-4");
        assert_eq!(editor.display_cell(&data, 0, 1), "---");
        assert_eq!(editor.display_cell(&data, 0, 3), "...");
        assert!(editor.is_active_at(&data, 0, 2));
        assert!(!editor.is_active_at(&data, 0, 3));
    }

    #[test]
    fn parameter_marker_detects_changed_volume() {
        let mut data = song_with_note_and_off();
        let mut param = Event::default();
        param.set_volume(8);
        param.set_timbre(2);
        data.songs[0].frames[1][0] = param;
        let editor = SfxEditor::new();
        assert!(editor.has_param_change(&data, 0, 1));
        assert_eq!(editor.display_cell(&data, 0, 1), "---");
    }

    #[test]
    fn delete_toggles_rest_and_empty_only_while_active() {
        let note = Event {
            param: 34,
            ..Event::default()
        };
        assert_eq!(delete_replacement(0, note, true), Some(rest_event(0)));
        assert_eq!(
            delete_replacement(0, rest_event(0), true),
            Some(Event::default())
        );
        assert_eq!(
            delete_replacement(0, Event::default(), true),
            Some(rest_event(0))
        );
        assert_eq!(delete_replacement(0, Event::default(), false), None);
        assert_eq!(delete_replacement(0, note, false), Some(Event::default()));
    }

    #[test]
    fn active_before_does_not_include_current_cell() {
        let data = song_with_note_and_off();
        let editor = SfxEditor::new();
        assert!(!editor.is_active_before(&data, 0, 0));
        assert!(editor.is_active_before(&data, 0, 1));
        assert!(editor.is_active_before(&data, 0, 3));
        assert!(!editor.is_active_before(&data, 0, 4));
    }

    #[test]
    fn noise_channel_has_no_gate_parameter() {
        let mut editor = SfxEditor::new();
        editor.channel = 3;
        assert_eq!(editor.param_count(), 4);
        assert_eq!(editor.param_label(0), "Volume");
        assert_eq!(editor.param_label(1), "Divider");
        assert_eq!(editor.param_label(2), "Shift");
        assert_eq!(editor.param_label(3), "Width");
    }

    #[test]
    fn parameter_cursor_moves_upward_for_up() {
        assert_eq!(move_param(0, -1, 5), 4);
        assert_eq!(move_param(4, 1, 5), 0);
    }

    #[test]
    fn song_coarse_step_wraps() {
        assert_eq!(next_song(0, 10), 10);
        assert_eq!(next_song(250, 10), 4);
        assert_eq!(next_song(4, -10), 250);
    }

    #[test]
    fn length_coarse_step_clamps_to_available_frames() {
        assert_eq!(next_length(0, 100, 8, true), 8);
        assert_eq!(next_length(0, 5, 8, true), 5);
        assert_eq!(next_length(250, 100, 8, true), 255);
        assert_eq!(next_length(100, 0, 1, true), 100);
        assert_eq!(next_length(5, 0, 8, false), 0);
    }

    #[test]
    fn clamp_state_keeps_cursor_in_range() {
        let data = song_with_note_and_off();
        let mut editor = SfxEditor::new();
        editor.frame = 10;
        editor.param = 99;
        editor.clamp_state(&data);
        assert_eq!(editor.frame, 3);
        assert_eq!(editor.param, editor.param_count() - 1);
    }
}
