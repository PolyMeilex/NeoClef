use std::fmt;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, de};

/// https://w3c.github.io/musicxml/musicxml-reference/elements/score-partwise/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ScorePartwise {
    #[serde(rename = "@version")]
    pub version: Option<String>,
    pub identification: Option<Identification>,
    pub part_list: PartList,
    pub part: Vec<Part>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/identification/
#[derive(Debug, Serialize, Deserialize)]
pub struct Identification {
    #[serde(default)]
    pub creator: Vec<Creator>,
    pub encoding: Option<Encoding>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/creator/
#[derive(Debug, Serialize, Deserialize)]
pub struct Creator {}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/encoding/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Encoding {
    #[serde(default)]
    pub supports: Vec<Supports>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/supports/
#[derive(Debug, Serialize, Deserialize)]
pub struct Supports {
    #[serde(rename = "@element")]
    pub element: String,
    #[serde(rename = "@type")]
    pub kind: String,
    #[serde(rename = "@attribute")]
    pub attribute: Option<String>,
    #[serde(rename = "@value")]
    pub value: Option<String>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/part-list/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PartList {}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/part-partwise/
#[derive(Debug, Serialize, Deserialize)]
pub struct Part {
    #[serde(rename = "@id")]
    pub id: String,
    pub measure: Vec<Measure>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/measure-partwise/
#[derive(Debug, Serialize, Deserialize)]
pub struct Measure {
    #[serde(rename = "@number")]
    pub number: String,
    #[serde(rename = "@id")]
    pub id: Option<String>,
    #[serde(rename = "@implicit")]
    pub implicit: Option<String>,
    #[serde(rename = "@non-controlling")]
    pub non_controlling: Option<String>,
    #[serde(rename = "@text")]
    pub text: Option<String>,
    #[serde(rename = "@width")]
    pub width: Option<String>,

    #[serde(rename = "$value")]
    pub content: Vec<MeasureItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(clippy::large_enum_variant)]
pub enum MeasureItem {
    Print(Print),
    Attributes(Attributes),
    #[serde(deserialize_with = "deserialize_data")]
    Note(Note),
    Barline(Barline),
    Backup(Backup),
    Direction(Direction),
}

#[derive(Debug)]
pub enum NoteKind {
    Grace,
    GraceCue,
    Cue,
    Regular,
}

fn deserialize_data<'de, D>(deserializer: D) -> Result<Note, D::Error>
where
    D: Deserializer<'de>,
{
    pub struct ValueVisitor;

    impl<'de> de::Visitor<'de> for ValueVisitor {
        type Value = IndexMap<String, serde_value::Value>;

        fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
            fmt.write_str("map")
        }

        fn visit_map<A>(self, mut visitor: A) -> Result<Self::Value, A::Error>
        where
            A: de::MapAccess<'de>,
        {
            let mut map = IndexMap::new();
            while let Some((key, value)) = visitor.next_entry()? {
                map.insert(key, value);
            }
            Ok(map)
        }
    }

    let value = deserializer.deserialize_map(ValueVisitor).unwrap();

    let mut iter = value.keys().skip_while(|key| key.starts_with("@"));

    let note_kind = if let Some(key) = iter.next() {
        match key.as_str() {
            "grace" => {
                if let Some("cue") = iter.next().map(|k| k.as_str()) {
                    NoteKind::GraceCue
                } else {
                    NoteKind::Grace
                }
            }
            "cue" => NoteKind::Cue,
            _ => NoteKind::Regular,
        }
    } else {
        NoteKind::Regular
    };

    dbg!(note_kind);

    todo!();

    // deserializer.deserialize_map(JsonStringVisitor).unwrap();

    Ok(Note::default())
}

/// https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Direction {
    pub sound: Option<Sound>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/sound/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Sound {
    #[serde(rename = "@tempo")]
    pub tempo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Print {
    pub system_layout: SystemLayout,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/system-layout/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SystemLayout {
    pub system_margins: Option<SystemMargins>,
    pub top_system_distance: Option<String>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/system-margins/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SystemMargins {
    pub left_margin: String,
    pub right_margin: String,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/attributes/
#[derive(Debug, Serialize, Deserialize)]
pub struct Attributes {
    pub divisions: Option<String>,
    #[serde(default)]
    pub key: Vec<Key>,
    #[serde(default)]
    pub time: Vec<Time>,
    #[serde(default)]
    pub clef: Vec<Clef>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Key {
    pub fifths: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Time {
    pub beats: String,
    pub beat_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Clef {
    pub sign: String,
    pub line: String,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/note/
#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Note {
    #[serde(rename = "@attack")]
    pub attack: Option<Divisions>,
    #[serde(rename = "@color")]
    pub color: Option<String>,
    #[serde(rename = "@default-x")]
    pub default_x: Option<Tenths>,
    #[serde(rename = "@default-y")]
    pub default_y: Option<Tenths>,
    #[serde(rename = "@dynamics")]
    pub dynamics: Option<NonNegativeDecimal>,
    #[serde(rename = "@end-dynamics")]
    pub end_dynamics: Option<NonNegativeDecimal>,
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
    pub measure: Option<String>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/barline/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Barline {
    #[serde(rename = "@location")]
    pub location: Option<String>,
    pub bar_style: Option<String>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/backup/
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Backup {
    pub duration: String,
}

pub use primitive::*;
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
