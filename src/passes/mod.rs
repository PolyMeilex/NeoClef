use std::{
    collections::BTreeMap,
    ops::{Add, AddAssign, Sub, SubAssign},
};

use crate::parsing::musicxml;

mod chord_symbols;
mod encode;
mod fermata;
mod layout;
mod playback_order;
mod sounding_notes;
mod tempo;
mod unroll;

pub use chord_symbols::chord_symbols;
pub use encode::encode;
pub use fermata::fermata;
pub use layout::layout;
pub use playback_order::playback_order;
pub use sounding_notes::sounding_notes;
pub use tempo::tempo;
pub use unroll::unroll;

pub type Tick = usize;

pub const TICKS_PER_QUARTER_NOTE: u16 = 480;

#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Time(f64);

impl Time {
    pub const ZERO: Self = Self(0.0);

    pub fn from_divisions(value: f64, divisions: f64) -> Self {
        Self(value / divisions)
    }

    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }

    pub fn to_ticks(self) -> Tick {
        (self.0 * f64::from(TICKS_PER_QUARTER_NOTE))
            .round()
            .max(0.0) as Tick
    }
}

pub fn round_tempo(bps: f64) -> f64 {
    let scale = 10f64.powi(6);
    (bps * scale + 0.5).floor() / scale
}

impl Add for Time {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Time {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl AddAssign for Time {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Time {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub direction_offset_always_sounds: bool,
    pub ignore_sound_offset: bool,
    pub harmony_offset_always_sounds: bool,
}

impl Policy {
    pub fn new(musescore_compat: bool) -> Self {
        Self {
            direction_offset_always_sounds: musescore_compat,
            ignore_sound_offset: musescore_compat,
            harmony_offset_always_sounds: musescore_compat,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeasureIR {
    pub start: Time,
    pub duration: Time,
    pub notes: Vec<NoteEv>,
    pub controls: Vec<(Time, Control)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteId {
    pub measure: usize,
    pub item: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    Regular,
    Cue,
    Grace,
    Rest,
    Unpitched,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TieFlags {
    pub start: bool,
    pub stop: bool,
}

#[derive(Debug, Clone)]
pub struct NoteEv {
    pub id: NoteId,
    pub at: Time,
    pub dur: Time,
    pub kind: NoteKind,
    pub pitch: Option<musicxml::Pitch>,
    pub staff: u8,
    pub tie: TieFlags,
    pub dynamics: Option<f64>,
    pub end_dynamics: Option<f64>,
    pub fermata: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Control {
    Tempo(f64),
    ChordSymbol { staff: u8, keys: Vec<u8> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visit {
    pub measure: usize,
}

#[derive(Debug, Clone)]
pub struct PEvent {
    pub id: NoteId,
    pub on: Tick,
    pub off: Tick,
    pub kind: NoteKind,
    pub pitch: Option<musicxml::Pitch>,
    pub staff: u8,
    pub tie: TieFlags,
    pub dynamics: Option<f64>,
    pub end_dynamics: Option<f64>,
    pub fermata: Option<f64>,
}

#[derive(Debug)]
pub struct Performance {
    pub events: Vec<PEvent>,
    pub controls: Vec<(Tick, Control)>,
    pub tempo: BTreeMap<Tick, f64>,
    pub end: Tick,
}

pub const DEFAULT_VELOCITY: u8 = 80;

#[derive(Debug, Clone)]
pub struct MidiNote {
    #[allow(
        dead_code,
        reason = "for Neothesia, to map what sounds back to the sheet"
    )]
    pub id: Option<NoteId>,
    pub on: Tick,
    pub off: Tick,
    pub key: u8,
    pub vel: u8,
    pub off_vel: u8,
    pub tie: TieFlags,
    pub staff: u8,
    pub channel: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_ticks() {
        assert_eq!(Time::from_divisions(3.0, 2.0).to_ticks(), 720);
        assert_eq!(Time::from_divisions(1.0, 768.0).to_ticks(), 1);
        assert_eq!((Time::ZERO - Time::from_divisions(1.0, 1.0)).to_ticks(), 0);
    }
}
