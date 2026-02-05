use std::fmt;

use indexmap::IndexMap;
use log::error;
use quick_xml::{
    events::{BytesStart, Event},
    name::QName,
};
use serde::{Deserialize, Deserializer, Serialize, de};

/// https://w3c.github.io/musicxml/musicxml-reference/elements/score-partwise/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ScorePartwise {
    pub part: Vec<Part>,
}

impl ScorePartwise {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut part = Vec::new();
        loop {
            match dbg!(reader.read_event().unwrap()) {
                Event::Start(b) => match b.name().as_ref() {
                    b"part" => {
                        part.push(Part::parse(reader, &b));
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => {
                    error!("Unexpected Eof");
                    break;
                }
                _ => {}
            }
        }

        Self { part }
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/part-partwise/
#[derive(Debug, Serialize, Deserialize)]
pub struct Part {
    pub measure: Vec<Measure>,
}

impl Part {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut measure = Vec::new();
        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"measure" => {
                        measure.push(Measure::parse(reader, &b));
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => todo!(),
                _ => {}
            }
        }

        Self { measure }
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/measure-partwise/
#[derive(Debug, Serialize, Deserialize)]
pub struct Measure {
    #[serde(rename = "$value")]
    pub content: Vec<MeasureItem>,
}

impl Measure {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut content = Vec::new();
        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"print" => {
                        content.push(MeasureItem::Print(Print::parse(reader, &b)));
                    }
                    b"attributes" => {
                        content.push(MeasureItem::Attributes(Attributes::parse(reader, &b)));
                    }
                    b"note" => {
                        content.push(MeasureItem::Note(Note::parse(reader, &b)));
                    }
                    b"barline" => {
                        content.push(MeasureItem::Barline(Barline::parse(reader, &b)));
                    }
                    b"backup" => {
                        content.push(MeasureItem::Backup(Backup::parse(reader, &b)));
                    }
                    b"direction" => {
                        content.push(MeasureItem::Direction(Direction::parse(reader, &b)));
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => todo!(),
                _ => {}
            }
        }

        Self { content }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(clippy::large_enum_variant)]
pub enum MeasureItem {
    Print(Print),
    Attributes(Attributes),
    Note(Note),
    Barline(Barline),
    Backup(Backup),
    Direction(Direction),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Print {}

impl Print {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        reader.read_to_end(start.name()).unwrap();
        Self {}
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/attributes/
#[derive(Debug, Serialize, Deserialize)]
pub struct Attributes {
    pub divisions: Option<PositiveDivisions>,
    #[serde(default)]
    pub key: Vec<Key>,
    #[serde(default)]
    pub time: Vec<Time>,
    #[serde(default)]
    pub clef: Vec<Clef>,
}

impl Attributes {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut divisions: Option<PositiveDivisions> = None;
        let mut key: Vec<Key> = vec![];
        let mut time: Vec<Time> = vec![];
        let mut clef: Vec<Clef> = vec![];

        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"divisions" => {
                        divisions = reader.read_text_and_parse(b.name());
                    }
                    b"key" => {
                        key.push(Key::parse(reader, &b));
                    }
                    b"time" => {
                        time.push(Time::parse(reader, &b));
                    }
                    b"clef" => {
                        clef.push(Clef::parse(reader, &b));
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => todo!(),
                _ => {}
            }
        }

        Self {
            divisions,
            key,
            time,
            clef,
        }
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/barline/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Barline {}

impl Barline {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        reader.read_to_end(start.name()).unwrap();
        Self {}
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/backup/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Backup {
    pub duration: PositiveDivisions,
}

impl Backup {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut duration: Option<PositiveDivisions> = None;

        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"duration" => {
                        duration = reader.read_text_and_parse(b.name());
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => todo!(),
                _ => {}
            }
        }

        let Some(duration) = duration else {
            todo!("duration missing");
        };

        Self { duration }
    }
}

/// https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Direction {
    pub sound: Option<Sound>,
}

impl Direction {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut sound: Option<Sound> = None;

        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"sound" => {
                        sound = Some(Sound::parse(reader, &b));
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => todo!(),
                _ => {}
            }
        }

        Self { sound }
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/sound/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Sound {
    #[serde(rename = "@tempo")]
    pub tempo: Option<NonNegativeDecimal>,
}

impl Sound {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        // TODO: Shity code
        let tempo: Option<NonNegativeDecimal> = start
            .attributes()
            .filter_map(|res| res.ok())
            .find(|attr| attr.key.into_inner() == b"tempo")
            .and_then(|attr| {
                let attr = std::str::from_utf8(&attr.value)
                    .inspect_err(|err| error!("{err}"))
                    .ok()?;
                attr.parse().inspect_err(|err| error!("{err}")).ok()
            });

        reader.read_to_end(start.name()).unwrap();

        Self { tempo }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Key {
    pub fifths: String,
}

impl Key {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut fifths: String = String::new();

        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"fifths" => {
                        fifths = reader.read_text_and_parse(b.name()).unwrap_or_default();
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => todo!(),
                _ => {}
            }
        }

        Self { fifths }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Time {
    pub beats: String,
    pub beat_type: String,
}

impl Time {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut beats: String = String::new();
        let mut beat_type: String = String::new();

        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"beats" => {
                        beats = reader.read_text_and_parse(b.name()).unwrap_or_default();
                    }
                    b"beat_type" => {
                        beat_type = reader.read_text_and_parse(b.name()).unwrap_or_default();
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => todo!(),
                _ => {}
            }
        }

        Self { beats, beat_type }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Clef {
    pub sign: String,
    pub line: String,
}

impl Clef {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut sign: String = String::new();
        let mut line: String = String::new();

        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"sign" => {
                        sign = reader.read_text_and_parse(b.name()).unwrap_or_default();
                    }
                    b"line" => {
                        line = reader.read_text_and_parse(b.name()).unwrap_or_default();
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => todo!(),
                _ => {}
            }
        }

        Self { sign, line }
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/note/
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Note {
    // TODO:
    // ... more attributes
    // ... There are multiple note types
    pub pitch: Option<Pitch>,
    pub chord: Option<Chord>,
    pub duration: String,
    pub voice: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub stem: Option<String>,
    pub rest: Option<Rest>,
    pub tie: Option<Tie>,
}

impl Note {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut rest: Option<Rest> = None;

        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"rest" => {
                        rest = Some(Rest::parse(reader, &b));
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => todo!(),
                _ => {}
            }
        }

        Self {
            pitch: None,
            chord: None,
            duration: todo!(),
            voice: None,
            kind: None,
            stem: None,
            rest,
            tie: None,
        }
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/tie/
#[derive(Debug, Serialize, Deserialize)]
pub struct Tie {
    #[serde(rename = "@type")]
    pub kind: StartStop,
    #[serde(rename = "@time-only")]
    pub time_only: Option<String>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/start-stop/
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum StartStop {
    Start,
    Stop,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/pitch/
#[derive(Debug, Serialize, Deserialize)]
pub struct Pitch {
    pub step: Step,
    pub alter: Option<Semitones>,
    pub octave: Octave,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chord {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rest {
    #[serde(rename = "@measure")]
    pub measure: bool,
}

impl Rest {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        // TODO: Shity code
        let measure: bool = start
            .attributes()
            .filter_map(|res| res.ok())
            .find(|attr| attr.key.into_inner() == b"measure")
            .map(|attr| match attr.value.as_ref() {
                b"yes" => true,
                b"no" => false,
                value => {
                    error!("Unexpected bool value: {value:?}");
                    false
                }
            })
            .unwrap_or(false);

        reader.read_to_end(start.name()).unwrap();

        Self { measure }
    }
}

pub use primitive::*;

use crate::{Reader, ReaderExt};
mod primitive {
    #![allow(unused)]

    use super::*;

    pub type Decimal = f64;

    /// The `tenths` type is a number representing tenths of interline staff space
    /// (positive or negative). Both integer and decimal values are allowed.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/tenths/
    pub type Tenths = Decimal;

    /// The `divisions` type is used to express values in terms of the musical divisions
    /// defined by the <divisions> element.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/divisions/
    pub type Divisions = Decimal;

    /// The `non-negative-decimal` type specifies a non-negative decimal value (>= 0).
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/non-negative-decimal/
    pub type NonNegativeDecimal = Decimal;

    /// The `positive-decimal` type specifies a positive decimal value (> 0).
    pub type PositiveDecimal = Decimal;

    /// The `positive-divisions` type restricts divisions values to positive numbers (> 0).
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/positive-divisions/
    pub type PositiveDivisions = Decimal;

    /// The `percent` type specifies a percentage from 0 to 100.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/percent/
    pub type Percent = Decimal;

    /// The `rotation-degrees` type specifies rotation, pan, and elevation values in degrees
    /// (range -180..180).
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/rotation-degrees/
    pub type RotationDegrees = Decimal;

    /// The `trill-beats` type specifies the beats used in a trill-sound or bend-sound attribute group.
    /// It is a decimal value with a minimum value of 2.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/trill-beats/
    pub type TrillBeats = Decimal;

    /// The `beam-level` type identifies concurrent beams in a beam group (1..8).
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/beam-level/
    pub type BeamLevel = u8;

    /// The `midi-16` type is used to express MIDI 1.0 values that range from 1 to 16.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/midi-16/
    pub type Midi16 = u8;

    /// The `midi-128` type is used to express MIDI 1.0 values that range from 1 to 128.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/midi-128/
    pub type Midi128 = u8;

    /// The `midi-16384` type is used to express MIDI 1.0 values that range from 1 to 16,384.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/midi-16384/
    pub type Midi16384 = u16;

    /// The `number-level` type distinguishes up to 16 concurrent objects of the same type
    /// when the objects overlap in MusicXML document order.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/number-level/
    pub type NumberLevel = u8;

    /// The `number-of-lines` type is used to specify the number of lines in text decoration attributes (0..3).
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/number-of-lines/
    pub type NumberOfLines = u8;

    /// The `numeral-value` type represents a Roman numeral or Nashville number value as a positive integer from 1 to 7.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/numeral-value/
    pub type NumeralValue = u8;

    /// The `string-number` type indicates a string number. Strings are numbered from high to low,
    /// with 1 being the highest pitched full-length string.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/string-number/
    pub type StringNumber = u32;

    /// The semitones type is a number representing semitones, used for chromatic alteration.
    /// A value of -1 corresponds to a flat and a value of 1 to a sharp.
    /// Decimal values like 0.5 (quarter tone sharp) are used for microtones.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/semitones/
    pub type Semitones = Decimal;

    /// Octaves are represented by the numbers 0 to 9, where 4 indicates the octave started by middle C.
    /// Minimum allowed value: 0
    ///
    /// Maximum allowed value: 9
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/octave/
    pub type Octave = u8;

    /// The step type represents a step of the diatonic scale, represented using the English letters A through G.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/step/
    #[derive(Debug, Clone, Copy, Eq, Ord, Hash, PartialEq, PartialOrd, Deserialize, Serialize)]
    pub enum Step {
        A,
        B,
        C,
        D,
        E,
        F,
        G,
    }
}
