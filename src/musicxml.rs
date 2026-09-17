#![allow(dead_code, unused)]

use std::{borrow::Cow, str::FromStr};

use log::error;
use quick_xml::events::BytesStart;

/// https://w3c.github.io/musicxml/musicxml-reference/elements/score-partwise/
#[derive(Debug)]
pub struct ScorePartwise {
    pub work: Option<Work>,
    pub movement_number: Option<String>,
    pub movement_title: Option<String>,
    pub identification: Option<Identification>,
    pub defaults: Option<Defaults>,
    pub credit: Vec<Credit>,
    pub part_list: PartList,
    pub part: Vec<Part>,
}

impl<'a> ParseContent<'a> for ScorePartwise {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let work = stream.optional(b"work")?;
        let movement_number = stream.optional(b"movement-number")?;
        let movement_title = stream.optional(b"movement-title")?;
        let identification = stream.optional(b"identification")?;
        let defaults = stream.optional(b"defaults")?;
        let credit = stream.zero_or_more(b"credit")?;
        let part_list = stream.required(b"part-list")?;
        let part = stream.one_or_more(b"part")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            work,
            movement_number,
            movement_title,
            identification,
            defaults,
            credit,
            part_list,
            part,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/work/
#[derive(Debug, Default)]
pub struct Work {
    pub number: Option<String>,
    pub title: Option<String>,
}

impl<'a> ParseContent<'a> for Work {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let number = stream.optional(b"work-number")?;
        let title = stream.optional(b"work-title")?;
        // A link to a separate MusicXML opus document; not meaningful outside
        // that multi-score-collection workflow, so discarded.
        let _opus = stream.optional::<()>(b"opus")?;

        stream.skip_to_end(tag)?;

        Ok(Self { number, title })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/credit/
#[derive(Debug, Default)]
pub struct Credit {
    pub kind: Vec<String>,
    pub words: Vec<String>,
}

impl<'a> ParseContent<'a> for Credit {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let kind = stream.zero_or_more(b"credit-type")?;

        let mut words = Vec::new();
        while let Some(child) = stream.peek_tag()? {
            match child {
                b"credit-words" => words.push(stream.required(b"credit-words")?),
                b"link" | b"bookmark" | b"credit-image" | b"credit-symbol" => {
                    let child = child.to_vec();
                    stream.required::<()>(&child)?;
                }
                _ => break,
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self { kind, words })
    }
}

#[derive(Debug, Clone)]
pub struct TypedText {
    pub kind: Option<String>,
    pub text: String,
}

impl<'a> ParseContent<'a> for TypedText {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            if attr.key.as_ref() == b"type" {
                kind = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned());
            }
        }

        let text = String::parse(stream, tag, start)?;

        Ok(Self { kind, text })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/miscellaneous-field/
#[derive(Debug, Clone)]
pub struct MiscellaneousField {
    pub name: String,
    pub value: String,
}

impl<'a> ParseContent<'a> for MiscellaneousField {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut name = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            if attr.key.as_ref() == b"name" {
                name = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned());
            }
        }

        let value = String::parse(stream, tag, start)?;

        Ok(Self {
            name: name.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement(
                    "miscellaneous-field/@name".into(),
                ))
            })?,
            value,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/miscellaneous/
#[derive(Debug, Default)]
pub struct Miscellaneous {
    pub field: Vec<MiscellaneousField>,
}

impl<'a> ParseContent<'a> for Miscellaneous {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let field = stream.zero_or_more(b"miscellaneous-field")?;

        stream.skip_to_end(tag)?;

        Ok(Self { field })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/identification/
#[derive(Debug)]
pub struct Identification {
    pub creator: Vec<TypedText>,
    pub rights: Vec<TypedText>,
    pub encoding: Option<Encoding>,
    pub source: Option<String>,
    pub relation: Vec<TypedText>,
    pub miscellaneous: Option<Miscellaneous>,
}

impl<'a> ParseContent<'a> for Identification {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let creator = stream.zero_or_more(b"creator")?;
        let rights = stream.zero_or_more(b"rights")?;
        let encoding = stream.optional(b"encoding")?;
        let source = stream.optional(b"source")?;
        let relation = stream.zero_or_more(b"relation")?;
        let miscellaneous = stream.optional(b"miscellaneous")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            creator,
            rights,
            encoding,
            source,
            relation,
            miscellaneous,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/encoding/
#[derive(Debug, Default)]
pub struct Encoding {
    pub software: Vec<String>,
    pub encoding_date: Option<String>,
}

impl<'a> ParseContent<'a> for Encoding {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut this = Self::default();

        while let Some(child) = stream.peek_tag()? {
            match child {
                b"software" => this.software.push(stream.required(b"software")?),
                b"encoding-date" => this.encoding_date = stream.optional(b"encoding-date")?,
                b"encoder" | b"encoding-description" | b"supports" => {
                    let child = child.to_vec();
                    stream.required::<()>(&child)?;
                }
                _ => break,
            }
        }

        stream.skip_to_end(tag)?;

        Ok(this)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/defaults/
#[derive(Debug)]
pub struct Defaults {
    pub scaling: Option<Scaling>,
    pub page_layout: Option<PageLayout>,
    pub appearance: Option<Appearance>,
    pub music_font: Option<Font>,
    pub word_font: Option<Font>,
    pub lyric_font: Vec<LyricFont>,
    pub lyric_language: Vec<LyricLanguage>,
}

impl<'a> ParseContent<'a> for Defaults {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let scaling = stream.optional(b"scaling")?;
        let _concert_score = stream.optional::<()>(b"concert-score")?;
        let page_layout = stream.optional(b"page-layout")?;
        let _system_layout = stream.optional::<()>(b"system-layout")?;
        let _staff_layout = stream.zero_or_more::<()>(b"staff-layout")?;
        let appearance = stream.optional(b"appearance")?;
        let music_font = stream.optional(b"music-font")?;
        let word_font = stream.optional(b"word-font")?;
        let lyric_font = stream.zero_or_more(b"lyric-font")?;
        let lyric_language = stream.zero_or_more(b"lyric-language")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            scaling,
            page_layout,
            appearance,
            music_font,
            word_font,
            lyric_font,
            lyric_language,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Font {
    pub family: Option<String>,
    pub style: Option<String>,
    pub size: Option<String>,
    pub weight: Option<String>,
}

impl Font {
    fn from_start(start: &BytesStart) -> Self {
        let mut this = Self::default();
        for attr in start.attributes().filter_map(|r| r.ok()) {
            let value = || String::from_utf8_lossy(attr.value.as_ref()).into_owned();
            match attr.key.as_ref() {
                b"font-family" => this.family = Some(value()),
                b"font-style" => this.style = Some(value()),
                b"font-size" => this.size = Some(value()),
                b"font-weight" => this.weight = Some(value()),
                _ => {}
            }
        }
        this
    }
}

impl<'a> ParseContent<'a> for Font {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let this = Self::from_start(start);
        stream.skip_to_end(tag)?;
        Ok(this)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/lyric-font/
#[derive(Debug, Clone)]
pub struct LyricFont {
    pub number: Option<String>,
    pub name: Option<String>,
    pub font: Font,
}

impl<'a> ParseContent<'a> for LyricFont {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut number = None;
        let mut name = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"number" => {
                    number = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned())
                }
                b"name" => name = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned()),
                _ => {}
            }
        }
        let font = Font::from_start(start);

        stream.skip_to_end(tag)?;

        Ok(Self { number, name, font })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/lyric-language/
#[derive(Debug, Clone)]
pub struct LyricLanguage {
    pub number: Option<String>,
    pub name: Option<String>,
    pub lang: Option<String>,
}

impl<'a> ParseContent<'a> for LyricLanguage {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut number = None;
        let mut name = None;
        let mut lang = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"number" => {
                    number = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned())
                }
                b"name" => name = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned()),
                b"xml:lang" => {
                    lang = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned())
                }
                _ => {}
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self { number, name, lang })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/scaling/
#[derive(Debug)]
pub struct Scaling {
    pub millimeters: Decimal,
    pub tenths: Tenths,
}

