pub const NOTE_MIN: u8 = 1;
pub const NOTE_MAX: u8 = 72;
pub const NOTE_OFF: u8 = 73;
pub const NOTE_COUNT: usize = 72;

/// Pulse channel periods for C-2 through B-7.
///
/// A4 is index 33 and uses period 1750.
pub const PULSE_PERIODS: [u16; NOTE_COUNT] = [
    44, 156, 262, 363, 457, 547, 631, 710, 786, 854, 923, 986, 1046, 1102, 1155, 1205, 1253, 1297,
    1339, 1379, 1417, 1452, 1486, 1517, 1546, 1575, 1602, 1627, 1650, 1673, 1694, 1714, 1732, 1750,
    1767, 1783, 1798, 1812, 1825, 1837, 1849, 1860, 1871, 1881, 1890, 1899, 1907, 1915, 1923, 1930,
    1936, 1943, 1949, 1954, 1959, 1964, 1969, 1974, 1978, 1982, 1985, 1988, 1992, 1995, 1998, 2001,
    2004, 2006, 2009, 2011, 2013, 2015,
];

pub fn pulse_period(note: u8) -> Option<u16> {
    if (NOTE_MIN..=NOTE_MAX).contains(&note) {
        Some(PULSE_PERIODS[(note - NOTE_MIN) as usize])
    } else {
        None
    }
}

/// The wave channel is one octave lower than a pulse channel for the same
/// period value, so the same written note needs the period of note + 12.
pub fn wave_period(note: u8) -> Option<u16> {
    note.checked_add(12).and_then(pulse_period)
}

pub fn note_parts(note: u8) -> Option<(&'static str, u8)> {
    const NAMES: [&str; 12] = [
        "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
    ];
    if (NOTE_MIN..=NOTE_MAX).contains(&note) {
        let index = (note - NOTE_MIN) as usize;
        Some((NAMES[index % 12], 2 + (index / 12) as u8))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_1750() {
        assert_eq!(pulse_period(34), Some(1750));
    }

    #[test]
    fn note_range_is_c2_to_b7() {
        assert_eq!(pulse_period(NOTE_MIN), Some(44));
        assert_eq!(pulse_period(NOTE_MAX), Some(2015));
        assert_eq!(pulse_period(0), None);
        assert_eq!(pulse_period(73), None);
    }

    #[test]
    fn wave_is_one_octave_lower_for_same_period() {
        assert_eq!(wave_period(34), pulse_period(46));
        assert_eq!(wave_period(34), Some(1899));
        assert_eq!(wave_period(70), None);
    }

    #[test]
    fn note_parts_match_table() {
        assert_eq!(note_parts(1), Some(("C-", 2)));
        assert_eq!(note_parts(34), Some(("A-", 4)));
        assert_eq!(note_parts(72), Some(("B-", 7)));
    }
}
