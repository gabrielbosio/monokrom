use std::collections::VecDeque;

use macroquad::prelude::*;

use crate::audio::format::{ChannelType, Instrument, SfxEntry, SoundData, TableStep};
use crate::audio::notes::{note_parts, NOTE_MAX};
use crate::config::{
    AUDIO_INSTRUMENT_COUNT, AUDIO_SFX_COUNT, AUDIO_TABLE_COUNT, AUDIO_TABLE_STEPS,
    AUDIO_WAVE_COUNT, COLOR_BLACK, COLOR_DARK_GRAY, COLOR_LIGHT_GRAY, COLOR_WHITE, TILE_HEIGHT,
    TILE_WIDTH,
};
use crate::input::{
    is_arrow_modifier_pressed, is_shift_pressed, is_shortcut_modifier_pressed, should_key_fire,
};
use crate::render::DrawHelpers;

const UNDO_LIMIT: usize = 16;
const TAB_Y: f32 = 0.0;
const HEADER_Y: f32 = 6.0;
const CONTENT_Y: f32 = 14.0;
const INFO_Y: f32 = 116.0;
const CELL_PX: f32 = 5.0;
const BARS_TOP: f32 = 20.0;
const BARS_BOTTOM: f32 = 66.0;
const TABLE_SHAPE_Y: f32 = 72.0;
const TABLE_VOLUME_Y: f32 = 78.0;
const TABLE_DETUNE_Y: f32 = 84.0;
const TABLE_COMMAND_Y: f32 = 90.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundTab {
    Instruments,
    Tables,
    Waves,
    Sfx,
}