impl<'a> ParseContent<'a> for Scaling {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let millimeters = stream.required(b"millimeters")?;
        let tenths = stream.required(b"tenths")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            millimeters,
            tenths,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/page-layout/
#[derive(Debug)]
pub struct PageLayout {
    pub page_height: Option<Tenths>,
    pub page_width: Option<Tenths>,
    pub page_margins: Vec<PageMargins>,
}

impl<'a> ParseContent<'a> for PageLayout {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let page_height = stream.optional(b"page-height")?;
        let page_width = stream.optional(b"page-width")?;
        let page_margins = stream.zero_or_more(b"page-margins")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            page_height,
            page_width,
            page_margins,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/page-margins/
#[derive(Debug)]
pub struct PageMargins {
    pub left_margin: Tenths,
    pub right_margin: Tenths,
    pub top_margin: Tenths,
    pub bottom_margin: Tenths,
}

impl<'a> ParseContent<'a> for PageMargins {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let left_margin = stream.required(b"left-margin")?;
        let right_margin = stream.required(b"right-margin")?;
        let top_margin = stream.required(b"top-margin")?;
        let bottom_margin = stream.required(b"bottom-margin")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            left_margin,
            right_margin,
            top_margin,
            bottom_margin,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/appearance/
#[derive(Debug)]
pub struct Appearance {
    pub line_width: Vec<Decimal>,
    pub note_size: Vec<Decimal>,
}

impl<'a> ParseContent<'a> for Appearance {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let line_width = stream.zero_or_more(b"line-width")?;
        let note_size = stream.zero_or_more(b"note-size")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            line_width,
            note_size,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/part-list/
#[derive(Debug)]
pub struct PartList {
    pub score_part: Vec<ScorePart>,
    pub part_group: Vec<PartGroup>,
}

impl<'a> ParseContent<'a> for PartList {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut score_part = Vec::new();
        let mut part_group = Vec::new();

        while let Some(child) = stream.peek_tag()? {
            match child {
                b"score-part" => score_part.push(stream.required(b"score-part")?),
                b"part-group" => part_group.push(stream.required(b"part-group")?),
                _ => break,
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            score_part,
            part_group,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/part-group/
#[derive(Debug, Clone)]
pub struct PartGroup {
    pub kind: StartStop,
    pub number: Option<String>,
    pub group_name: Option<String>,
    pub group_abbreviation: Option<String>,
    pub group_symbol: Option<String>,
    pub group_barline: Option<String>,
}

impl<'a> ParseContent<'a> for PartGroup {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind = None;
        let mut number = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"type" => kind = StartStop::parse(attr.value.as_ref()),
                b"number" => {
                    number = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned())
                }
                _ => {}
            }
        }

        let group_name = stream.optional(b"group-name")?;
        let _group_name_display = stream.optional::<()>(b"group-name-display")?;
        let group_abbreviation = stream.optional(b"group-abbreviation")?;
        let _group_abbreviation_display = stream.optional::<()>(b"group-abbreviation-display")?;
        let group_symbol = stream.optional(b"group-symbol")?;
        let group_barline = stream.optional(b"group-barline")?;
        let _group_time = stream.optional::<()>(b"group-time")?;
        let _footnote = stream.optional::<()>(b"footnote")?;
        let _level = stream.optional::<()>(b"level")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("part-group/@type".into()))
            })?,
            number,
            group_name,
            group_abbreviation,
            group_symbol,
            group_barline,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/score-part/
#[derive(Debug)]
pub struct ScorePart {
    pub part_name: String,
    pub part_abbreviation: Option<String>,
    pub score_instrument: Vec<ScoreInstrument>,
    pub midi_instrument: Vec<MidiInstrument>,
}

impl<'a> ParseContent<'a> for ScorePart {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let _identification = stream.optional::<()>(b"identification")?;
        let _part_link = stream.zero_or_more::<()>(b"part-link")?;
        let part_name = stream.required(b"part-name")?;
        let _part_name_display = stream.optional::<()>(b"part-name-display")?;
        let part_abbreviation = stream.optional(b"part-abbreviation")?;
        let _part_abbreviation_display = stream.optional::<()>(b"part-abbreviation-display")?;
        let _group = stream.zero_or_more::<()>(b"group")?;
        let score_instrument = stream.zero_or_more(b"score-instrument")?;
        let _player = stream.zero_or_more::<()>(b"player")?;

