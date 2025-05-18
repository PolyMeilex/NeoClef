use serde::{Deserialize, Serialize};

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
    Note(Note),
    Barline(Barline),
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
#[derive(Debug, Serialize, Deserialize)]
pub struct Note {
    #[serde(rename = "@attack")]
    pub attack: Option<String>,
    #[serde(rename = "@color")]
    pub color: Option<String>,
    #[serde(rename = "@default-x")]
    pub default_x: Option<String>,
    #[serde(rename = "@default-y")]
    pub default_y: Option<String>,
    #[serde(rename = "@dynamics")]
    pub dynamics: Option<String>,
    #[serde(rename = "@end-dynamics")]
    pub end_dynamics: Option<String>,
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
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/pitch/
#[derive(Debug, Serialize, Deserialize)]
pub struct Pitch {
    pub step: String,
    pub alter: Option<String>,
    pub octave: String,
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