impl SoundTab {
    fn next(self) -> Self {
        match self {
            Self::Instruments => Self::Tables,
            Self::Tables => Self::Waves,
            Self::Waves => Self::Sfx,
            Self::Sfx => Self::Instruments,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Instruments => Self::Sfx,
            Self::Tables => Self::Instruments,
            Self::Waves => Self::Tables,
            Self::Sfx => Self::Waves,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Instruments => "INST",
            Self::Tables => "TABLE",
            Self::Waves => "WAVE",
            Self::Sfx => "SFX",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewRequest {
    Instrument(u8, u8),
    Table(u8),
    Wave(u8),
    Sfx(u8),
}

pub enum SoundEditorAction {
    None,
    ExitToMapEditor,
    ExitToMusicEditor,
    Save,
    Run,
    Preview(PreviewRequest),
    Stop,
}

pub struct SoundEditorOutput {
    pub action: SoundEditorAction,
    pub modified: bool,
}

impl SoundEditorOutput {
    fn none() -> Self {
        Self {
            action: SoundEditorAction::None,
            modified: false,
        }
    }
    fn action(action: SoundEditorAction) -> Self {
        Self {
            action,
            modified: false,
        }
    }
    fn modified() -> Self {
        Self {
            action: SoundEditorAction::None,
            modified: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstrumentField {
    Type,
    Volume,
    EnvDir,
    EnvPeriod,
    Duty,
    Length,
    LengthEnabled,
    SweepPace,
    SweepDir,
    SweepStep,
    WaveSlot,
    WaveVolume,
    NoiseWidth,
    NoiseDivider,
    NoiseShift,
    Table,
    Panning,
    Finetune,
}

const PULSE_FIELDS: &[InstrumentField] = &[
    InstrumentField::Type,
    InstrumentField::Volume,
    InstrumentField::EnvDir,
    InstrumentField::EnvPeriod,
    InstrumentField::Duty,
    InstrumentField::Length,
    InstrumentField::LengthEnabled,
    InstrumentField::SweepPace,
    InstrumentField::SweepDir,
    InstrumentField::SweepStep,
    InstrumentField::Table,
    InstrumentField::Panning,
    InstrumentField::Finetune,
];

const WAVE_FIELDS: &[InstrumentField] = &[
    InstrumentField::Type,
    InstrumentField::WaveSlot,
    InstrumentField::WaveVolume,
    InstrumentField::Length,
    InstrumentField::LengthEnabled,
    InstrumentField::Table,
    InstrumentField::Panning,
    InstrumentField::Finetune,
];

const NOISE_FIELDS: &[InstrumentField] = &[
    InstrumentField::Type,
    InstrumentField::Volume,
    InstrumentField::EnvDir,
    InstrumentField::EnvPeriod,
    InstrumentField::Length,
    InstrumentField::LengthEnabled,
    InstrumentField::NoiseWidth,
    InstrumentField::NoiseDivider,
    InstrumentField::NoiseShift,
    InstrumentField::Table,
    InstrumentField::Panning,
    InstrumentField::Finetune,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TableRow {
    Pitch,
    Shape,
    Volume,
    Detune,
    Command,
}

impl TableRow {
    fn next(self) -> Self {
        match self {
            Self::Pitch => Self::Shape,
            Self::Shape => Self::Volume,
            Self::Volume => Self::Detune,
            Self::Detune => Self::Command,
            Self::Command => Self::Pitch,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Pitch => Self::Command,
            Self::Shape => Self::Pitch,
            Self::Volume => Self::Shape,
            Self::Detune => Self::Volume,
            Self::Command => Self::Detune,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Pitch => "Pitch",
            Self::Shape => "Shape",
            Self::Volume => "Volume",
            Self::Detune => "Detune",
            Self::Command => "Command",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TableHeaderField {
    Table,
    Speed,
    Loop,
    LoopStart,
    LoopEnd,
}

impl TableHeaderField {
    fn next(self, loop_enabled: bool) -> Self {
        match self {
            Self::Table => Self::Speed,
            Self::Speed => Self::Loop,
            Self::Loop => {
                if loop_enabled {
                    Self::LoopStart
                } else {
                    Self::Table
                }
            }
            Self::LoopStart => Self::LoopEnd,
            Self::LoopEnd => Self::Table,
        }
    }
    fn prev(self, loop_enabled: bool) -> Self {
        match self {
            Self::Table => {
                if loop_enabled {
                    Self::LoopEnd
                } else {
                    Self::Loop
                }
            }
            Self::Speed => Self::Table,
            Self::Loop => Self::Speed,
            Self::LoopStart => Self::Loop,
            Self::LoopEnd => Self::LoopStart,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SfxField {
    Channel,
    Instrument,
    Table,
    Note,
    Priority,
    Duration,
}

impl SfxField {
    fn next(self) -> Self {
        match self {
            Self::Channel => Self::Instrument,
            Self::Instrument => Self::Table,
            Self::Table => Self::Note,
            Self::Note => Self::Priority,
            Self::Priority => Self::Duration,
            Self::Duration => Self::Channel,
        }
    }
    fn prev(self) -> Self {
        match self {
            Self::Channel => Self::Duration,
            Self::Instrument => Self::Channel,
            Self::Table => Self::Instrument,
            Self::Note => Self::Table,
            Self::Priority => Self::Note,
            Self::Duration => Self::Priority,
        }
    }
}

const SFX_COLUMNS: [(SfxField, &str, usize, usize); 6] = [
    (SfxField::Channel, "CHAN", 0, 2),
    (SfxField::Instrument, "INST", 5, 2),
    (SfxField::Table, "TBL", 10, 2),
    (SfxField::Note, "NOTE", 15, 3),
    (SfxField::Priority, "PRI", 20, 1),
    (SfxField::Duration, "DUR", 25, 3),
];
const SFX_ROWS_Y: f32 = CONTENT_Y + 6.0;

pub struct SoundEditor {
    tab: SoundTab,
    instrument: u8,
    instrument_field: usize,
    table: u8,
    table_header_focus: bool,
    table_header_field: TableHeaderField,
    table_step: u8,
    table_row: TableRow,
    wave: u8,
    wave_sample: u8,
    sfx: u8,
    sfx_field: SfxField,
    undo: VecDeque<SoundData>,
    redo: VecDeque<SoundData>,
    cell_clipboard: Option<TableStep>,
    entry_clipboard: Option<u16>,
}

impl Default for SoundEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundEditor {
    pub fn new() -> Self {
        Self {
            tab: SoundTab::Instruments,
            instrument: 1,
            instrument_field: 0,
            table: 1,
            table_header_focus: false,
            table_header_field: TableHeaderField::Table,
            table_step: 0,
            table_row: TableRow::Pitch,
            wave: 0,
            wave_sample: 0,
            sfx: 0,
            sfx_field: SfxField::Channel,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            cell_clipboard: None,
            entry_clipboard: None,
        }
    }

    pub fn update(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        if is_key_pressed(KeyCode::Escape) {
            return SoundEditorOutput::action(if is_shift_pressed() {
                SoundEditorAction::ExitToMapEditor
            } else {
                SoundEditorAction::ExitToMusicEditor
            });
        }

        let modifier = is_shortcut_modifier_pressed();
        let shift = is_shift_pressed();

        if modifier && !shift && is_key_pressed(KeyCode::S) {
            return SoundEditorOutput::action(SoundEditorAction::Save);
        }
        if modifier
            && !shift
            && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter))
        {
            return SoundEditorOutput::action(SoundEditorAction::Run);
        }
        if modifier && !shift && is_key_pressed(KeyCode::Z) {
            return self.do_undo(data);
        }
        if (modifier && is_key_pressed(KeyCode::Y))
            || (modifier && shift && is_key_pressed(KeyCode::Z))
        {
            return self.do_redo(data);
        }

        if matches!(
            self.tab,
            SoundTab::Instruments | SoundTab::Tables | SoundTab::Waves
        ) && is_arrow_modifier_pressed()
            && (is_key_down(KeyCode::Left) || is_key_down(KeyCode::Right))
        {
            if should_key_fire(KeyCode::Left, false) {
                self.change_container(-1);
            } else if should_key_fire(KeyCode::Right, false) {
                self.change_container(1);
            }
            return SoundEditorOutput::none();
        }

        if is_key_pressed(KeyCode::PageDown) {
            self.tab = self.tab.next();
            return SoundEditorOutput::none();
        }
        if is_key_pressed(KeyCode::PageUp) {
            self.tab = self.tab.prev();
            return SoundEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Tab) {
            self.tab = if shift {
                self.tab.prev()
            } else {
                self.tab.next()
            };
            return SoundEditorOutput::none();
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
            if matches!(self.tab, SoundTab::Tables) {
                self.table_header_focus = !self.table_header_focus;
            }
            return SoundEditorOutput::none();
        }

        if is_key_pressed(KeyCode::Space) {
            return SoundEditorOutput::action(if shift {
                SoundEditorAction::Stop
            } else {
                SoundEditorAction::Preview(self.preview_request())
            });
        }

        if modifier && shift && is_key_pressed(KeyCode::C) {
            self.copy_entry(data);
            return SoundEditorOutput::none();
        }
        if modifier && shift && is_key_pressed(KeyCode::V) {
            return self.paste_entry(data);
        }
        if modifier && !shift && is_key_pressed(KeyCode::C) {
            self.copy_cell(data);
            return SoundEditorOutput::none();
        }
        if modifier && !shift && is_key_pressed(KeyCode::V) {
            return self.paste_cell(data);
        }

        match self.tab {
            SoundTab::Instruments => self.update_instruments(data),
            SoundTab::Tables => self.update_tables(data),
            SoundTab::Waves => self.update_waves(data),
            SoundTab::Sfx => self.update_sfx(data),
        }
    }

    fn change_container(&mut self, direction: i32) {
        match self.tab {
            SoundTab::Instruments => {
                let count = AUDIO_INSTRUMENT_COUNT as i32 - 1;
                self.instrument =
                    ((self.instrument as i32 - 1 + direction).rem_euclid(count) + 1) as u8;
            }
            SoundTab::Tables => {
                self.table = wrap_table(self.table, direction);
            }
            SoundTab::Waves => {
                self.wave =
                    (self.wave as i32 + direction).rem_euclid(AUDIO_WAVE_COUNT as i32) as u8;
            }
            SoundTab::Sfx => {
                self.sfx = (self.sfx as i32 + direction).rem_euclid(AUDIO_SFX_COUNT as i32) as u8;
            }
        }
    }

    fn preview_request(&self) -> PreviewRequest {
        match self.tab {
            SoundTab::Instruments => PreviewRequest::Instrument(self.instrument, 34),
            SoundTab::Tables => PreviewRequest::Table(self.table),
            SoundTab::Waves => PreviewRequest::Wave(self.wave),
            SoundTab::Sfx => PreviewRequest::Sfx(self.sfx),
        }
    }

    fn fields_for(&self, data: &SoundData) -> &'static [InstrumentField] {
        match data.instruments[self.instrument as usize].channel {
            ChannelType::Pulse => PULSE_FIELDS,
            ChannelType::Wave => WAVE_FIELDS,
            ChannelType::Noise => NOISE_FIELDS,
        }
    }

    fn update_instruments(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        let shift = is_shift_pressed();
        let arrow_mod = is_arrow_modifier_pressed();
        let fields = self.fields_for(data);
        if self.instrument_field >= fields.len() {
            self.instrument_field = 0;
        }

        let up = should_key_fire(KeyCode::Up, false);
        let down = should_key_fire(KeyCode::Down, false);
        if arrow_mod {
            if up || down {
                let field = fields[self.instrument_field];
                let step = if shift {
                    instrument_coarse_step(field)
                } else {
                    1
                };
                self.push_undo(data);
                let sign = if up { 1 } else { -1 };
                let instrument = &mut data.instruments[self.instrument as usize];
                bump_instrument(instrument, field, sign * step);
                return SoundEditorOutput::modified();
            }
        } else {
            if up {
                self.instrument_field = (self.instrument_field + fields.len() - 1) % fields.len();
            }
            if down {
                self.instrument_field = (self.instrument_field + 1) % fields.len();
            }
        }

        let left = should_key_fire(KeyCode::Left, false);
        let right = should_key_fire(KeyCode::Right, false);
        if left || right {
            let field = fields[self.instrument_field];
            let step = if shift {
                instrument_coarse_step(field)
            } else {
                1
            };
            self.push_undo(data);
            let sign = if right { 1 } else { -1 };
            let instrument = &mut data.instruments[self.instrument as usize];
            bump_instrument(instrument, field, sign * step);
            return SoundEditorOutput::modified();
        }

        if is_key_pressed(KeyCode::Delete) {
            self.push_undo(data);
            data.instruments[self.instrument as usize] = Instrument::default();
            return SoundEditorOutput::modified();
        }
        SoundEditorOutput::none()
    }

    fn update_tables(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        if self.table_header_focus {
            return self.update_table_header(data);
        }

        let shift = is_shift_pressed();
        let arrow_mod = is_arrow_modifier_pressed();
        let left = should_key_fire(KeyCode::Left, false);
        let right = should_key_fire(KeyCode::Right, false);
        let up = should_key_fire(KeyCode::Up, false);
        let down = should_key_fire(KeyCode::Down, false);

        if arrow_mod {
            let bump = if up || down {
                if shift {
                    match self.table_row {
                        TableRow::Pitch => 12,
                        TableRow::Command => 2,
                        TableRow::Shape | TableRow::Volume | TableRow::Detune => 4,
                    }
                } else {
                    1
                }
            } else {
                0
            };
            if bump != 0 {
                self.push_undo(data);
                let sign = if up { 1 } else { -1 };
                let bump = sign * bump;
                let table = &mut data.tables[self.table as usize];
                let step = &mut table.steps[self.table_step as usize];
                match self.table_row {
                    TableRow::Pitch => {
                        step.pitch = (step.pitch as i32 + bump).clamp(-128, 127) as i8;
                    }
                    TableRow::Shape => step.shape = (step.shape as i32 + bump).rem_euclid(16) as u8,
                    TableRow::Volume => {
                        step.volume = (step.volume as i32 + bump).rem_euclid(16) as u8
                    }
                    TableRow::Detune => {
                        step.detune = (step.detune as i32 + bump).rem_euclid(16) as u8
                    }
                    TableRow::Command => {
                        step.command = (step.command as i32 + bump).rem_euclid(4) as u8
                    }
                }
                return SoundEditorOutput::modified();
            }
        } else {
            let move_step = if shift { 8 } else { 1 };
            if left {
                self.table_step = (self.table_step + AUDIO_TABLE_STEPS as u8 - move_step)
                    % AUDIO_TABLE_STEPS as u8;
            }
            if right {
                self.table_step = (self.table_step + move_step) % AUDIO_TABLE_STEPS as u8;
            }
            if up {
                self.table_row = self.table_row.prev();
            }
            if down {
                self.table_row = self.table_row.next();
            }
        }

        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            self.push_undo(data);
            data.tables[self.table as usize].steps[self.table_step as usize] = TableStep::default();
            return SoundEditorOutput::modified();
        }
        SoundEditorOutput::none()
    }

    fn update_table_header(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        let loop_enabled = data.tables[self.table as usize].loop_enabled;
        if should_key_fire(KeyCode::Left, false) {
            self.table_header_field = self.table_header_field.prev(loop_enabled);
            return SoundEditorOutput::none();
        }
        if should_key_fire(KeyCode::Right, false) {
            self.table_header_field = self.table_header_field.next(loop_enabled);
            return SoundEditorOutput::none();
        }
        let bump = if should_key_fire(KeyCode::Up, false) {
            if is_shift_pressed() {
                10
            } else {
                1
            }
        } else if should_key_fire(KeyCode::Down, false) {
            if is_shift_pressed() {
                -10
            } else {
                -1
            }
        } else {
            0
        };
        if bump == 0 {
            return SoundEditorOutput::none();
        }
        self.push_undo(data);
        let table = &mut data.tables[self.table as usize];
        match self.table_header_field {
            TableHeaderField::Table => {
                self.table = wrap_table(self.table, bump);
            }
            TableHeaderField::Speed => {
                table.speed = (table.speed as i32 + bump).clamp(1, 255) as u8;
            }
            TableHeaderField::Loop => {
                table.loop_enabled = !table.loop_enabled;
                if table.loop_enabled {
                    if table.loop_end == 0 || table.loop_end <= table.loop_start {
                        table.loop_start = 0;
                        table.loop_end = AUDIO_TABLE_STEPS as u8;
                    }
                } else {
                    self.table_header_field = TableHeaderField::Loop;
                }
            }
            TableHeaderField::LoopStart => {
                table.loop_start =
                    (table.loop_start as i32 + bump).clamp(0, AUDIO_TABLE_STEPS as i32 - 1) as u8;
                if table.loop_enabled && table.loop_end <= table.loop_start {
                    table.loop_end = (table.loop_start + 1).min(AUDIO_TABLE_STEPS as u8);
                }
            }
            TableHeaderField::LoopEnd => {
                table.loop_end =
                    (table.loop_end as i32 + bump).clamp(1, AUDIO_TABLE_STEPS as i32) as u8;
                if table.loop_enabled && table.loop_end <= table.loop_start {
                    table.loop_start = table.loop_end.saturating_sub(1);
                }
            }
        }
        SoundEditorOutput::modified()
    }

    fn update_waves(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        let shift = is_shift_pressed();
        let left = should_key_fire(KeyCode::Left, false);
        let right = should_key_fire(KeyCode::Right, false);
        let up = should_key_fire(KeyCode::Up, false);
        let down = should_key_fire(KeyCode::Down, false);

        if left || right {
            let sign = if right { 1 } else { -1 };
            let step = if shift { 8 } else { 1 };
            self.wave_sample = (self.wave_sample as i32 + sign * step).rem_euclid(32) as u8;
        }
        if up || down {
            self.push_undo(data);
            let sign = if up { 1 } else { -1 };
            let step = if shift { 4 } else { 1 };
            let wave = &mut data.waves[self.wave as usize];
            let byte = &mut wave[(self.wave_sample / 2) as usize];
            let shift = if self.wave_sample.is_multiple_of(2) {
                4
            } else {
                0
            };
            let current = ((*byte >> shift) & 0x0F) as i32;
            let next = (current + sign * step).clamp(0, 15) as u8;
            *byte = (*byte & !(0x0F << shift)) | (next << shift);
            return SoundEditorOutput::modified();
        }
        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            self.push_undo(data);
            data.waves[self.wave as usize] = [0; 16];
            return SoundEditorOutput::modified();
        }
        SoundEditorOutput::none()
    }

    fn update_sfx(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        let shift = is_shift_pressed();
        let arrow_mod = is_arrow_modifier_pressed();
        if should_key_fire(KeyCode::Left, false) {
            self.sfx_field = self.sfx_field.prev();
        }
        if should_key_fire(KeyCode::Right, false) {
            self.sfx_field = self.sfx_field.next();
        }

        let up = should_key_fire(KeyCode::Up, false);
        let down = should_key_fire(KeyCode::Down, false);
        if arrow_mod {
            let bump = if up || down {
                let step = if shift { 10 } else { 1 };
                if up {
                    step
                } else {
                    -step
                }
            } else {
                0
            };
            if bump != 0 {
                self.push_undo(data);
                let entry = &mut data.sfx[self.sfx as usize];
                match self.sfx_field {
                    SfxField::Channel => {
                        entry.channel = match (entry.channel as i32 + bump).rem_euclid(3) {
                            0 => ChannelType::Pulse,
                            1 => ChannelType::Wave,
                            _ => ChannelType::Noise,
                        };
                    }
                    SfxField::Instrument => {
                        if entry.instrument == 0 {
                            entry.instrument = if bump > 0 {
                                1
                            } else {
                                AUDIO_INSTRUMENT_COUNT as u8 - 1
                            };
                        } else {
                            let count = AUDIO_INSTRUMENT_COUNT as i32 - 1;
                            entry.instrument =
                                ((entry.instrument as i32 - 1 + bump).rem_euclid(count) + 1) as u8;
                        }
                    }
                    SfxField::Table => {
                        entry.table =
                            (entry.table as i32 + bump).rem_euclid(AUDIO_TABLE_COUNT as i32) as u8;
                    }
                    SfxField::Note => {
                        entry.note =
                            (entry.note as i32 + bump).rem_euclid(NOTE_MAX as i32 + 1) as u8;
                    }
                    SfxField::Priority => {
                        entry.priority = (entry.priority as i32 + bump).rem_euclid(4) as u8;
                    }
                    SfxField::Duration => {
                        entry.duration = (entry.duration as i32 + bump).rem_euclid(256) as u8;
                    }
                }
                return SoundEditorOutput::modified();
            }
        } else {
            if up {
                self.sfx = (self.sfx as i32 - 1).rem_euclid(AUDIO_SFX_COUNT as i32) as u8;
            }
            if down {
                self.sfx = (self.sfx as i32 + 1).rem_euclid(AUDIO_SFX_COUNT as i32) as u8;
            }
        }

        if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
            self.push_undo(data);
            data.sfx[self.sfx as usize] = SfxEntry::default();
            return SoundEditorOutput::modified();
        }
        SoundEditorOutput::none()
    }

    fn copy_entry(&mut self, data: &SoundData) {
        self.entry_clipboard = Some(match self.tab {
            SoundTab::Instruments => self.instrument as u16,
            SoundTab::Tables => self.table as u16 | 0x0100,
            SoundTab::Waves => self.wave as u16 | 0x0200,
            SoundTab::Sfx => self.sfx as u16 | 0x0300,
        });
        let _ = data;
    }

    fn paste_entry(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        let Some(clipboard) = self.entry_clipboard else {
            return SoundEditorOutput::none();
        };
        let source = clipboard & 0x00FF;
        let kind = clipboard >> 8;
        if !matches!(
            (self.tab, kind),
            (SoundTab::Instruments, 0)
                | (SoundTab::Tables, 1)
                | (SoundTab::Waves, 2)
                | (SoundTab::Sfx, 3)
        ) {
            return SoundEditorOutput::none();
        }
        self.push_undo(data);
        match (self.tab, kind) {
            (SoundTab::Instruments, _) => {
                data.instruments[self.instrument as usize] = data.instruments[source as usize];
            }
            (SoundTab::Tables, _) => {
                data.tables[self.table as usize] = data.tables[source as usize];
            }
            (SoundTab::Waves, _) => {
                data.waves[self.wave as usize] = data.waves[source as usize];
            }
            (SoundTab::Sfx, _) => {
                data.sfx[self.sfx as usize] = data.sfx[source as usize];
            }
        }
        SoundEditorOutput::modified()
    }

    fn copy_cell(&mut self, data: &SoundData) {
        if self.tab == SoundTab::Tables {
            self.cell_clipboard =
                Some(data.tables[self.table as usize].steps[self.table_step as usize]);
        }
    }

    fn paste_cell(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        if self.tab == SoundTab::Tables {
            if let Some(step) = self.cell_clipboard {
                self.push_undo(data);
                data.tables[self.table as usize].steps[self.table_step as usize] = step;
                return SoundEditorOutput::modified();
            }
        }
        SoundEditorOutput::none()
    }

    fn push_undo(&mut self, data: &SoundData) {
        self.undo.push_back(data.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    fn do_undo(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        if let Some(snapshot) = self.undo.pop_back() {
            self.redo.push_back(data.clone());
            *data = snapshot;
            return SoundEditorOutput::modified();
        }
        SoundEditorOutput::none()
    }

    fn do_redo(&mut self, data: &mut SoundData) -> SoundEditorOutput {
        if let Some(snapshot) = self.redo.pop_back() {
            self.undo.push_back(data.clone());
            *data = snapshot;
            return SoundEditorOutput::modified();
        }
        SoundEditorOutput::none()
    }

    pub fn draw(&self, data: &SoundData, helpers: &DrawHelpers) {
        self.draw_tabs(helpers);
        match self.tab {
            SoundTab::Instruments => self.draw_instruments(data, helpers),
            SoundTab::Tables => self.draw_tables(data, helpers),
            SoundTab::Waves => self.draw_waves(data, helpers),
            SoundTab::Sfx => self.draw_sfx(data, helpers),
        }
    }

    fn draw_tabs(&self, helpers: &DrawHelpers) {
        let mut x = 0.0;
        for tab in [
            SoundTab::Instruments,
            SoundTab::Tables,
            SoundTab::Waves,
            SoundTab::Sfx,
        ] {
            let color = if tab == self.tab {
                COLOR_WHITE
            } else {
                COLOR_DARK_GRAY
            };
            draw_text(helpers, x, TAB_Y, tab.label(), color);
            x += (tab.label().len() as f32 + 2.0) * TILE_WIDTH as f32;
        }
    }

    fn draw_instruments(&self, data: &SoundData, helpers: &DrawHelpers) {
        let instrument = data.instruments[self.instrument as usize];
        let fields = self.fields_for(data);
        draw_text(
            helpers,
            0.0,
            HEADER_Y,
            &format!("INS:{:02}", self.instrument),
            COLOR_WHITE,
        );
        for (index, field) in fields.iter().enumerate() {
            let y = CONTENT_Y + index as f32 * (TILE_HEIGHT as f32 + 1.0);
            let selected = index == self.instrument_field;
            let label_color = if selected {
                COLOR_WHITE
            } else {
                COLOR_LIGHT_GRAY
            };
            draw_text(helpers, 0.0, y, field.label(), label_color);
            let value = field_value(instrument, *field);
            let value_x = 12.0 * TILE_WIDTH as f32;
            if selected {
                let width = value.chars().count() as f32 * TILE_WIDTH as f32;
                helpers.draw_rect(value_x, y, width, TILE_HEIGHT as f32, COLOR_WHITE);
                draw_text(helpers, value_x, y, &value, COLOR_BLACK);
            } else {
                draw_text(helpers, value_x, y, &value, COLOR_LIGHT_GRAY);
            }
        }
    }

    fn draw_tables(&self, data: &SoundData, helpers: &DrawHelpers) {
        let table = &data.tables[self.table as usize];
        let header = if table.loop_enabled {
            format!(
                "TBL:{:02} SPD:{:03} L:ON {:02}-{:02}",
                self.table, table.speed, table.loop_start, table.loop_end
            )
        } else {
            format!("TBL:{:02} SPD:{:03} L:OFF", self.table, table.speed)
        };
        draw_text(helpers, 0.0, HEADER_Y, &header, COLOR_WHITE);
        if self.table_header_focus {
            let (selected, width) = match self.table_header_field {
                TableHeaderField::Table => (4, 2),
                TableHeaderField::Speed => (11, 3),
                TableHeaderField::Loop => (17, if table.loop_enabled { 2 } else { 3 }),
                TableHeaderField::LoopStart => (20, 2),
                TableHeaderField::LoopEnd => (23, 2),
            };
            helpers.draw_rect(
                selected as f32 * TILE_WIDTH as f32,
                HEADER_Y + 6.0,
                width as f32 * TILE_WIDTH as f32,
                1.0,
                COLOR_WHITE,
            );
        }

        let height = BARS_BOTTOM - BARS_TOP;
        helpers.draw_rect(0.0, BARS_TOP, 160.0, 1.0, COLOR_DARK_GRAY);
        helpers.draw_rect(0.0, BARS_BOTTOM, 160.0, 1.0, COLOR_DARK_GRAY);
        for step_index in 0..AUDIO_TABLE_STEPS {
            let step = table.steps[step_index];
            let selected_column = step_index as u8 == self.table_step;
            let column_color = if selected_column {
                COLOR_WHITE
            } else {
                COLOR_LIGHT_GRAY
            };
            let x = step_index as f32 * CELL_PX + 1.0;
            let pitch_height = ((step.pitch as i32 + 128) as f32 / 255.0 * height).max(1.0);
            if selected_column && self.table_row == TableRow::Pitch {
                helpers.draw_rect(
                    step_index as f32 * CELL_PX,
                    BARS_TOP,
                    CELL_PX,
                    height,
                    COLOR_WHITE,
                );
                helpers.draw_rect(
                    x,
                    BARS_BOTTOM - pitch_height,
                    CELL_PX - 2.0,
                    pitch_height,
                    COLOR_BLACK,
                );
            } else {
                helpers.draw_rect(
                    x,
                    BARS_BOTTOM - pitch_height,
                    CELL_PX - 2.0,
                    pitch_height,
                    column_color,
                );
            }
            let shape = char::from_digit((step.shape & 0x0F) as u32, 16).unwrap_or('?');
            let volume = char::from_digit((step.volume & 0x0F) as u32, 16).unwrap_or('?');
            let detune = char::from_digit((step.detune & 0x0F) as u32, 16).unwrap_or('?');
            let command = table_command_char(step.command);
            for (row, y, c) in [
                (TableRow::Shape, TABLE_SHAPE_Y, shape),
                (TableRow::Volume, TABLE_VOLUME_Y, volume),
                (TableRow::Detune, TABLE_DETUNE_Y, detune),
                (TableRow::Command, TABLE_COMMAND_Y, command),
            ] {
                if selected_column && row == self.table_row {
                    helpers.draw_rect(
                        step_index as f32 * CELL_PX,
                        y,
                        CELL_PX,
                        TILE_HEIGHT as f32,
                        COLOR_WHITE,
                    );
                    helpers.draw_char(c, x, y, COLOR_BLACK);
                } else {
                    helpers.draw_char(c, x, y, column_color);
                }
            }
        }

        let step = table.steps[self.table_step as usize];
        let label = self.table_row.label();
        let text = match self.table_row {
            TableRow::Pitch => {
                let sign = if step.pitch < 0 { '-' } else { ' ' };
                format!("{label:<7}{sign}{:03}", (step.pitch as i32).abs())
            }
            TableRow::Shape => format!("{label:<8}{:>3}", format!("{:02}", step.shape)),
            TableRow::Volume => format!("{label:<8}{:>3}", format!("{:02}", step.volume)),
            TableRow::Detune => format!("{label:<8}{:>3}", format!("{:02}", step.detune)),
            TableRow::Command => format!("{label} {}", table_command_name(step.command)),
        };
        draw_text(helpers, 0.0, INFO_Y, &text, COLOR_WHITE);
    }

    fn draw_waves(&self, data: &SoundData, helpers: &DrawHelpers) {
        let wave = &data.waves[self.wave as usize];
        draw_text(
            helpers,
            0.0,
            HEADER_Y,
            &format!("WAV:{:02}", self.wave),
            COLOR_WHITE,
        );

        let top = 24.0;
        let bottom = 84.0;
        helpers.draw_rect(0.0, top, 160.0, 1.0, COLOR_DARK_GRAY);
        helpers.draw_rect(0.0, bottom, 160.0, 1.0, COLOR_DARK_GRAY);
        for index in 0..32 {
            let byte = wave[index / 2];
            let value = if index.is_multiple_of(2) {
                byte >> 4
            } else {
                byte & 0x0F
            };
            let selected = index as u8 == self.wave_sample;
            let color = if selected {
                COLOR_WHITE
            } else {
                COLOR_LIGHT_GRAY
            };
            let x = index as f32 * CELL_PX + 1.0;
            let height = (value as f32 / 15.0 * (bottom - top)).max(1.0);
            if selected {
                helpers.draw_rect(
                    index as f32 * CELL_PX,
                    top,
                    CELL_PX,
                    bottom - top,
                    COLOR_WHITE,
                );
                helpers.draw_rect(x, bottom - height, CELL_PX - 2.0, height, COLOR_BLACK);
            } else {
                helpers.draw_rect(x, bottom - height, CELL_PX - 2.0, height, COLOR_LIGHT_GRAY);
            }
            let c = char::from_digit(value as u32, 16).unwrap_or('?');
            helpers.draw_char(c, x, 90.0, color);
        }
    }

    fn draw_sfx(&self, data: &SoundData, helpers: &DrawHelpers) {
        draw_text(
            helpers,
            0.0,
            HEADER_Y,
            &format!("SFX:{:02}", self.sfx),
            COLOR_WHITE,
        );
        for (field, label, x, _) in SFX_COLUMNS {
            let color = if field == self.sfx_field {
                COLOR_WHITE
            } else {
                COLOR_DARK_GRAY
            };
            draw_text(
                helpers,
                x as f32 * TILE_WIDTH as f32,
                CONTENT_Y,
                label,
                color,
            );
        }
        for index in 0..AUDIO_SFX_COUNT {
            let entry = data.sfx[index];
            let y = SFX_ROWS_Y + index as f32 * TILE_HEIGHT as f32;
            let selected = index as u8 == self.sfx;
            let color = if selected {
                COLOR_WHITE
            } else {
                COLOR_LIGHT_GRAY
            };
            let note = note_parts(entry.note)
                .map(|(name, octave)| format!("{name}{octave}"))
                .unwrap_or_else(|| "---".to_string());
            let text = format!(
                "{:<4} {:<4} {:<4} {:<4} {:<4} {:<3}",
                entry.channel.label(),
                instrument_text(entry.instrument),
                table_text(entry.table),
                note,
                entry.priority,
                sfx_duration_text(entry.duration)
            );
            draw_text(helpers, 0.0, y, &text, color);
            if selected {
                let (field_x, field_width) = sfx_field_span(self.sfx_field);
                let field_text = sfx_field_text(entry, self.sfx_field, &note);
                let px = field_x as f32 * TILE_WIDTH as f32;
                helpers.draw_rect(
                    px,
                    y,
                    field_width as f32 * TILE_WIDTH as f32,
                    TILE_HEIGHT as f32,
                    COLOR_WHITE,
                );
                draw_text(helpers, px, y, &field_text, COLOR_BLACK);
            }
        }
    }
}

fn sfx_field_span(field: SfxField) -> (usize, usize) {
    SFX_COLUMNS
        .iter()
        .find(|(candidate, _, _, _)| *candidate == field)
        .map(|(_, _, x, width)| (*x, *width))
        .unwrap_or((0, 1))
}

fn sfx_field_text(entry: SfxEntry, field: SfxField, note: &str) -> String {
    match field {
        SfxField::Channel => entry.channel.label().to_string(),
        SfxField::Instrument => instrument_text(entry.instrument),
        SfxField::Table => table_text(entry.table),
        SfxField::Note => note.to_string(),
        SfxField::Priority => format!("{}", entry.priority),
        SfxField::Duration => sfx_duration_text(entry.duration),
    }
}

fn sfx_duration_text(duration: u8) -> String {
    if duration == 0 {
        "---".to_string()
    } else {
        format!("{duration:03}")
    }
}

fn instrument_text(instrument: u8) -> String {
    if instrument == 0 {
        "..".to_string()
    } else {
        format!("{instrument:02}")
    }
}

fn table_text(table: u8) -> String {
    if table == 0 {
        "..".to_string()
    } else {
        format!("{table:02}")
    }
}

fn wrap_table(table: u8, delta: i32) -> u8 {
    let count = AUDIO_TABLE_COUNT as i32 - 1;
    ((table as i32 - 1 + delta).rem_euclid(count) + 1) as u8
}

fn table_command_char(command: u8) -> char {
    match command {
        0 => '-',
        1 => 'E',
        2 => 'J',
        3 => 'R',
        _ => '?',
    }
}

fn table_command_name(command: u8) -> &'static str {
    match command {
        0 => "None",
        1 => "End",
        2 => "Jump",
        3 => "Restart",
        _ => "Unknown",
    }
}

impl InstrumentField {
    fn label(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Volume => "Volume",
            Self::EnvDir => "Env dir",
            Self::EnvPeriod => "Env speed",
            Self::Duty => "Duty",
            Self::Length => "Length",
            Self::LengthEnabled => "Length en",
            Self::SweepPace => "Sweep pace",
            Self::SweepDir => "Sweep dir",
            Self::SweepStep => "Sweep step",
            Self::WaveSlot => "Wave slot",
            Self::WaveVolume => "Wave vol",
            Self::NoiseWidth => "Noise 7bit",
            Self::NoiseDivider => "Noise div",
            Self::NoiseShift => "Noise shift",
            Self::Table => "Table",
            Self::Panning => "Panning",
            Self::Finetune => "Finetune",
        }
    }
}

fn field_value(instrument: Instrument, field: InstrumentField) -> String {
    match field {
        InstrumentField::Type => instrument.channel.label().to_string(),
        InstrumentField::Volume => instrument.volume.to_string(),
        InstrumentField::EnvDir => {
            if instrument.envelope_dir_up {
                "up".to_string()
            } else {
                "down".to_string()
            }
        }
        InstrumentField::EnvPeriod => instrument.envelope_period.to_string(),
        InstrumentField::Duty => instrument.duty.to_string(),
        InstrumentField::Length => instrument.length.to_string(),
        InstrumentField::LengthEnabled => {
            if instrument.length_enabled {
                "on".to_string()
            } else {
                "off".to_string()
            }
        }
        InstrumentField::SweepPace => instrument.sweep_pace.to_string(),
        InstrumentField::SweepDir => {
            if instrument.sweep_dir_up {
                "up".to_string()
            } else {
                "down".to_string()
            }
        }
        InstrumentField::SweepStep => instrument.sweep_step.to_string(),
        InstrumentField::WaveSlot => instrument.wave_slot.to_string(),
        InstrumentField::WaveVolume => instrument.wave_volume.to_string(),
        InstrumentField::NoiseWidth => {
            if instrument.noise_width_7bit {
                "7".to_string()
            } else {
                "15".to_string()
            }
        }
        InstrumentField::NoiseDivider => instrument.noise_divider.to_string(),
        InstrumentField::NoiseShift => instrument.noise_shift.to_string(),
        InstrumentField::Table => instrument.table.to_string(),
        InstrumentField::Panning => instrument.panning.to_string(),
        InstrumentField::Finetune => instrument.finetune.to_string(),
    }
}

fn instrument_coarse_step(field: InstrumentField) -> i32 {
    match field {
        InstrumentField::Volume
        | InstrumentField::WaveSlot
        | InstrumentField::NoiseShift
        | InstrumentField::Table => 4,
        InstrumentField::EnvPeriod
        | InstrumentField::SweepPace
        | InstrumentField::SweepStep
        | InstrumentField::NoiseDivider => 2,
        InstrumentField::Duty | InstrumentField::WaveVolume | InstrumentField::Panning => 2,
        InstrumentField::Length | InstrumentField::Finetune => 10,
        InstrumentField::Type
        | InstrumentField::EnvDir
        | InstrumentField::LengthEnabled
        | InstrumentField::SweepDir
        | InstrumentField::NoiseWidth => 1,
    }
}

fn bump_instrument(instrument: &mut Instrument, field: InstrumentField, delta: i32) {
    match field {
        InstrumentField::Type => {
            instrument.channel = match (instrument.channel as i32 + delta).rem_euclid(3) {
                0 => ChannelType::Pulse,
                1 => ChannelType::Wave,
                _ => ChannelType::Noise,
            };
        }
        InstrumentField::Volume => {
            instrument.volume = (instrument.volume as i32 + delta).rem_euclid(16) as u8;
        }
        InstrumentField::EnvDir => instrument.envelope_dir_up = !instrument.envelope_dir_up,
        InstrumentField::EnvPeriod => {
            instrument.envelope_period =
                (instrument.envelope_period as i32 + delta).rem_euclid(8) as u8;
        }
        InstrumentField::Duty => {
            instrument.duty = (instrument.duty as i32 + delta).rem_euclid(4) as u8;
        }
        InstrumentField::Length => {
            instrument.length = (instrument.length as i32 + delta).rem_euclid(256) as u8;
        }
        InstrumentField::LengthEnabled => instrument.length_enabled = !instrument.length_enabled,
        InstrumentField::SweepPace => {
            instrument.sweep_pace = (instrument.sweep_pace as i32 + delta).rem_euclid(8) as u8;
        }
        InstrumentField::SweepDir => instrument.sweep_dir_up = !instrument.sweep_dir_up,
        InstrumentField::SweepStep => {
            instrument.sweep_step = (instrument.sweep_step as i32 + delta).rem_euclid(8) as u8;
        }
        InstrumentField::WaveSlot => {
            instrument.wave_slot =
                (instrument.wave_slot as i32 + delta).rem_euclid(AUDIO_WAVE_COUNT as i32) as u8;
        }
        InstrumentField::WaveVolume => {
            instrument.wave_volume = (instrument.wave_volume as i32 + delta).rem_euclid(4) as u8;
        }
        InstrumentField::NoiseWidth => instrument.noise_width_7bit = !instrument.noise_width_7bit,
        InstrumentField::NoiseDivider => {
            instrument.noise_divider =
                (instrument.noise_divider as i32 + delta).rem_euclid(8) as u8;
        }
        InstrumentField::NoiseShift => {
            instrument.noise_shift = (instrument.noise_shift as i32 + delta).rem_euclid(16) as u8;
        }
        InstrumentField::Table => {
            instrument.table =
                (instrument.table as i32 + delta).rem_euclid(AUDIO_TABLE_COUNT as i32) as u8;
        }
        InstrumentField::Panning => {
            instrument.panning = (instrument.panning as i32 + delta).rem_euclid(4) as u8;
        }
        InstrumentField::Finetune => {
            instrument.finetune = (instrument.finetune as i32 + delta).rem_euclid(256) as i8;
        }
    }
}

fn draw_text(helpers: &DrawHelpers, x: f32, y: f32, text: &str, color: Color) {
    for (index, c) in text.chars().enumerate() {
        helpers.draw_char(c, x + index as f32 * TILE_WIDTH as f32, y, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::notes::NOTE_MIN;

    #[test]
    fn instrument_bump_wraps() {
        let mut instrument = Instrument::default();
        bump_instrument(&mut instrument, InstrumentField::Volume, 16);
        assert_eq!(instrument.volume, 0);
        bump_instrument(&mut instrument, InstrumentField::Duty, 1);
        assert_eq!(instrument.duty, 3);
    }

    #[test]
    fn full_range_instrument_fields_wrap() {
        let mut instrument = Instrument::default();
        bump_instrument(&mut instrument, InstrumentField::Length, -1);
        assert_eq!(instrument.length, 255);
        bump_instrument(&mut instrument, InstrumentField::Length, 1);
        assert_eq!(instrument.length, 0);

        instrument.finetune = 127;
        bump_instrument(&mut instrument, InstrumentField::Finetune, 1);
        assert_eq!(instrument.finetune, -128);
        bump_instrument(&mut instrument, InstrumentField::Finetune, -1);
        assert_eq!(instrument.finetune, 127);
    }

    #[test]
    fn instrument_coarse_steps_follow_ranges() {
        assert_eq!(instrument_coarse_step(InstrumentField::Volume), 4);
        assert_eq!(instrument_coarse_step(InstrumentField::Duty), 2);
        assert_eq!(instrument_coarse_step(InstrumentField::EnvPeriod), 2);
        assert_eq!(instrument_coarse_step(InstrumentField::Length), 10);
        assert_eq!(instrument_coarse_step(InstrumentField::Type), 1);
    }

    #[test]
    fn note_labels_use_notes_module() {
        assert_eq!(note_parts(34), Some(("A-", 4)));
        assert_eq!(NOTE_MIN, 1);
        assert_eq!(NOTE_MAX, 72);
    }

    #[test]
    fn instrument_zero_is_empty() {
        assert_eq!(instrument_text(0), "..");
        assert_eq!(instrument_text(1), "01");
        assert_eq!(instrument_text(31), "31");
    }

    #[test]
    fn tables_are_numbered_from_one() {
        assert_eq!(wrap_table(1, -1), 15);
        assert_eq!(wrap_table(15, 1), 1);
        assert_eq!(wrap_table(1, 10), 11);
    }

    #[test]
    fn table_zero_is_empty_in_sfx_editor() {
        assert_eq!(table_text(0), "..");
        assert_eq!(table_text(1), "01");
        assert_eq!(table_text(15), "15");
    }

    #[test]
    fn table_commands_have_labels() {
        assert_eq!(table_command_char(0), '-');
        assert_eq!(table_command_char(1), 'E');
        assert_eq!(table_command_char(2), 'J');
        assert_eq!(table_command_char(3), 'R');
        assert_eq!(table_command_name(0), "None");
        assert_eq!(table_command_name(1), "End");
        assert_eq!(table_command_name(2), "Jump");
        assert_eq!(table_command_name(3), "Restart");
    }
}