        let mut midi_instrument = Vec::new();
        while let Some(child) = stream.peek_tag()? {
            match child {
                b"midi-device" => {
                    stream.required::<()>(b"midi-device")?;
                }
                b"midi-instrument" => midi_instrument.push(stream.required(b"midi-instrument")?),
                _ => break,
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            part_name,
            part_abbreviation,
            score_instrument,
            midi_instrument,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/score-instrument/
#[derive(Debug)]
pub struct ScoreInstrument {
    pub instrument_name: String,
    pub instrument_sound: Option<String>,
}

impl<'a> ParseContent<'a> for ScoreInstrument {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let instrument_name = stream.required(b"instrument-name")?;
        let _instrument_abbreviation = stream.optional::<()>(b"instrument-abbreviation")?;
        let instrument_sound = stream.optional(b"instrument-sound")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            instrument_name,
            instrument_sound,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/midi-instrument/
#[derive(Debug)]
pub struct MidiInstrument {
    pub midi_channel: Option<Midi16>,
    pub midi_program: Option<Midi128>,
    pub volume: Option<Percent>,
    pub pan: Option<RotationDegrees>,
}

impl<'a> ParseContent<'a> for MidiInstrument {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let midi_channel = stream.optional(b"midi-channel")?;
        let _midi_name = stream.optional::<()>(b"midi-name")?;
        let _midi_bank = stream.optional::<()>(b"midi-bank")?;
        let midi_program = stream.optional(b"midi-program")?;
        let _midi_unpitched = stream.optional::<()>(b"midi-unpitched")?;
        let volume = stream.optional(b"volume")?;
        let pan = stream.optional(b"pan")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            midi_channel,
            midi_program,
            volume,
            pan,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/part-partwise/
#[derive(Debug)]
pub struct Part {
    pub measure: Vec<Measure>,
}

impl<'a> ParseContent<'a> for Part {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let measure = stream.one_or_more(b"measure")?;

        stream.skip_to_end(tag)?;

        Ok(Self { measure })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/measure-partwise/
#[derive(Debug)]
pub struct Measure {
    pub content: Vec<MeasureItem>,
}

impl<'a> ParseContent<'a> for Measure {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut content = Vec::new();
        while let Some(tag) = stream.peek_tag()? {
            match tag {
                b"print" => {
                    content.push(MeasureItem::Print(stream.required(b"print")?));
                }
                b"attributes" => {
                    content.push(MeasureItem::Attributes(stream.required(b"attributes")?));
                }
                b"note" => {
                    content.push(MeasureItem::Note(stream.required(b"note")?));
                }
                b"barline" => {
                    content.push(MeasureItem::Barline(stream.required(b"barline")?));
                }
                b"backup" => {
                    content.push(MeasureItem::Backup(stream.required(b"backup")?));
                }
                b"forward" => {
                    content.push(MeasureItem::Forward(stream.required(b"forward")?));
                }
                b"direction" => {
                    content.push(MeasureItem::Direction(stream.required(b"direction")?));
                }
                b"harmony" => {
                    content.push(MeasureItem::Harmony(stream.required(b"harmony")?));
                }
                tag => {
                    let tag = tag.to_vec();
                    stream.required::<()>(&tag)?;
                }
            }
        }

        stream.skip_to_end(tag)?;

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
    Forward(Forward),
    Direction(Direction),
    Harmony(Harmony),
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/print/
#[derive(Debug)]
pub struct Print {
    pub system_layout: Option<SystemLayout>,
    pub staff_layout: Vec<StaffLayout>,
}

impl<'a> ParseContent<'a> for Print {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let _page_layout = stream.optional::<()>(b"page-layout")?;
        let system_layout = stream.optional(b"system-layout")?;
        let staff_layout = stream.zero_or_more(b"staff-layout")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            system_layout,
            staff_layout,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/system-layout/
#[derive(Debug)]
pub struct SystemLayout {
    pub system_margins: Option<SystemMargins>,
    pub system_distance: Option<Tenths>,
    pub top_system_distance: Option<Tenths>,
}

impl<'a> ParseContent<'a> for SystemLayout {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let system_margins = stream.optional(b"system-margins")?;
        let system_distance = stream.optional(b"system-distance")?;
        let top_system_distance = stream.optional(b"top-system-distance")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            system_margins,
            system_distance,
            top_system_distance,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/system-margins/
#[derive(Debug)]
pub struct SystemMargins {
    pub left_margin: Tenths,
    pub right_margin: Tenths,
}

impl<'a> ParseContent<'a> for SystemMargins {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let left_margin = stream.required(b"left-margin")?;
        let right_margin = stream.required(b"right-margin")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            left_margin,
            right_margin,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/staff-layout/
#[derive(Debug)]
pub struct StaffLayout {
    pub staff_distance: Option<Tenths>,
}

impl<'a> ParseContent<'a> for StaffLayout {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let staff_distance = stream.optional(b"staff-distance")?;

        stream.skip_to_end(tag)?;

        Ok(Self { staff_distance })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/attributes/
#[derive(Debug)]
pub struct Attributes {
    pub divisions: Option<PositiveDivisions>,
    pub key: Vec<Key>,
    pub time: Vec<Time>,
    pub clef: Vec<Clef>,
    pub measure_style: Vec<MeasureStyle>,
}

impl<'a> ParseContent<'a> for Attributes {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let _footnote = stream.optional::<()>(b"footnote")?;
        let _level = stream.optional::<()>(b"level")?;
        let divisions = stream.optional(b"divisions")?;
        let key = stream.zero_or_more(b"key")?;
        let time = stream.zero_or_more(b"time")?;
        let _staves = stream.optional::<()>(b"staves")?;
        let _part_symbol = stream.optional::<()>(b"part-symbol")?;
        let _instruments = stream.optional::<()>(b"instruments")?;
        let clef = stream.zero_or_more(b"clef")?;
        let measure_style = stream.zero_or_more(b"measure-style")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            divisions,
            key,
            time,
            clef,
            measure_style,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/barline/
#[derive(Debug)]
pub struct Barline {
    pub bar_style: Option<String>,
    pub wavy_line: Option<WavyLine>,
    /// Presence of a `<segno>`/`<coda>` symbol on this barline (visual marker;
    /// the playback jump target itself lives in the `sound`/`barline` attrs).
    pub segno: bool,
    pub coda: bool,
    pub fermata: Vec<Fermata>,
    pub ending: Option<Ending>,
    pub repeat: Option<Repeat>,
}

impl<'a> ParseContent<'a> for Barline {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let bar_style = stream.optional(b"bar-style")?;
        let _footnote = stream.optional::<()>(b"footnote")?;
        let _level = stream.optional::<()>(b"level")?;
        let wavy_line = stream.optional(b"wavy-line")?;
        let segno = stream.optional::<()>(b"segno")?.is_some();
        let coda = stream.optional::<()>(b"coda")?.is_some();
        let fermata = stream.zero_or_more(b"fermata")?;
        let ending = stream.optional(b"ending")?;
        let repeat = stream.optional(b"repeat")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            bar_style,
            wavy_line,
            segno,
            coda,
            fermata,
            ending,
            repeat,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/start-stop-continue/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartStopContinue {
    Start,
    Stop,
    Continue,
}

impl StartStopContinue {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v = match bytes {
            b"start" => StartStopContinue::Start,
            b"stop" => StartStopContinue::Stop,
            b"continue" => StartStopContinue::Continue,
            other => {
                error!("Unexpected start-stop-continue: {other:?}");
                return None;
            }
        };
        Some(v)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/wavy-line/
#[derive(Debug, Clone)]
pub struct WavyLine {
    pub kind: StartStopContinue,
}

impl<'a> ParseContent<'a> for WavyLine {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            if attr.key.as_ref() == b"type" {
                kind = StartStopContinue::parse(attr.value.as_ref());
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("wavy-line/@type".into()))
            })?,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/fermata/
#[derive(Debug, Clone)]
pub struct Fermata {
    pub shape: FermataShape,
    /// Upright if not specified.
    pub kind: Option<UprightInverted>,
}

impl<'a> ParseContent<'a> for Fermata {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            if attr.key.as_ref() == b"type" {
                kind = UprightInverted::parse(attr.value.as_ref());
            }
        }

        let shape_text = String::parse(stream, tag, start)?;
        let shape = shape_text
            .parse()
            .map_err(|e| stream.error(parser::ErrorKind::UnexpectedValue(e)))?;

        Ok(Self { shape, kind })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/fermata-shape/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FermataShape {
    /// An empty `<fermata/>` also means Normal.
    Normal,
    Angled,
    Square,
    DoubleAngled,
    DoubleSquare,
    DoubleDot,
    HalfCurve,
    Curlew,
}

impl FromStr for FermataShape {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = match s.trim() {
            "" | "normal" => FermataShape::Normal,
            "angled" => FermataShape::Angled,
            "square" => FermataShape::Square,
            "double-angled" => FermataShape::DoubleAngled,
            "double-square" => FermataShape::DoubleSquare,
            "double-dot" => FermataShape::DoubleDot,
            "half-curve" => FermataShape::HalfCurve,
            "curlew" => FermataShape::Curlew,
            other => return Err(format!("unknown fermata shape: {}", other)),
        };
        Ok(v)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/upright-inverted/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UprightInverted {
    Upright,
    Inverted,
}

impl UprightInverted {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v = match bytes {
            b"upright" => UprightInverted::Upright,
            b"inverted" => UprightInverted::Inverted,
            other => {
                error!("Unexpected upright-inverted: {other:?}");
                return None;
            }
        };
        Some(v)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/start-stop-discontinue/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartStopDiscontinue {
    Start,
    Stop,
    Discontinue,
}

impl StartStopDiscontinue {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v = match bytes {
            b"start" => StartStopDiscontinue::Start,
            b"stop" => StartStopDiscontinue::Stop,
            b"discontinue" => StartStopDiscontinue::Discontinue,
            other => {
                error!("Unexpected start-stop-discontinue: {other:?}");
                return None;
            }
        };
        Some(v)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/ending/
#[derive(Debug, Clone)]
pub struct Ending {
    pub number: String,
    pub kind: StartStopDiscontinue,
    /// Visible ending text, when different from `number` (e.g. "1." vs "1").
    pub text: String,
}

impl<'a> ParseContent<'a> for Ending {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut number = None;
        let mut kind = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"number" => {
                    number = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned())
                }
                b"type" => kind = StartStopDiscontinue::parse(attr.value.as_ref()),
                _ => {}
            }
        }

        let text = String::parse(stream, tag, start)?;

        Ok(Self {
            number: number.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("ending/@number".into()))
            })?,
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("ending/@type".into()))
            })?,
            text,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/backward-forward/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackwardForward {
    Backward,
    Forward,
}

impl BackwardForward {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v = match bytes {
            b"backward" => BackwardForward::Backward,
            b"forward" => BackwardForward::Forward,
            other => {
                error!("Unexpected backward-forward: {other:?}");
                return None;
            }
        };
        Some(v)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/repeat/
#[derive(Debug, Clone)]
pub struct Repeat {
    pub direction: BackwardForward,
    pub times: Option<u32>,
}

impl<'a> ParseContent<'a> for Repeat {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut direction = None;
        let mut times = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"direction" => direction = BackwardForward::parse(attr.value.as_ref()),
                b"times" => times = parse_str_as(&attr.value),
                _ => {}
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            direction: direction.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement(
                    "repeat/@direction".into(),
                ))
            })?,
            times,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/backup/
#[derive(Debug)]
pub struct Backup {
    pub duration: PositiveDivisions,
}

impl<'a> ParseContent<'a> for Backup {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let duration = stream.required(b"duration")?;

        stream.skip_to_end(tag)?;

        Ok(Self { duration })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/forward/
#[derive(Debug)]
pub struct Forward {
    pub duration: PositiveDivisions,
    pub voice: Option<String>,
    pub staff: Option<u8>,
}

impl<'a> ParseContent<'a> for Forward {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let duration = stream.required(b"duration")?;
        let _footnote = stream.optional::<()>(b"footnote")?;
        let _level = stream.optional::<()>(b"level")?;
        let voice = stream.optional(b"voice")?;
        let staff = stream.optional(b"staff")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            duration,
            voice,
            staff,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/root/
#[derive(Debug, Clone)]
pub struct Root {
    pub step: Step,
    pub alter: Option<Semitones>,
}

impl<'a> ParseContent<'a> for Root {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let step = stream.required(b"root-step")?;
        let alter = stream.optional(b"root-alter")?;

        stream.skip_to_end(tag)?;

        Ok(Self { step, alter })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/bass/
#[derive(Debug, Clone)]
pub struct Bass {
    pub step: Step,
    pub alter: Option<Semitones>,
}

impl<'a> ParseContent<'a> for Bass {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let _bass_separator = stream.optional::<()>(b"bass-separator")?;
        let step = stream.required(b"bass-step")?;
        let alter = stream.optional(b"bass-alter")?;

        stream.skip_to_end(tag)?;

        Ok(Self { step, alter })
    }
}

#[derive(Debug, Clone)]
pub enum HarmonyChordRoot {
    Root(Root),
    // TODO
    Numeral,
    // TODO
    Function,
}

/// The `harmony-chord` group: one chord within a (possibly stacked) harmony.
#[derive(Debug, Clone)]
pub struct HarmonyChord {
    pub root: HarmonyChordRoot,
    pub kind: HarmonyKind,
    pub bass: Option<Bass>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/kind-value/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmonyKind {
    Major,
    Minor,
    Augmented,
    Diminished,
    Dominant,
    MajorSeventh,
    MinorSeventh,
    DiminishedSeventh,
    AugmentedSeventh,
    HalfDiminished,
    MajorMinor,
    MajorSixth,
    MinorSixth,
    DominantNinth,
    MajorNinth,
    MinorNinth,
    Dominant11th,
    Major11th,
    Minor11th,
    Dominant13th,
    Major13th,
    Minor13th,
    SuspendedSecond,
    SuspendedFourth,
    Neapolitan,
    Italian,
    French,
    German,
    Pedal,
    Power,
    Tristan,
    /// A chord type not covered by the standard values; the `text` attribute
    /// (discarded here) carries the display spelling in that case.
    Other,
    /// No chord.
    None,
}

impl<'a> ParseContent<'a> for HarmonyKind {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let v = Cow::<str>::parse(stream, tag, start)?;
        v.parse::<Self>()
            .map_err(|e| stream.error(parser::ErrorKind::UnexpectedValue(e)))
    }
}

impl FromStr for HarmonyKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v = match s {
            "major" => HarmonyKind::Major,
            "minor" => HarmonyKind::Minor,
            "augmented" => HarmonyKind::Augmented,
            "diminished" => HarmonyKind::Diminished,
            "dominant" => HarmonyKind::Dominant,
            "major-seventh" => HarmonyKind::MajorSeventh,
            "minor-seventh" => HarmonyKind::MinorSeventh,
            "diminished-seventh" => HarmonyKind::DiminishedSeventh,
            "augmented-seventh" => HarmonyKind::AugmentedSeventh,
            "half-diminished" => HarmonyKind::HalfDiminished,
            "major-minor" => HarmonyKind::MajorMinor,
            "major-sixth" => HarmonyKind::MajorSixth,
            "minor-sixth" => HarmonyKind::MinorSixth,
            "dominant-ninth" => HarmonyKind::DominantNinth,
            "major-ninth" => HarmonyKind::MajorNinth,
            "minor-ninth" => HarmonyKind::MinorNinth,
            "dominant-11th" => HarmonyKind::Dominant11th,
            "major-11th" => HarmonyKind::Major11th,
            "minor-11th" => HarmonyKind::Minor11th,
            "dominant-13th" => HarmonyKind::Dominant13th,
            "major-13th" => HarmonyKind::Major13th,
            "minor-13th" => HarmonyKind::Minor13th,
            "suspended-second" => HarmonyKind::SuspendedSecond,
            "suspended-fourth" => HarmonyKind::SuspendedFourth,
            "Neapolitan" => HarmonyKind::Neapolitan,
            "Italian" => HarmonyKind::Italian,
            "French" => HarmonyKind::French,
            "German" => HarmonyKind::German,
            "pedal" => HarmonyKind::Pedal,
            "power" => HarmonyKind::Power,
            "Tristan" => HarmonyKind::Tristan,
            "other" => HarmonyKind::Other,
            "none" => HarmonyKind::None,
            other => return Err(format!("unknown harmony kind: {}", other)),
        };
        Ok(v)
    }
}

impl<'a> ParseContentFlat<'a> for HarmonyChord {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, parser::Error> {
        let root = match stream.peek_tag()? {
            Some(b"root") => HarmonyChordRoot::Root(stream.required(b"root")?),
            Some(b"numeral") => {
                stream.required::<()>(b"numeral")?;
                HarmonyChordRoot::Numeral
            }
            Some(b"function") => {
                stream.required::<()>(b"function")?;
                HarmonyChordRoot::Function
            }
            Some(other) => {
                let msg = format!(
                    "Invalid harmony-chord root: {:?}",
                    String::from_utf8_lossy(other)
                );
                return Err(stream.error(parser::ErrorKind::UnexpectedEvent(msg)));
            }
            None => {
                return Err(stream.error(parser::ErrorKind::MissingElement(
                    "Expected root/numeral/function in harmony-chord".into(),
                )));
            }
        };

        let kind = stream.required(b"kind")?;
        let _inversion = stream.optional::<()>(b"inversion")?;
        let bass = stream.optional(b"bass")?;
        let _degree = stream.zero_or_more::<()>(b"degree")?;

        Ok(Self { root, kind, bass })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/harmony/
#[derive(Debug)]
pub struct Harmony {
    /// Usually a single chord; more than one when stacked (e.g. "V of II").
    pub chords: Vec<HarmonyChord>,
    pub staff: Option<u8>,
}

impl<'a> ParseContent<'a> for Harmony {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut chords = Vec::new();
        while let Some(b"root" | b"numeral" | b"function") = stream.peek_tag()? {
            chords.push(stream.flatten()?);
        }

        // Fretboard diagram / positioning offset; visual only.
        let _frame = stream.optional::<()>(b"frame")?;
        let _offset = stream.optional::<()>(b"offset")?;
        let _footnote = stream.optional::<()>(b"footnote")?;
        let _level = stream.optional::<()>(b"level")?;
        let staff = stream.optional(b"staff")?;

        stream.skip_to_end(tag)?;

        Ok(Self { chords, staff })
    }
}

/// https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/direction/
#[derive(Debug)]
pub struct Direction {
    pub direction_type: Vec<DirectionType>,
    pub sound: Option<Sound>,
}

impl<'a> ParseContent<'a> for Direction {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let direction_type = stream.one_or_more(b"direction-type")?;
        let _offset = stream.optional::<()>(b"offset")?;
        let _footnote = stream.optional::<()>(b"footnote")?;
        let _level = stream.optional::<()>(b"level")?;
        let _voice = stream.optional::<()>(b"voice")?;
        let _staff = stream.optional::<()>(b"staff")?;
        let sound = stream.optional(b"sound")?;
        let _listening = stream.optional::<()>(b"listening")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            direction_type,
            sound,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/direction-type/
#[derive(Debug)]
pub enum DirectionType {
    Metronome(Metronome),
    /// `words` (a standard text direction, e.g. "rit.", "Allegro") — a
    /// `direction-type` can hold a repeated run of them.
    Words(Vec<String>),
    /// `dynamics` (e.g. `mf`, `sfz`) attached to a direction rather than a
    /// note — a `direction-type` can hold a repeated run of them too.
    Dynamics(Vec<Dynamics>),
    Other,
}

impl<'a> ParseContent<'a> for DirectionType {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let this = match stream.peek_tag()? {
            Some(b"metronome") => DirectionType::Metronome(stream.required(b"metronome")?),
            Some(b"words") => {
                let mut words = Vec::new();
                while let Some(b"words") = stream.peek_tag()? {
                    words.push(stream.required(b"words")?);
                }
                DirectionType::Words(words)
            }
            Some(b"dynamics") => {
                let mut dynamics = Vec::new();
                while let Some(b"dynamics") = stream.peek_tag()? {
                    dynamics.push(stream.required(b"dynamics")?);
                }
                DirectionType::Dynamics(dynamics)
            }
            Some(other) => {
                let other = other.to_vec();
                stream.required::<()>(&other)?;
                DirectionType::Other
            }
            None => DirectionType::Other,
        };

        stream.skip_to_end(tag)?;

        Ok(this)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/metronome/
#[derive(Debug)]
pub struct Metronome {
    pub beat_unit: String,
    pub per_minute: Option<String>,
}

impl<'a> ParseContent<'a> for Metronome {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let beat_unit = stream.required(b"beat-unit")?;
        let _beat_unit_dot = stream.zero_or_more::<()>(b"beat-unit-dot")?;
        let per_minute = stream.optional(b"per-minute")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            beat_unit,
            per_minute,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/sound/
#[derive(Debug)]
pub struct Sound {
    pub tempo: Option<NonNegativeDecimal>,
}

impl<'a> ParseContent<'a> for Sound {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut tempo: Option<NonNegativeDecimal> = None;

        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"tempo" => tempo = parse_str_as(&attr.value),
                _ => {}
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self { tempo })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/key/
#[derive(Debug)]
pub struct Key {
    pub kind: KeyKind,
    pub key_octave: Vec<()>,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/key/
#[derive(Debug)]
pub enum KeyKind {
    Traditional(TraditionalKey),
    NonTraditional(Vec<NonTraditionalKey>),
}

/// The `traditional-key` group: a key signature using the cycle of fifths.
#[derive(Debug)]
pub struct TraditionalKey {
    pub cancel: Option<()>,
    pub fifths: Fifths,
    pub mode: Option<String>,
}

/// A single altered tone within a non-traditional key signature (`non-traditional-key` group).
#[derive(Debug)]
pub struct NonTraditionalKey {
    pub step: Step,
    pub alter: Semitones,
    pub accidental: Option<String>,
}

impl<'a> ParseContentFlat<'a> for NonTraditionalKey {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, parser::Error> {
        let step = stream.required(b"key-step")?;
        let alter = stream.required(b"key-alter")?;
        let accidental = stream.optional(b"key-accidental")?;

        Ok(Self {
            step,
            alter,
            accidental,
        })
    }
}

impl<'a> ParseContent<'a> for Key {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let kind = match stream.peek_tag()? {
            Some(b"key-step") => {
                let mut items = Vec::new();
                while let Some(b"key-step") = stream.peek_tag()? {
                    items.push(stream.flatten()?);
                }
                KeyKind::NonTraditional(items)
            }
            _ => {
                let cancel = stream.optional::<()>(b"cancel")?;
                let fifths = stream.required(b"fifths")?;
                let mode = stream.optional(b"mode")?;
                KeyKind::Traditional(TraditionalKey {
                    cancel,
                    fifths,
                    mode,
                })
            }
        };

        let key_octave = stream.zero_or_more::<()>(b"key-octave")?;

        stream.skip_to_end(tag)?;

        Ok(Self { kind, key_octave })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/time/
#[derive(Debug, PartialEq)]
pub struct Time {
    pub kind: TimeKind,
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/time/
#[derive(Debug, PartialEq)]
pub enum TimeKind {
    Signature {
        signatures: Vec<TimeSignature>,
        interchangeable: Option<Interchangeable>,
    },
    SenzaMisura(String),
}

/// The `time-signature` group: a single beats/beat-type pair.
#[derive(Debug, PartialEq)]
pub struct TimeSignature {
    pub beats: String,
    pub beat_type: String,
}

impl<'a> ParseContentFlat<'a> for TimeSignature {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, parser::Error> {
        let beats = stream.required(b"beats")?;
        let beat_type = stream.required(b"beat-type")?;

        Ok(Self { beats, beat_type })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/interchangeable/
#[derive(Debug, PartialEq)]
pub struct Interchangeable {
    pub time_relation: Option<String>,
    pub time_signature: Vec<TimeSignature>,
}

impl<'a> ParseContent<'a> for Interchangeable {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let time_relation = stream.optional(b"time-relation")?;

        let mut time_signature = Vec::new();
        while let Some(b"beats") = stream.peek_tag()? {
            time_signature.push(stream.flatten()?);
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            time_relation,
            time_signature,
        })
    }
}

impl<'a> ParseContent<'a> for Time {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let kind = match stream.peek_tag()? {
            Some(b"senza-misura") => TimeKind::SenzaMisura(stream.required(b"senza-misura")?),
            _ => {
                let mut signatures = Vec::new();
                while let Some(b"beats") = stream.peek_tag()? {
                    signatures.push(stream.flatten()?);
                }
                let interchangeable = stream.optional(b"interchangeable")?;
                TimeKind::Signature {
                    signatures,
                    interchangeable,
                }
            }
        };

        stream.skip_to_end(tag)?;

        Ok(Self { kind })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/clef/
#[derive(Debug)]
pub struct Clef {
    pub sign: ClefSign,
    pub line: Option<StaffLinePosition>,
    pub clef_octave_change: Option<i32>,
}

impl<'a> ParseContent<'a> for Clef {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let sign = stream.required(b"sign")?;
        let line = stream.optional(b"line")?;
        let clef_octave_change = stream.optional(b"clef-octave-change")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            sign,
            line,
            clef_octave_change,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/measure-style/
#[derive(Debug, Clone)]
pub enum MeasureStyle {
    /// Number of measures covered by a multi-measure rest.
    MultipleRest(u32),
    /// `measure-repeat`, `beat-repeat`, or `slash` — recognized but not
    /// modeled (no score in the reference test corpus uses them).
    Other,
}

impl<'a> ParseContent<'a> for MeasureStyle {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let this = match stream.peek_tag()? {
            Some(b"multiple-rest") => {
                MeasureStyle::MultipleRest(stream.required(b"multiple-rest")?)
            }
            Some(other) => {
                let other = other.to_vec();
                stream.required::<()>(&other)?;
                MeasureStyle::Other
            }
            None => MeasureStyle::Other,
        };

        stream.skip_to_end(tag)?;

        Ok(this)
    }
}

#[derive(Debug, Clone)]
pub enum NoteKindStatePitchKind {
    Pitch(Pitch),
    Unpitched(String),
    Rest(Rest),
}

impl<'a> ParseContentFlat<'a> for NoteKindStatePitchKind {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, crate::parser::Error> {
        let kind = match stream.peek_tag()? {
            Some(b"pitch") => Self::Pitch(stream.required(b"pitch")?),
            Some(b"unpitched") => Self::Unpitched(stream.required(b"unpitched")?),
            Some(b"rest") => Self::Rest(stream.required(b"rest")?),
            Some(unknown) => {
                let msg = format!("Invalid choice: {:?}", String::from_utf8_lossy(unknown));
                return Err(stream.error(parser::ErrorKind::UnexpectedEvent(msg)));
            }
            None => {
                return Err(stream.error(parser::ErrorKind::MissingElement(
                    "Expected choice element in note".into(),
                )));
            }
        };

        Ok(kind)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/note/
#[derive(Debug, Clone)]
pub struct NoteKindStateGrace {
    pub chord: Option<Chord>,
    pub kind: NoteKindStatePitchKind,
    // 0 to 2 times
    pub tie: Vec<Tie>,
}

#[derive(Debug, Clone)]
pub struct NoteKindStateGraceCue {
    pub chord: Option<Chord>,
    pub kind: NoteKindStatePitchKind,
}

#[derive(Debug)]
enum GraceOrGraceCue {
    Grace(NoteKindStateGrace),
    GraceCue(NoteKindStateGraceCue),
}

impl<'a> ParseContentFlat<'a> for GraceOrGraceCue {
    fn parse(stream: &mut XmlStream<'a>) -> std::prelude::v1::Result<Self, parser::Error> {
        let _grace: () = stream.required(b"grace")?;
        match stream.optional::<()>(b"cue")? {
            Some(_) => {
                let chord = stream.optional(b"chord")?;
                let kind = stream.flatten()?;
                Ok(Self::GraceCue(NoteKindStateGraceCue { chord, kind }))
            }
            None => {
                let chord = stream.optional(b"chord")?;
                let kind = stream.flatten()?;
                let tie = stream.zero_or_more(b"tie")?;
                Ok(Self::Grace(NoteKindStateGrace { chord, kind, tie }))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct NoteKindStateCue {
    pub chord: Option<Chord>,
    pub kind: NoteKindStatePitchKind,
    pub duration: PositiveDivisions,
    // Not valid per the strict `note` content model (cue notes have no tie),
    // but real-world exporters (e.g. MuseScore) emit it on cue notes anyway
    // to preserve tie continuation across a cue passage.
    pub tie: Vec<Tie>,
}

impl<'a> ParseContentFlat<'a> for NoteKindStateCue {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, parser::Error> {
        let _cue: () = stream.required(b"cue")?;
        let chord = stream.optional(b"chord")?;
        let kind = stream.flatten()?;
        let duration = stream.required(b"duration")?;
        let tie = stream.zero_or_more(b"tie")?;
        Ok(Self {
            chord,
            kind,
            duration,
            tie,
        })
    }
}

#[derive(Debug, Clone)]
pub struct NoteKindStateRegular {
    pub chord: Option<Chord>,
    pub kind: NoteKindStatePitchKind,
    pub duration: PositiveDivisions,
    // 0 to 2 times
    pub tie: Vec<Tie>,
}

impl<'a> ParseContentFlat<'a> for NoteKindStateRegular {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, parser::Error> {
        let chord = stream.optional(b"chord")?;
        let kind = stream.flatten()?;
        let duration = stream.required(b"duration")?;
        let tie = stream.zero_or_more(b"tie")?;

        Ok(Self {
            chord,
            kind,
            duration,
            tie,
        })
    }
}

#[derive(Debug, Clone)]
pub enum NoteKindState {
    Grace(NoteKindStateGrace),
    GraceCue(NoteKindStateGraceCue),
    Cue(NoteKindStateCue),
    Regular(NoteKindStateRegular),
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/note/
#[derive(Debug)]
pub struct Note {
    pub kind: NoteKindState,
    pub voice: Option<String>,
    pub note_type: Option<String>,
    pub stem: Option<String>,
    pub dot: usize,
    pub accidental: Option<String>,
    pub time_modification: Option<TimeModification>,
    pub staff: Option<u8>,
    pub beam: Vec<String>,
    pub notations: Vec<Notations>,
}

impl<'a> ParseContent<'a> for Note {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, crate::parser::Error> {
        let kind = match stream.peek_tag()? {
            Some(b"grace") => match stream.flatten()? {
                GraceOrGraceCue::Grace(v) => NoteKindState::Grace(v),
                GraceOrGraceCue::GraceCue(v) => NoteKindState::GraceCue(v),
            },
            Some(b"cue") => NoteKindState::Cue(stream.flatten()?),
            _ => NoteKindState::Regular(stream.flatten()?),
        };

        let _instrument = stream.zero_or_more::<()>(b"instrument")?;
        let _footnote = stream.optional::<()>(b"footnote")?;
        let _level = stream.optional::<()>(b"level")?;
        let voice = stream.optional(b"voice")?;
        let note_type = stream.optional(b"type")?;
        let dot = stream.zero_or_more::<()>(b"dot")?.len();
        let accidental = stream.optional(b"accidental")?;
        let time_modification = stream.optional(b"time-modification")?;
        let stem = stream.optional(b"stem")?;
        let _notehead = stream.optional::<()>(b"notehead")?;
        let _notehead_text = stream.optional::<()>(b"notehead-text")?;
        let staff = stream.optional(b"staff")?;
        let beam = stream.zero_or_more(b"beam")?;
        let notations = stream.zero_or_more(b"notations")?;
        let _lyric = stream.zero_or_more::<()>(b"lyric")?;
        let _play = stream.optional::<()>(b"play")?;
        let _listen = stream.optional::<()>(b"listen")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            kind,
            voice,
            note_type,
            stem,
            dot,
            accidental,
            time_modification,
            staff,
            beam,
            notations,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/time-modification/
#[derive(Debug, Clone)]
pub struct TimeModification {
    pub actual_notes: u32,
    pub normal_notes: u32,
}

impl<'a> ParseContent<'a> for TimeModification {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let actual_notes = stream.required(b"actual-notes")?;
        let normal_notes = stream.required(b"normal-notes")?;
        let _normal_type = stream.optional::<()>(b"normal-type")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            actual_notes,
            normal_notes,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/notations/
#[derive(Debug, Default)]
pub struct Notations {
    pub tied: Vec<Tied>,
    pub slurs: Vec<Slur>,
    pub tuplet: Vec<Tuplet>,
    pub glissando: Vec<Glissando>,
    pub slide: Vec<Slide>,
    pub ornaments: Vec<Ornaments>,
    pub technical: Vec<Technical>,
    pub articulations: Vec<Articulations>,
    pub dynamics: Vec<Dynamics>,
    pub fermata: Vec<Fermata>,
    pub arpeggiate: Vec<Arpeggiate>,
    pub non_arpeggiate: Vec<NonArpeggiate>,
    pub accidental_marks: Vec<String>,
    pub other_notation: Vec<OtherNotation>,
}

impl<'a> ParseContent<'a> for Notations {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut this = Self::default();

        let _footnote = stream.optional::<()>(b"footnote")?;
        let _level = stream.optional::<()>(b"level")?;

        while let Some(child) = stream.peek_tag()? {
            match child {
                b"tied" => this.tied.push(stream.required(b"tied")?),
                b"slur" => this.slurs.push(stream.required(b"slur")?),
                b"tuplet" => this.tuplet.push(stream.required(b"tuplet")?),
                b"glissando" => this.glissando.push(stream.required(b"glissando")?),
                b"slide" => this.slide.push(stream.required(b"slide")?),
                b"ornaments" => this.ornaments.push(stream.required(b"ornaments")?),
                b"technical" => this.technical.push(stream.required(b"technical")?),
                b"articulations" => this.articulations.push(stream.required(b"articulations")?),
                b"dynamics" => this.dynamics.push(stream.required(b"dynamics")?),
                b"fermata" => this.fermata.push(stream.required(b"fermata")?),
                b"arpeggiate" => this.arpeggiate.push(stream.required(b"arpeggiate")?),
                b"non-arpeggiate" => this
                    .non_arpeggiate
                    .push(stream.required(b"non-arpeggiate")?),
                b"accidental-mark" => this
                    .accidental_marks
                    .push(stream.required(b"accidental-mark")?),
                b"other-notation" => this
                    .other_notation
                    .push(stream.required(b"other-notation")?),
                _ => break,
            }
        }

        stream.skip_to_end(tag)?;

        Ok(this)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/slur/
#[derive(Debug, Clone)]
pub struct Slur {
    pub kind: StartStopContinue,
    pub number: NumberLevel,
}

impl<'a> ParseContent<'a> for Slur {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind = None;
        let mut number = 1;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"type" => kind = StartStopContinue::parse(attr.value.as_ref()),
                b"number" => number = parse_str_as(&attr.value).unwrap_or(1),
                _ => {}
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("slur/@type".into()))
            })?,
            number,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/glissando/
#[derive(Debug, Clone)]
pub struct Glissando {
    pub kind: StartStop,
    pub number: NumberLevel,
    pub text: String,
}

impl<'a> ParseContent<'a> for Glissando {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind = None;
        let mut number = 1;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"type" => kind = StartStop::parse(attr.value.as_ref()),
                b"number" => number = parse_str_as(&attr.value).unwrap_or(1),
                _ => {}
            }
        }

        let text = String::parse(stream, tag, start)?;

        Ok(Self {
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("glissando/@type".into()))
            })?,
            number,
            text,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/slide/
#[derive(Debug, Clone)]
pub struct Slide {
    pub kind: StartStop,
    pub number: NumberLevel,
    pub text: String,
}

impl<'a> ParseContent<'a> for Slide {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind = None;
        let mut number = 1;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"type" => kind = StartStop::parse(attr.value.as_ref()),
                b"number" => number = parse_str_as(&attr.value).unwrap_or(1),
                _ => {}
            }
        }

        let text = String::parse(stream, tag, start)?;

        Ok(Self {
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("slide/@type".into()))
            })?,
            number,
            text,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/ornaments/
#[derive(Debug, Clone)]
pub enum Ornament {
    TrillMark,
    Turn,
    DelayedTurn,
    InvertedTurn,
    DelayedInvertedTurn,
    VerticalTurn,
    InvertedVerticalTurn,
    Shake,
    WavyLine(WavyLine),
    Mordent,
    InvertedMordent,
    Schleifer,
    /// Number of tremolo marks (0-8); 0 is used for unmeasured tremolos.
    Tremolo(u8),
    Haydn,
    Other(String),
}

impl<'a> ParseContentFlat<'a> for Ornament {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, parser::Error> {
        macro_rules! presence {
            ($tag:literal, $variant:expr) => {{
                stream.required::<()>($tag)?;
                $variant
            }};
        }

        let this = match stream.peek_tag()? {
            Some(b"trill-mark") => presence!(b"trill-mark", Self::TrillMark),
            Some(b"turn") => presence!(b"turn", Self::Turn),
            Some(b"delayed-turn") => presence!(b"delayed-turn", Self::DelayedTurn),
            Some(b"inverted-turn") => presence!(b"inverted-turn", Self::InvertedTurn),
            Some(b"delayed-inverted-turn") => {
                presence!(b"delayed-inverted-turn", Self::DelayedInvertedTurn)
            }
            Some(b"vertical-turn") => presence!(b"vertical-turn", Self::VerticalTurn),
            Some(b"inverted-vertical-turn") => {
                presence!(b"inverted-vertical-turn", Self::InvertedVerticalTurn)
            }
            Some(b"shake") => presence!(b"shake", Self::Shake),
            Some(b"wavy-line") => Self::WavyLine(stream.required(b"wavy-line")?),
            Some(b"mordent") => presence!(b"mordent", Self::Mordent),
            Some(b"inverted-mordent") => presence!(b"inverted-mordent", Self::InvertedMordent),
            Some(b"schleifer") => presence!(b"schleifer", Self::Schleifer),
            Some(b"tremolo") => Self::Tremolo(stream.required(b"tremolo")?),
            Some(b"haydn") => presence!(b"haydn", Self::Haydn),
            Some(b"other-ornament") => Self::Other(stream.required(b"other-ornament")?),
            Some(other) => {
                let msg = format!("Invalid ornament: {:?}", String::from_utf8_lossy(other));
                return Err(stream.error(parser::ErrorKind::UnexpectedEvent(msg)));
            }
            None => {
                return Err(stream.error(parser::ErrorKind::MissingElement(
                    "Expected ornament element".into(),
                )));
            }
        };

        Ok(this)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/ornaments/
#[derive(Debug, Clone, Default)]
pub struct Ornaments {
    pub items: Vec<Ornament>,
    pub accidental_marks: Vec<String>,
}

impl<'a> ParseContent<'a> for Ornaments {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut items = Vec::new();
        let mut accidental_marks = Vec::new();

        loop {
            match stream.peek_tag()? {
                Some(b"accidental-mark") => {
                    accidental_marks.push(stream.required(b"accidental-mark")?)
                }
                Some(_) => items.push(stream.flatten()?),
                None => break,
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            items,
            accidental_marks,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/fingering/
#[derive(Debug, Clone)]
pub struct Fingering {
    pub value: String,
    pub substitution: bool,
    pub alternate: bool,
}

impl<'a> ParseContent<'a> for Fingering {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut substitution = false;
        let mut alternate = false;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"substitution" => substitution = parse_yes_no(attr.value.as_ref()),
                b"alternate" => alternate = parse_yes_no(attr.value.as_ref()),
                _ => {}
            }
        }

        let value = String::parse(stream, tag, start)?;

        Ok(Self {
            value,
            substitution,
            alternate,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/technical/
#[derive(Debug, Clone)]
pub enum TechnicalMark {
    Fingering(Fingering),
    String(StringNumber),
    // TODO:
    Other(String),
}

impl<'a> ParseContentFlat<'a> for TechnicalMark {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, parser::Error> {
        let this = match stream.peek_tag()? {
            Some(b"fingering") => Self::Fingering(stream.required(b"fingering")?),
            Some(b"string") => Self::String(stream.required(b"string")?),
            Some(other) => {
                let name = String::from_utf8_lossy(other).into_owned();
                let other = other.to_vec();
                stream.required::<()>(&other)?;
                Self::Other(name)
            }
            None => {
                return Err(stream.error(parser::ErrorKind::MissingElement(
                    "Expected technical element".into(),
                )));
            }
        };

        Ok(this)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/technical/
#[derive(Debug, Clone, Default)]
pub struct Technical {
    pub marks: Vec<TechnicalMark>,
}

impl<'a> ParseContent<'a> for Technical {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut marks = Vec::new();
        while stream.peek_tag()?.is_some() {
            marks.push(stream.flatten()?);
        }

        stream.skip_to_end(tag)?;

        Ok(Self { marks })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/dynamics/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicsMarkKind {
    P,
    Pp,
    Ppp,
    Pppp,
    Ppppp,
    Pppppp,
    F,
    Ff,
    Fff,
    Ffff,
    Fffff,
    Ffffff,
    Mp,
    Mf,
    Sf,
    Sfp,
    Sfpp,
    Fp,
    Rf,
    Rfz,
    Sfz,
    Sffz,
    Fz,
    N,
    Pf,
    Sfzp,
}

#[derive(Debug, Clone)]
pub enum DynamicsMark {
    Named(DynamicsMarkKind),
    Other(String),
}

impl<'a> ParseContentFlat<'a> for DynamicsMark {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, parser::Error> {
        use DynamicsMarkKind::*;

        macro_rules! named {
            ($tag:literal, $variant:expr) => {{
                stream.required::<()>($tag)?;
                Self::Named($variant)
            }};
        }

        let this = match stream.peek_tag()? {
            Some(b"p") => named!(b"p", P),
            Some(b"pp") => named!(b"pp", Pp),
            Some(b"ppp") => named!(b"ppp", Ppp),
            Some(b"pppp") => named!(b"pppp", Pppp),
            Some(b"ppppp") => named!(b"ppppp", Ppppp),
            Some(b"pppppp") => named!(b"pppppp", Pppppp),
            Some(b"f") => named!(b"f", F),
            Some(b"ff") => named!(b"ff", Ff),
            Some(b"fff") => named!(b"fff", Fff),
            Some(b"ffff") => named!(b"ffff", Ffff),
            Some(b"fffff") => named!(b"fffff", Fffff),
            Some(b"ffffff") => named!(b"ffffff", Ffffff),
            Some(b"mp") => named!(b"mp", Mp),
            Some(b"mf") => named!(b"mf", Mf),
            Some(b"sf") => named!(b"sf", Sf),
            Some(b"sfp") => named!(b"sfp", Sfp),
            Some(b"sfpp") => named!(b"sfpp", Sfpp),
            Some(b"fp") => named!(b"fp", Fp),
            Some(b"rf") => named!(b"rf", Rf),
            Some(b"rfz") => named!(b"rfz", Rfz),
            Some(b"sfz") => named!(b"sfz", Sfz),
            Some(b"sffz") => named!(b"sffz", Sffz),
            Some(b"fz") => named!(b"fz", Fz),
            Some(b"n") => named!(b"n", N),
            Some(b"pf") => named!(b"pf", Pf),
            Some(b"sfzp") => named!(b"sfzp", Sfzp),
            Some(b"other-dynamics") => Self::Other(stream.required(b"other-dynamics")?),
            Some(other) => {
                let msg = format!(
                    "Invalid dynamics mark: {:?}",
                    String::from_utf8_lossy(other)
                );
                return Err(stream.error(parser::ErrorKind::UnexpectedEvent(msg)));
            }
            None => {
                return Err(stream.error(parser::ErrorKind::MissingElement(
                    "Expected dynamics mark element".into(),
                )));
            }
        };

        Ok(this)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/dynamics/
#[derive(Debug, Clone, Default)]
pub struct Dynamics {
    pub marks: Vec<DynamicsMark>,
}

impl<'a> ParseContent<'a> for Dynamics {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut marks = Vec::new();
        while stream.peek_tag()?.is_some() {
            marks.push(stream.flatten()?);
        }

        stream.skip_to_end(tag)?;

        Ok(Self { marks })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/up-down/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpDown {
    Up,
    Down,
}

impl UpDown {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v = match bytes {
            b"up" => UpDown::Up,
            b"down" => UpDown::Down,
            other => {
                error!("Unexpected up-down: {other:?}");
                return None;
            }
        };
        Some(v)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/top-bottom/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopBottom {
    Top,
    Bottom,
}

impl TopBottom {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v = match bytes {
            b"top" => TopBottom::Top,
            b"bottom" => TopBottom::Bottom,
            other => {
                error!("Unexpected top-bottom: {other:?}");
                return None;
            }
        };
        Some(v)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/arpeggiate/
#[derive(Debug, Clone)]
pub struct Arpeggiate {
    pub direction: Option<UpDown>,
}

impl<'a> ParseContent<'a> for Arpeggiate {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut direction = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            if attr.key.as_ref() == b"direction" {
                direction = UpDown::parse(attr.value.as_ref());
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self { direction })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/non-arpeggiate/
#[derive(Debug, Clone)]
pub struct NonArpeggiate {
    pub kind: TopBottom,
}

impl<'a> ParseContent<'a> for NonArpeggiate {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            if attr.key.as_ref() == b"type" {
                kind = TopBottom::parse(attr.value.as_ref());
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement(
                    "non-arpeggiate/@type".into(),
                ))
            })?,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/other-notation/
#[derive(Debug, Clone)]
pub struct OtherNotation {
    pub kind: String,
    pub text: String,
}

impl<'a> ParseContent<'a> for OtherNotation {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind = None;
        for attr in start.attributes().filter_map(|r| r.ok()) {
            if attr.key.as_ref() == b"type" {
                kind = Some(String::from_utf8_lossy(attr.value.as_ref()).into_owned());
            }
        }

        let text = String::parse(stream, tag, start)?;

        Ok(Self {
            kind: kind.unwrap_or_default(),
            text,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/tied/
#[derive(Debug, Clone)]
pub struct Tied {
    pub kind: TiedType,
}

impl<'a> ParseContent<'a> for Tied {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind: Option<TiedType> = None;

        for attr in start.attributes().filter_map(|r| r.ok()) {
            if attr.key.as_ref() == b"type" {
                kind = TiedType::parse(attr.value.as_ref());
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("tied/@type".into()))
            })?,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/data-types/tied-type/
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TiedType {
    Start,
    Stop,
    Continue,
    LetRing,
}

impl TiedType {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v = match bytes {
            b"start" => TiedType::Start,
            b"stop" => TiedType::Stop,
            b"continue" => TiedType::Continue,
            b"let-ring" => TiedType::LetRing,
            other => {
                error!("Unexpected tied type: {other:?}");
                return None;
            }
        };

        Some(v)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/tuplet/
#[derive(Debug, Clone)]
pub struct Tuplet {
    pub kind: StartStop,
    pub actual: Option<TupletPortion>,
    pub normal: Option<TupletPortion>,
}

impl<'a> ParseContent<'a> for Tuplet {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind: Option<StartStop> = None;

        for attr in start.attributes().filter_map(|r| r.ok()) {
            if attr.key.as_ref() == b"type" {
                kind = StartStop::parse(attr.value.as_ref());
            }
        }

        let actual = stream.optional(b"tuplet-actual")?;
        let normal = stream.optional(b"tuplet-normal")?;

        stream.skip_to_end(tag)?;

        Ok(Self {
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("tuplet/@type".into()))
            })?,
            actual,
            normal,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/tuplet-actual/
#[derive(Debug, Clone, Default)]
pub struct TupletPortion {
    pub number: Option<u32>,
    pub note_type: Option<String>,
    pub dots: usize,
}

impl<'a> ParseContent<'a> for TupletPortion {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let number = stream.optional(b"tuplet-number")?;
        let note_type = stream.optional(b"tuplet-type")?;
        let dots = stream.zero_or_more::<()>(b"tuplet-dot")?.len();

        stream.skip_to_end(tag)?;

        Ok(Self {
            number,
            note_type,
            dots,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/articulations/
#[derive(Debug, Default, Clone)]
pub struct Articulations {
    pub staccato: usize,
}

impl<'a> ParseContent<'a> for Articulations {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut this = Self::default();

        while let Some(child) = stream.peek_tag()? {
            match child {
                b"staccato" => {
                    stream.required::<()>(b"staccato")?;
                    this.staccato += 1;
                }
                _ => {
                    let child = child.to_vec();
                    stream.required::<()>(&child)?;
                }
            }
        }

        stream.skip_to_end(tag)?;

        Ok(this)
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/tie/
#[derive(Debug, Clone)]
pub struct Tie {
    pub kind: StartStop,
    pub time_only: Option<String>,
}

impl<'a> ParseContent<'a> for Tie {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        let mut kind: Option<StartStop> = None;
        let mut time_only: Option<String> = None;

        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"type" => kind = StartStop::parse(attr.value.as_ref()),
                b"time-only" => time_only = parse_str_as(&attr.value),
                _ => {}
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self {
            kind: kind.ok_or_else(|| {
                stream.error(parser::ErrorKind::MissingElement("tie/@type".into()))
            })?,
            time_only,
        })
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
#[derive(Debug, Clone)]
pub struct Pitch {
    pub step: Step,
    pub alter: Option<Semitones>,
    pub octave: Octave,
}

impl<'a> ParseContent<'a> for Pitch {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> std::result::Result<Self, crate::parser::Error> {
        let step = stream.required(b"step")?;
        let alter = stream.optional(b"alter")?;
        let octave = stream.required(b"octave")?;

        stream.skip_to_end(tag)?;

        Ok(Pitch {
            step,
            alter,
            octave,
        })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/chord/
#[derive(Debug, Clone)]
pub struct Chord {}

impl<'a> ParseContent<'a> for Chord {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, parser::Error> {
        stream.skip_to_end(tag)?;
        Ok(Self {})
    }
}

#[derive(Debug, Clone)]
pub struct DisplayStepOctave {
    pub step: Step,
    pub octave: Octave,
}

impl<'a> ParseContentFlat<'a> for DisplayStepOctave {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, parser::Error> {
        let step = stream.required(b"display-step")?;
        let octave = stream.required(b"display-octave")?;

        Ok(Self { step, octave })
    }
}

/// https://w3c.github.io/musicxml/musicxml-reference/elements/rest/
#[derive(Debug, Clone)]
pub struct Rest {
    pub measure: bool,
    pub display: Option<DisplayStepOctave>,
}

impl<'a> ParseContent<'a> for Rest {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, crate::parser::Error> {
        let mut measure = false;

        for attr in start.attributes().filter_map(|r| r.ok()) {
            match attr.key.as_ref() {
                b"measure" => measure = parse_yes_no(attr.value.as_ref()),
                _ => {}
            }
        }

        let display = match stream.peek_tag()? {
            Some(b"display-step") => Some(stream.flatten()?),
            _ => None,
        };

        stream.skip_to_end(tag)?;

        Ok(Self { measure, display })
    }
}

pub use primitive::*;

use crate::parser::{self, ParseContent, ParseContentFlat, XmlStream};
mod primitive {
    #![allow(unused)]

    use std::borrow::Cow;

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

    /// The `fifths` type represents the number of flats or sharps in a traditional key
    /// signature. Negative numbers are used for flats and positive numbers for sharps.
    ///
    /// Spec: https://www.w3.org/2021/06/musicxml40/musicxml-reference/data-types/fifths/
    pub type Fifths = i32;

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
    pub type StaffLinePosition = i32;

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

    impl<'a> ParseContent<'a> for Step {
        fn parse(
            stream: &mut XmlStream<'a>,
            tag: &[u8],
            start: &BytesStart<'a>,
        ) -> Result<Self, crate::parser::Error> {
            let str = Cow::<str>::parse(stream, tag, start)?;

            let step =
                match str.trim() {
                    "A" => Step::A,
                    "B" => Step::B,
                    "C" => Step::C,
                    "D" => Step::D,
                    "E" => Step::E,
                    "F" => Step::F,
                    "G" => Step::G,
                    other => {
                        return Err(stream.error(crate::parser::ErrorKind::UnexpectedValue(
                            format!("Unexpected step value: {other:?}"),
                        )));
                    }
                };

            Ok(step)
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

    impl<'a> ParseContent<'a> for ClefSign {
        fn parse(
            stream: &mut XmlStream<'a>,
            tag: &[u8],
            start: &BytesStart<'a>,
        ) -> Result<Self, parser::Error> {
            let v = Cow::<str>::parse(stream, tag, start)?;
            v.parse::<Self>()
                .map_err(|e| stream.error(parser::ErrorKind::UnexpectedValue(e)))
        }
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
