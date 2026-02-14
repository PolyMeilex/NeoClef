#![allow(dead_code)]

use std::str::FromStr;

use log::error;
use quick_xml::{
    events::{BytesStart, Event},
    reader::Span,
};

#[derive(thiserror::Error, Debug)]
pub enum MusicXmlParseError {
    #[error("Missing tag {0:?} at {1:?}")]
    MissingTag(&'static str, Span),
    #[error("Unexpected end of file")]
    UnexpectedEof,
    #[error(transparent)]
    Xml(#[from] quick_xml::errors::Error),
}

pub type Result<T, E = MusicXmlParseError> = std::result::Result<T, E>;

/// https://w3c.github.io/musicxml/musicxml-reference/elements/score-partwise/
#[derive(Debug)]
pub struct ScorePartwise {
    pub part: Vec<Part>,
}

impl ScorePartwise {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let mut part = Vec::new();
        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"part" => part.push(Part::parse(reader, &b)?),
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        Ok(Self { part })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/part-partwise/
#[derive(Debug)]
pub struct Part {
    pub measure: Vec<Measure>,
}

impl Part {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let mut measure = Vec::new();
        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"measure" => measure.push(Measure::parse(reader, &b)?),
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        Ok(Self { measure })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/measure-partwise/
#[derive(Debug)]
pub struct Measure {
    pub content: Vec<MeasureItem>,
}

impl Measure {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let mut content = Vec::new();
        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"print" => {
                        content.push(MeasureItem::Print(Print::parse(reader, &b)));
                    }
                    b"attributes" => {
                        content.push(MeasureItem::Attributes(Attributes::parse(reader, &b)?));
                    }
                    b"note" => {
                        content.push(MeasureItem::Note(Note::parse(reader, &b)?));
                    }
                    b"barline" => {
                        content.push(MeasureItem::Barline(Barline::parse(reader, &b)));
                    }
                    b"backup" => {
                        content.push(MeasureItem::Backup(Backup::parse(reader, &b)?));
                    }
                    b"direction" => {
                        content.push(MeasureItem::Direction(Direction::parse(reader, &b)?));
                    }
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        Ok(Self { content })
    }
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum MeasureItem {
    Print(Print),
    Attributes(Attributes),
    Note(Note),
    Barline(Barline),
    Backup(Backup),
    Direction(Direction),
}

#[derive(Debug)]
pub struct Print {}

impl Print {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        reader.read_to_end(start.name()).unwrap();
        Self {}
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/attributes/
#[derive(Debug)]
pub struct Attributes {
    pub divisions: Option<PositiveDivisions>,
    pub key: Vec<Key>,
    pub time: Vec<Time>,
    pub clef: Vec<Clef>,
}

impl Attributes {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let mut divisions: Option<PositiveDivisions> = None;
        let mut key: Vec<Key> = vec![];
        let mut time: Vec<Time> = vec![];
        let mut clef: Vec<Clef> = vec![];

        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"divisions" => divisions = reader.read_text_as(b.name()),
                    b"key" => key.push(Key::parse(reader, &b)?),
                    b"time" => time.push(Time::parse(reader, &b)?),
                    b"clef" => clef.push(Clef::parse(reader, &b)?),
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        Ok(Self {
            divisions,
            key,
            time,
            clef,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/barline/
#[derive(Debug)]
pub struct Barline {}

impl Barline {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        reader.read_to_end(start.name()).unwrap();
        Self {}
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/backup/
#[derive(Debug)]
pub struct Backup {
    pub duration: PositiveDivisions,
}

impl Backup {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let span_start = reader.buffer_position();

        let mut duration: Option<PositiveDivisions> = None;

        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"duration" => duration = reader.read_text_as(b.name()),
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        let Some(duration) = duration else {
            return Err(MusicXmlParseError::MissingTag(
                "duration",
                span_start..reader.buffer_position(),
            ));
        };

        Ok(Self { duration })
    }
}

/// https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/
#[derive(Debug)]
pub struct Direction {
    pub sound: Option<Sound>,
}

impl Direction {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let mut sound: Option<Sound> = None;

        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"sound" => sound = Some(Sound::parse(reader, &b)),
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        Ok(Self { sound })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/sound/
#[derive(Debug)]
pub struct Sound {
    pub tempo: Option<NonNegativeDecimal>,
}

impl Sound {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut tempo: Option<NonNegativeDecimal> = None;

        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"tempo" => tempo = parse_str_as(&attr.value),
                _ => {}
            }
        }

        reader.read_to_end(start.name()).unwrap();

        Self { tempo }
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/key/
#[derive(Debug)]
pub struct Key {
    pub fifths: String,
}

impl Key {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let start_span = reader.buffer_position();

        let mut fifths: Option<String> = None;

        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"fifths" => fifths = reader.read_text_as(b.name()),
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        let fifths = fifths.ok_or(MusicXmlParseError::MissingTag(
            "fifths",
            start_span..reader.buffer_position(),
        ))?;

        Ok(Self { fifths })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/time/
#[derive(Debug, PartialEq)]
pub struct Time {
    pub beats: String,
    pub beat_type: String,
}

impl Time {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let mut beats = None;
        let mut beat_type = None;

        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"beats" => beats = reader.read_text(b.name()).ok(),
                    b"beat-type" => beat_type = reader.read_text(b.name()).ok(),
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        let beats = beats.unwrap().to_string();
        let beat_type = beat_type.unwrap().to_string();

        Ok(Self { beats, beat_type })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/clef/
#[derive(Debug)]
pub struct Clef {
    pub sign: ClefSign,
    pub line: Option<StaffLinePosition>,
}

impl Clef {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let mut sign: Option<ClefSign> = None;
        let mut line: Option<StaffLinePosition> = None;

        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"sign" => sign = reader.read_text_as(b.name()),
                    b"line" => line = reader.read_text_as(b.name()),
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        let sign = sign.unwrap();

        Ok(Self { sign, line })
    }
}

#[derive(Debug)]
pub enum NoteKind {
    Grace,
    // GraceCue,
    Cue,
    Regular,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/note/
#[derive(Debug)]
pub struct Note {
    pub pitch: Option<Pitch>,
    pub chord: Option<Chord>,
    pub duration: PositiveDivisions,
    pub voice: Option<String>,
    pub kind: Option<String>,
    pub stem: Option<String>,
    pub rest: Option<Rest>,
    pub tie: Option<Tie>,
    pub note_kind: NoteKind,
}

impl Note {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let span_start = reader.buffer_position();

        let mut pitch: Option<Pitch> = None;
        let mut chord: Option<Chord> = None;
        let mut duration: Option<PositiveDivisions> = None;
        let mut voice: Option<String> = None;
        let mut kind: Option<String> = None;
        let mut stem: Option<String> = None;
        let mut rest: Option<Rest> = None;
        let mut tie: Option<Tie> = None;

        let first = loop {
            match reader.read_event()? {
                Event::Start(b) => break b,
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    // TODO: Wrong name
                    return Err(MusicXmlParseError::MissingTag(
                        "note",
                        span_start..reader.buffer_position(),
                    ));
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        };

        let note_kind = match first.name().as_ref() {
            b"grace" => {
                // Ignore this grace element
                reader.read_to_end(first.name()).unwrap();
                // Ignore this whole note
                reader.read_to_end(start.name()).unwrap();
                NoteKind::Grace
            }
            b"cue" => {
                // Ignore this cue element
                reader.read_to_end(first.name()).unwrap();
                // Ignore this whole note element
                reader.read_to_end(start.name()).unwrap();
                NoteKind::Cue
            }
            _ => {
                let mut handle_start = |b: &BytesStart<'_>| -> Result<()> {
                    match b.name().as_ref() {
                        b"pitch" => pitch = Some(Pitch::parse(reader, b)?),
                        b"chord" => chord = Some(Chord::parse(reader, b)?),
                        b"duration" => duration = reader.read_text_as(b.name()),
                        b"voice" => voice = reader.read_text_as(b.name()),
                        b"type" => kind = reader.read_text_as(b.name()),
                        b"stem" => stem = reader.read_text_as(b.name()),
                        b"rest" => rest = Some(Rest::parse(reader, b)),
                        b"tie" => tie = Some(Tie::parse(reader, b)),
                        _ => {
                            reader.read_to_end(b.name()).unwrap();
                        }
                    }

                    Ok(())
                };

                handle_start(start)?;

                loop {
                    match reader.read_event()? {
                        Event::Start(b) => handle_start(&b)?,
                        Event::End(b) => {
                            assert_eq!(b.name(), start.name());
                            break;
                        }
                        Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                        _ => {}
                    }
                }

                NoteKind::Regular
            }
        };

        let Some(duration) = duration else {
            return Err(MusicXmlParseError::MissingTag(
                "duration",
                span_start..reader.buffer_position(),
            ));
        };

        Ok(Self {
            pitch,
            chord,
            duration,
            voice,
            kind,
            stem,
            rest,
            tie,
            note_kind,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/tie/
#[derive(Debug)]
pub struct Tie {
    pub kind: StartStop,
    pub time_only: Option<String>,
}

impl Tie {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut kind: Option<StartStop> = None;
        let mut time_only: Option<String> = None;

        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"type" => kind = StartStop::parse(attr.value.as_ref()),
                b"time-only" => time_only = parse_str_as(&attr.value),
                _ => {}
            }
        }

        reader.read_to_end(start.name()).unwrap();

        Tie {
            kind: kind.expect("Missing kind"),
            time_only,
        }
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/start-stop/
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartStop {
    Start,
    Stop,
}

impl StartStop {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v = match bytes {
            b"start" => StartStop::Start,
            b"stop" => StartStop::Stop,
            other => {
                error!("Unexpected tie type: {other:?}");
                return None;
            }
        };

        Some(v)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/pitch/
#[derive(Debug)]
pub struct Pitch {
    pub step: Step,
    pub alter: Option<Semitones>,
    pub octave: Octave,
}

impl Pitch {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        let mut step: Option<Step> = None;
        let mut alter: Option<Semitones> = None;
        let mut octave: Option<Octave> = None;

        loop {
            match reader.read_event()? {
                Event::Start(b) => match b.name().as_ref() {
                    b"step" => step = Step::parse(reader, &b),
                    b"alter" => alter = reader.read_text_as(b.name()),
                    b"octave" => octave = reader.read_text_as(b.name()),
                    _ => {
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name(), start.name());
                    break;
                }
                Event::Eof => return Err(MusicXmlParseError::UnexpectedEof),
                _ => {}
            }
        }

        let step = step.expect("missing step in pitch");
        let octave = octave.expect("missing octave in pitch");

        Ok(Self {
            step,
            alter,
            octave,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/chord/
#[derive(Debug)]
pub struct Chord {}

impl Chord {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Result<Self> {
        reader.read_to_end(start.name())?;
        Ok(Self {})
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/rest/
#[derive(Debug)]
pub struct Rest {
    pub measure: bool,
}

impl Rest {
    pub fn parse(reader: &mut Reader, start: &BytesStart) -> Self {
        let mut measure = false;

        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"measure" => measure = parse_yes_no(attr.value.as_ref()),
                _ => {}
            }
        }

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

    /// https://w3c.github.io/musicxml/musicxml-reference/data-types/staff-line-position/
    pub type StaffLinePosition = u8;

    pub fn parse_yes_no(s: &[u8]) -> bool {
        match s {
            b"yes" => true,
            b"no" => false,
            value => {
                error!("Unexpected bool value: {value:?}");
                false
            }
        }
    }

    pub fn parse_str(s: &[u8]) -> &str {
        std::str::from_utf8(s).unwrap()
    }

    pub fn parse_str_as<T: FromStr>(v: &[u8]) -> Option<T>
    where
        T::Err: std::fmt::Display,
    {
        let v = std::str::from_utf8(v)
            .inspect_err(|err| error!("{err}"))
            .ok()?;
        v.parse().inspect_err(|err| error!("{err}")).ok()
    }

    /// The step type represents a step of the diatonic scale, represented using the English letters A through G.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/step/
    #[derive(Debug, Clone, Copy, Eq, Ord, Hash, PartialEq, PartialOrd)]
    pub enum Step {
        A,
        B,
        C,
        D,
        E,
        F,
        G,
    }

    impl Step {
        pub fn parse(reader: &mut Reader, start: &BytesStart) -> Option<Self> {
            let txt = reader.read_text(start.name()).unwrap_or_default();

            let step = match txt.as_ref() {
                "A" => Step::A,
                "B" => Step::B,
                "C" => Step::C,
                "D" => Step::D,
                "E" => Step::E,
                "F" => Step::F,
                "G" => Step::G,
                other => {
                    error!("Unexpected step value: {other}");
                    return None;
                }
            };

            Some(step)
        }
    }

    /// https://w3c.github.io/musicxml/musicxml-reference/data-types/clef-sign/
    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub enum ClefSign {
        G,
        F,
        C,
        Percussion,
        Tab,
        Jianpu,
        None,
    }

    impl FromStr for ClefSign {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let v = match s {
                "G" => ClefSign::G,
                "F" => ClefSign::F,
                "C" => ClefSign::C,
                "percussion" => ClefSign::Percussion,
                "TAB" => ClefSign::Tab,
                "jianpu" => ClefSign::Jianpu,
                "none" => ClefSign::None,
                other => return Err(format!("unknown clef sign: {}", other)),
            };

            Ok(v)
        }
    }
}
