#![allow(clippy::single_match, clippy::let_unit_value)]

mod chord_symbol;
mod musicxml;

use std::{borrow::Cow, collections::BTreeMap, time::Duration};

use musicxml::MeasureItem;
use quick_xml::events::{BytesStart, Event};

mod parser;

pub use parser::Error as ParseError;

#[cfg(test)]
mod test_support;

const TICKS_PER_QUARTER_NOTE: u16 = 480;
const TICKS_PER_QUARTER_NOTE_F64: f64 = TICKS_PER_QUARTER_NOTE as f64;

const MINUTE: Duration = Duration::from_secs(60);

const DEFAULT_BPM: u32 = 120;

const DEFAULT_VELOCITY: u8 = 80;
const HARMONY_CHANNEL: u8 = 1;

// 1s = 1_000_000µ
// 1m = 60_000_000µ

// BPM = 60_000_000 / MicrosecondsPerQuarterNote
// BPM * MicrosecondsPerQuarterNote = 60_000_000
// MicrosecondsPerQuarterNote = 60_000_000 / BPM

/// Error returned by [`musicxml_to_smf`]
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),
    #[error("unsupported score: {0}")]
    Unsupported(String),
}

/// Convert a `score-partwise` MusicXML document into a MIDI file, one track per staff
pub fn musicxml_to_smf(src: &str) -> Result<midly::Smf<'static>, Error> {
    // The converter is still WIP and asserts on cases it doesn't handle yet, don't take the caller down with it
    std::panic::catch_unwind(|| parse(src, false))
        .map_err(|_| Error::Unsupported("score uses features not supported yet".into()))?
}

/// Like [`musicxml_to_smf`], but panics on scores the converter doesn't handle yet,
/// and can mimic MuseScore where it disagrees with the spec
#[doc(hidden)]
pub fn parse(src: &str, musescore_compat: bool) -> Result<midly::Smf<'static>, Error> {
    let mut stream = parser::XmlStream::new("score.musicxml", src);
    let v: musicxml::ScorePartwise = stream.required(b"score-partwise")?;
    log::debug!("{v:#?}");

    if v.part.len() != 1 {
        return Err(Error::Unsupported(format!(
            "expected exactly one part, found {}",
            v.part.len()
        )));
    }

    let timeline = timeline(&v, musescore_compat);
    Ok(to_smf(&timeline))
}

#[derive(Debug)]
enum TimelineItem {
    Note(TimelineNote),
    Tempo(TimelineTempo),
}

#[derive(Debug, Clone)]
struct TimelineNote {
    start: f64,
    duration: f64,
    key: u8,
    velocity: u8,
    off_velocity: u8,
    tie_start: bool,
    tie_stop: bool,
    staff: u8,
    channel: u8,
}

#[derive(Debug)]
struct TimelineTempo {
    at: f64,
    bpm: f64,
}

fn timeline(v: &musicxml::ScorePartwise, musescore_compat: bool) -> Vec<TimelineItem> {
    assert_eq!(v.part.len(), 1);

    let measures = v.part.iter().flat_map(|part| &part.measure);

    let mut divisions = 1.0;
    let mut position = 0.0f64;
    let mut staves = 1;

    let mut timeline = Vec::new();
    let mut chord_symbols = Vec::new();

    for measure in measures {
        let mut iter = measure.content.iter();

        while let Some(item) = iter.next() {
            log::trace!("{item:#?}");

            match item {
                MeasureItem::Attributes(attributes) => {
                    if let Some(d) = attributes.divisions.as_ref() {
                        divisions = *d;
                    }
                    if let Some(s) = attributes.staves {
                        staves = s;
                    }
                }
                MeasureItem::Note(note) => {
                    let msg = match &note.kind {
                        // Grace notes take no time, and aren't played, cause I don't care, it brings too much complexity
                        musicxml::NoteKindState::Grace(_)
                        | musicxml::NoteKindState::GraceCue(_) => continue,
                        musicxml::NoteKindState::Cue(_) | musicxml::NoteKindState::Regular(_) => {
                            full_note(note).unwrap()
                        }
                    };
                    assert!(!msg.chord);

                    let quarters = msg.duration / divisions;

                    match msg.kind {
                        musicxml::NoteKindStatePitchKind::Pitch(pitch) => {
                            if msg.play {
                                let (tie_start, tie_stop) = tie_kinds(msg.tie);
                                timeline.push(TimelineItem::Note(TimelineNote {
                                    start: position,
                                    duration: quarters,
                                    key: midi_note_number(
                                        pitch.step,
                                        pitch.octave,
                                        pitch.alter.unwrap_or(0.0),
                                    ),
                                    velocity: note_on_velocity(note),
                                    off_velocity: note_off_velocity(note),
                                    tie_start,
                                    tie_stop,
                                    staff: note.staff.unwrap_or(1),
                                    channel: 0,
                                }));
                            }

                            let mut peek_iter = iter.clone();
                            while let Some(MeasureItem::Note(note)) = peek_iter.next() {
                                let Some(next) = full_note(note) else {
                                    break;
                                };
                                let Some(pitch) = next.chord.then_some(()).and(match next.kind {
                                    musicxml::NoteKindStatePitchKind::Pitch(pitch) => Some(pitch),
                                    _ => None,
                                }) else {
                                    break;
                                };

                                iter.next();

                                if !next.play {
                                    continue;
                                }

                                // Chord notes share the duration of the chord's first note
                                let (tie_start, tie_stop) = tie_kinds(next.tie);
                                timeline.push(TimelineItem::Note(TimelineNote {
                                    start: position,
                                    duration: quarters,
                                    key: midi_note_number(
                                        pitch.step,
                                        pitch.octave,
                                        pitch.alter.unwrap_or(0.0),
                                    ),
                                    velocity: note_on_velocity(note),
                                    off_velocity: note_off_velocity(note),
                                    tie_start,
                                    tie_stop,
                                    staff: note.staff.unwrap_or(1),
                                    channel: 0,
                                }));
                            }

                            position += quarters;
                        }
                        musicxml::NoteKindStatePitchKind::Rest(_) => {
                            position += quarters;
                        }
                        _ => {}
                    }
                }
                MeasureItem::Backup(backup) => {
                    position = (position - backup.duration / divisions).max(0.0);
                }
                MeasureItem::Forward(forward) => {
                    position += forward.duration / divisions;
                }
                MeasureItem::Print(_) => {}
                MeasureItem::Barline(_) => {}
                MeasureItem::Harmony(harmony) => {
                    if let Some(keys) = chord_symbol::realize(harmony) {
                        let offset = harmony
                            .offset
                            .as_ref()
                            // MuseScore ignores `sound`, like for directions
                            .filter(|offset| offset.sound || musescore_compat)
                            .map(|offset| offset.value / divisions)
                            .unwrap_or(0.0);

                        let staff = harmony
                            .staff
                            .filter(|staff| (1..=staves).contains(&u32::from(*staff)))
                            .unwrap_or(1);

                        chord_symbols.push((staff, (position + offset).max(0.0), keys));
                    }
                }
                MeasureItem::Direction(direction) => {
                    if let Some(sound) = direction.sound.as_ref()
                        && let Some(tempo) = sound.tempo.as_ref()
                    {
                        let offset = sound
                            .offset
                            .as_ref()
                            .or(direction
                                .offset
                                .as_ref()
                                // Why does MuseScore ignore this? Spec seems to suggest I should filter here
                                .filter(|offset| offset.sound || musescore_compat))
                            .map(|offset| offset.value / divisions)
                            .unwrap_or(0.0);

                        timeline.push(TimelineItem::Tempo(TimelineTempo {
                            at: (position + offset).max(0.0),
                            bpm: *tempo,
                        }));
                    }
                }
            }
        }
    }

    let end = timeline
        .iter()
        .filter_map(|item| match item {
            TimelineItem::Note(note) => Some(note.start + note.duration),
            _ => None,
        })
        .fold(position, f64::max);

    // Like MuseScore, chord symbols last until the next one on the same staff, or the end
    chord_symbols.sort_by(|(staff_a, start_a, _), (staff_b, start_b, _)| {
        staff_a.cmp(staff_b).then(start_a.total_cmp(start_b))
    });

    for (i, (staff, start, keys)) in chord_symbols.iter().enumerate() {
        let next = chord_symbols[i + 1..]
            .iter()
            .take_while(|(next_staff, _, _)| next_staff == staff)
            .map(|(_, next_start, _)| *next_start)
            .find(|next_start| next_start > start)
            .unwrap_or(end);

        for key in keys {
            timeline.push(TimelineItem::Note(TimelineNote {
                start: *start,
                duration: (next - start).max(0.0),
                key: *key,
                velocity: DEFAULT_VELOCITY,
                off_velocity: 0,
                tie_start: false,
                tie_stop: false,
                staff: *staff,
                channel: HARMONY_CHANNEL,
            }));
        }
    }

    timeline
}

type TrackEvents = BTreeMap<usize, Vec<midly::TrackEventKind<'static>>>;

fn to_smf(timeline: &[TimelineItem]) -> midly::Smf<'static> {
    let staves = timeline
        .iter()
        .filter_map(|item| match item {
            TimelineItem::Note(note) => Some(note.staff),
            _ => None,
        })
        .max()
        .unwrap_or(1)
        .max(1);

    let mut tracks: Vec<TrackEvents> = vec![TrackEvents::new(); usize::from(staves)];

    for item in timeline {
        match item {
            TimelineItem::Note(note) => {
                let events = &mut tracks[usize::from(note.staff.max(1)) - 1];

                if !note.tie_stop {
                    events.entry(to_ticks(note.start)).or_default().push(
                        midly::TrackEventKind::Midi {
                            channel: note.channel.into(),
                            message: midly::MidiMessage::NoteOn {
                                key: note.key.into(),
                                vel: note.velocity.into(),
                            },
                        },
                    );
                }

                if !note.tie_start {
                    events
                        .entry(to_ticks(note.start + note.duration))
                        .or_default()
                        .push(midly::TrackEventKind::Midi {
                            channel: note.channel.into(),
                            message: midly::MidiMessage::NoteOff {
                                key: note.key.into(),
                                vel: note.off_velocity.into(),
                            },
                        });
                }
            }
            TimelineItem::Tempo(TimelineTempo { at, bpm }) if *bpm > 0.0 => {
                let microseconds_per_quarter_note =
                    (MINUTE.as_micros() as f64 / bpm).round() as u32;

                tracks[0]
                    .entry(to_ticks(*at))
                    .or_default()
                    .push(midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(
                        microseconds_per_quarter_note.into(),
                    )));
            }
            TimelineItem::Tempo(_) => {}
        }
    }

    // Like MuseScore, write the MIDI default of 120 BPM when the score doesn't start with a tempo
    let events = tracks[0].entry(0).or_default();
    let has_tempo = events.iter().any(|event| {
        matches!(
            event,
            midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(..))
        )
    });
    if !has_tempo {
        events.insert(
            0,
            midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(
                (MINUTE.as_micros() as u32 / DEFAULT_BPM).into(),
            )),
        );
    }

    midly::Smf {
        header: midly::Header {
            format: midly::Format::Parallel,
            timing: midly::Timing::Metrical(midly::num::u15::new(TICKS_PER_QUARTER_NOTE)),
        },
        tracks: tracks.into_iter().map(delta_encode).collect(),
    }
}

fn delta_encode(events: TrackEvents) -> Vec<midly::TrackEvent<'static>> {
    let mut track = vec![];

    let mut prev = 0;
    for (position, events) in events {
        let mut delta = position - prev;
        prev = position;

        for kind in events {
            track.push(midly::TrackEvent {
                delta: (delta as u32).into(),
                kind,
            });
            delta = 0;
        }
    }

    track.push(midly::TrackEvent {
        delta: 1.into(),
        kind: midly::TrackEventKind::Meta(midly::MetaMessage::EndOfTrack),
    });

    track
}

fn to_ticks(quarters: f64) -> usize {
    (quarters * TICKS_PER_QUARTER_NOTE_F64).round() as usize
}

const MIDI_FORTE_VELOCITY: f64 = 90.0;

fn dynamics_to_velocity(dynamics: f64) -> u8 {
    (dynamics / 100.0 * MIDI_FORTE_VELOCITY)
        .round()
        .clamp(0.0, 127.0) as u8
}

fn note_on_velocity(note: &musicxml::Note) -> u8 {
    note.dynamics
        .map(|dynamics| dynamics_to_velocity(dynamics).max(1))
        .unwrap_or(DEFAULT_VELOCITY)
}

fn note_off_velocity(note: &musicxml::Note) -> u8 {
    note.end_dynamics.map(dynamics_to_velocity).unwrap_or(0)
}

/// The parts of a regular or cue note
struct FullNote<'a> {
    chord: bool,
    kind: &'a musicxml::NoteKindStatePitchKind,
    duration: f64,
    tie: &'a [musicxml::Tie],
    /// Cue notes don't play
    play: bool,
}

fn full_note(note: &musicxml::Note) -> Option<FullNote<'_>> {
    match &note.kind {
        musicxml::NoteKindState::Regular(msg) => Some(FullNote {
            chord: msg.chord.is_some(),
            kind: &msg.kind,
            duration: msg.duration,
            tie: &msg.tie,
            play: true,
        }),
        musicxml::NoteKindState::Cue(msg) => Some(FullNote {
            chord: msg.chord.is_some(),
            kind: &msg.kind,
            duration: msg.duration,
            tie: &msg.tie,
            play: false,
        }),
        musicxml::NoteKindState::Grace(_) | musicxml::NoteKindState::GraceCue(_) => None,
    }
}

/// Returns `(start, stop)`, a note in the middle of a tie chain has both
fn tie_kinds(ties: &[musicxml::Tie]) -> (bool, bool) {
    let start = ties.iter().any(|t| t.kind == musicxml::StartStop::Start);
    let stop = ties.iter().any(|t| t.kind == musicxml::StartStop::Stop);
    (start, stop)
}

fn midi_note_number(step: musicxml::Step, octave: u8, alter: f64) -> u8 {
    use musicxml::Step;
    let base = match step {
        Step::C => 0,
        Step::D => 2,
        Step::E => 4,
        Step::F => 5,
        Step::G => 7,
        Step::A => 9,
        Step::B => 11,
    };

    // No microtones for now
    let alter = alter.round() as i32;

    (((octave + 1) * 12 + base) as i32 + alter) as u8
}

type Reader<'a> = quick_xml::reader::Reader<&'a [u8]>;

#[derive(Debug)]
enum DocumentElement<'a> {
    Child(Document<'a>),
    Text(Cow<'a, str>),
}

#[derive(Debug)]
struct Document<'a> {
    start: BytesStart<'a>,
    events: Vec<DocumentElement<'a>>,
}

impl<'a> Document<'a> {
    fn new(start: BytesStart<'a>) -> Self {
        Self {
            start,
            events: Vec::new(),
        }
    }

    fn read(reader: &mut Reader<'a>, start: BytesStart<'a>) -> quick_xml::Result<Self> {
        let mut document = Self::new(start);

        loop {
            let event = reader.read_event()?;
            match event {
                Event::Start(start) => {
                    let child = Document::read(reader, start)?;
                    document.events.push(DocumentElement::Child(child));
                    continue;
                }
                Event::End(end) => {
                    break;
                }
                Event::Empty(_) => unreachable!(),
                Event::Text(text) => {
                    let text = text.decode().unwrap();
                    document.events.push(DocumentElement::Text(text));
                    continue;
                }
                Event::CData(bytes_cdata) => todo!(),
                Event::Comment(bytes_text) => todo!(),
                Event::Decl(bytes_decl) => todo!(),
                Event::PI(bytes_pi) => todo!(),
                Event::DocType(bytes_text) => todo!(),
                Event::GeneralRef(bytes_ref) => todo!(),
                Event::Eof => break,
            }
        }

        Ok(document)
    }
}

fn simple_xml_format(src: &str) -> String {
    let mut reader = Reader::from_str(src);
    reader.config_mut().trim_text(true);

    let mut out = String::new();
    let mut depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                out.push_str(&"  ".repeat(depth));
                out.push('<');
                out.push_str(std::str::from_utf8(e.name().as_ref()).unwrap());
                for attr in e.attributes().flatten() {
                    out.push(' ');
                    out.push_str(std::str::from_utf8(attr.key.as_ref()).unwrap());
                    out.push_str("=\"");
                    out.push_str(&String::from_utf8_lossy(&attr.value));
                    out.push('"');
                }
                out.push_str(">\n");
                depth += 1;
            }

            Ok(Event::End(e)) => {
                depth -= 1;
                out.push_str(&"  ".repeat(depth));
                out.push_str("</");
                out.push_str(std::str::from_utf8(e.name().as_ref()).unwrap());
                out.push_str(">\n");
            }

            Ok(Event::Empty(e)) => {
                out.push_str(&"  ".repeat(depth));
                out.push('<');
                out.push_str(std::str::from_utf8(e.name().as_ref()).unwrap());
                for attr in e.attributes().flatten() {
                    out.push(' ');
                    out.push_str(std::str::from_utf8(attr.key.as_ref()).unwrap());
                    out.push_str("=\"");
                    out.push_str(&String::from_utf8_lossy(&attr.value));
                    out.push('"');
                }
                out.push_str("/>\n");
            }

            Ok(Event::Text(e)) => {
                let text = e.decode().unwrap();
                if !text.trim().is_empty() {
                    out.push_str(&"  ".repeat(depth));
                    out.push_str(text.trim());
                    out.push('\n');
                }
            }

            Ok(Event::Comment(e)) => {
                out.push_str(&"  ".repeat(depth));
                out.push_str("<!--");
                out.push_str(&String::from_utf8_lossy(&e));
                out.push_str("-->\n");
            }

            Ok(Event::Decl(e)) => {
                out.push_str(&"  ".repeat(depth));
                out.push_str(&String::from_utf8_lossy(&e));
                out.push('\n');
            }

            Ok(Event::Eof) => break,

            Err(_) => return src.to_owned(),
            _ => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! xml {
        ( $($t:tt)* ) => {
            stringify!($($t)*)
        };
    }

    /// NOTE: RUN THIS IN RELEASE! TO SLOW FOR DEBUG!
    ///
    /// Runs the real `musicxml::ScorePartwise` parser over every score in
    /// the `test-files/lib` submodule.
    #[test]
    #[ignore]
    fn parses_all_library_scores() {
        for path in test_support::all_scores() {
            let xml = test_support::extract_mxl(&path);

            let mut stream = parser::XmlStream::new(path.to_string_lossy(), &xml);
            stream
                .required::<musicxml::ScorePartwise>(b"score-partwise")
                .unwrap_or_else(|e| panic!("failed to parse {path:?}: {e:?}"));
        }
    }

    #[test]
    fn test_name() {
        let src = xml!(
            <score-partwise version="4.0">
              <part-list>
                <score-part id="P1">
                  <part-name>Piano</part-name>
                </score-part>
              </part-list>
              <part id="P1">
                <measure number="1">
                  <attributes>
                    <divisions>1</divisions>
                    <key>
                      <fifths>0</fifths>
                    </key>
                    <time>
                      <beats>4</beats>
                      <beat-type>4</beat-type>
                    </time>
                    <clef>
                      <sign>G</sign>
                      <line>2</line>
                    </clef>
                  </attributes>
                  <note>
                    <pitch>
                      <step>G</step>
                      <octave>4</octave>
                    </pitch>
                    <duration>1</duration>
                  </note>
                  <note>
                    <pitch>
                      <step>A</step>
                      <octave>4</octave>
                    </pitch>
                    <duration>1</duration>
                  </note>
                  <note>
                    <chord />
                    <pitch>
                      <step>D</step>
                      <octave>5</octave>
                    </pitch>
                    <duration>1</duration>
                  </note>
                  <note>
                    <chord />
                    <pitch>
                      <step>F</step>
                      <octave>5</octave>
                    </pitch>
                    <duration>1</duration>
                  </note>
                  <note>
                    <pitch>
                      <step>G</step>
                      <octave>4</octave>
                    </pitch>
                    <duration>1</duration>
                  </note>
                  <note>
                    <rest />
                    <duration>1</duration>
                  </note>
                </measure>
              </part>
            </score-partwise>
        );

        let src = simple_xml_format(src);
        let midi = parse(&src, false).unwrap();
        insta::assert_debug_snapshot!(midi);
    }

    #[test]
    fn b() {
        let src = xml!(
        <score-partwise version="4.0">
          <part-list>
            <score-part id="P1">
              <part-name>Piano</part-name>
            </score-part>
          </part-list>
          <part id="P1">
            <measure number="1" width="537.79">
              <attributes>
                <divisions>2</divisions>
                <key>
                  <fifths>0</fifths>
                </key>
                <time>
                  <beats>4</beats>
                  <beat-type>4</beat-type>
                </time>
                <staves>2</staves>
                <clef number="1">
                  <sign>G</sign>
                  <line>2</line>
                </clef>
                <clef number="2">
                  <sign>F</sign>
                  <line>4</line>
                </clef>
              </attributes>
              <note>
                <pitch>
                  <step>G</step>
                  <octave>4</octave>
                </pitch>
                <duration>2</duration>
                <staff>1</staff>
              </note>
              <note>
                <pitch>
                  <step>A</step>
                  <octave>4</octave>
                </pitch>
                <duration>2</duration>
                <staff>1</staff>
              </note>
              <note>
                <chord />
                <pitch>
                  <step>C</step>
                  <octave>5</octave>
                </pitch>
                <duration>2</duration>
                <staff>1</staff>
              </note>
              <note>
                <chord />
                <pitch>
                  <step>F</step>
                  <octave>5</octave>
                </pitch>
                <duration>2</duration>
                <staff>1</staff>
              </note>
              <note>
                <pitch>
                  <step>G</step>
                  <octave>4</octave>
                </pitch>
                <duration>2</duration>
                <staff>1</staff>
              </note>
              <note>
                <rest />
                <duration>2</duration>
              </note>
              <backup>
                <duration>8</duration>
              </backup>
              <note>
                <pitch>
                  <step>G</step>
                  <octave>2</octave>
                </pitch>
                <duration>2</duration>
                <staff>2</staff>
              </note>
              <note>
                <pitch>
                  <step>B</step>
                  <octave>2</octave>
                </pitch>
                <duration>1</duration>
                <staff>2</staff>
                <beam number="1">begin</beam>
              </note>
              <note>
                <pitch>
                  <step>C</step>
                  <octave>3</octave>
                </pitch>
                <duration>1</duration>
                <staff>2</staff>
                <beam number="1">end</beam>
              </note>
              <note>
                <pitch>
                  <step>E</step>
                  <octave>3</octave>
                </pitch>
                <duration>1</duration>
                <staff>2</staff>
              </note>
              <note>
                <rest />
                <duration>1</duration>
              </note>
              <note>
                <rest />
                <duration>2</duration>
              </note>
            </measure>
          </part>
        </score-partwise>
        );

        let midi = parse(src, false).unwrap();
        insta::assert_debug_snapshot!(midi);
    }

    #[test]
    fn oddtaxi() {
        let src = std::fs::read_to_string("./schema/ODDTAXI.musicxml").unwrap();

        let mut stream = parser::XmlStream::new("ODDTAXI.musicxml", &src);

        let v: musicxml::ScorePartwise = stream.required(b"score-partwise").unwrap();

        assert_eq!(v.part.len(), 1);
        assert_eq!(v.part_list.score_part.len(), 1);

        let midi = parse(&src, false).unwrap();
        insta::assert_debug_snapshot!(midi);
    }

    #[test]
    fn ties() {
        let src = xml!(
        <score-partwise version="4.0">
          <part-list>
            <score-part id="P1">
              <part-name>Piano</part-name>
            </score-part>
          </part-list>
          <part id="P1">
            <measure number="1">
              <attributes>
                <divisions>1</divisions>
              </attributes>
              <note>
                <pitch><step>C</step><octave>4</octave></pitch>
                <duration>1</duration>
                <tie type="start"/>
              </note>
              <note>
                <pitch><step>C</step><octave>4</octave></pitch>
                <duration>1</duration>
                <tie type="stop"/>
                <tie type="start"/>
              </note>
              <note>
                <pitch><step>C</step><octave>4</octave></pitch>
                <duration>1</duration>
                <tie type="stop"/>
              </note>
              <note>
                <pitch><step>E</step><octave>4</octave></pitch>
                <duration>1</duration>
              </note>
              <note>
                <chord/>
                <pitch><step>G</step><octave>4</octave></pitch>
                <duration>1</duration>
                <tie type="start"/>
              </note>
              <note>
                <pitch><step>D</step><octave>4</octave></pitch>
                <duration>1</duration>
              </note>
              <note>
                <chord/>
                <pitch><step>G</step><octave>4</octave></pitch>
                <duration>1</duration>
                <tie type="stop"/>
              </note>
            </measure>
          </part>
        </score-partwise>
        );

        let src = simple_xml_format(src);
        let midi = parse(&src, false).unwrap();
        insta::assert_debug_snapshot!(midi);
    }

    #[test]
    fn direction_offset() {
        let src = xml!(
        <score-partwise version="4.0">
          <part-list>
            <score-part id="P1">
              <part-name>Piano</part-name>
            </score-part>
          </part-list>
          <part id="P1">
            <measure number="1">
              <attributes>
                <divisions>2</divisions>
              </attributes>
              <note>
                <pitch><step>C</step><octave>4</octave></pitch>
                <duration>4</duration>
              </note>
              <direction>
                <direction-type><words>visual only</words></direction-type>
                <offset>-2</offset>
                <sound tempo="60"/>
              </direction>
              <note>
                <pitch><step>D</step><octave>4</octave></pitch>
                <duration>4</duration>
              </note>
              <direction>
                <direction-type><words>affects sound</words></direction-type>
                <offset sound="yes">-2</offset>
                <sound tempo="70"/>
              </direction>
              <note>
                <pitch><step>E</step><octave>4</octave></pitch>
                <duration>4</duration>
              </note>
              <direction>
                <direction-type><words>sound offset</words></direction-type>
                <sound tempo="80">
                  <offset>1</offset>
                </sound>
              </direction>
              <note>
                <pitch><step>F</step><octave>4</octave></pitch>
                <duration>4</duration>
              </note>
            </measure>
          </part>
        </score-partwise>
        );

        let src = simple_xml_format(src);
        insta::assert_debug_snapshot!(parse(&src, false).unwrap());
        insta::assert_debug_snapshot!(
            "direction_offset_musescore_compat",
            parse(&src, true).unwrap()
        );
    }

    #[test]
    fn harmony() {
        let src = xml!(
        <score-partwise version="4.0">
          <part-list>
            <score-part id="P1">
              <part-name>Guitar</part-name>
            </score-part>
          </part-list>
          <part id="P1">
            <measure number="1">
              <attributes>
                <divisions>2</divisions>
              </attributes>
              <harmony>
                <root><root-step>G</root-step></root>
                <kind>dominant</kind>
                <bass><bass-step>B</bass-step></bass>
              </harmony>
              <note>
                <pitch><step>B</step><octave>4</octave></pitch>
                <duration>4</duration>
              </note>
              <harmony>
                <root><root-step>C</root-step><root-alter>1</root-alter></root>
                <kind>minor-seventh</kind>
                <degree>
                  <degree-value>5</degree-value>
                  <degree-alter>-1</degree-alter>
                  <degree-type>alter</degree-type>
                </degree>
                <offset>2</offset>
              </harmony>
              <note>
                <pitch><step>C</step><octave>5</octave></pitch>
                <duration>4</duration>
              </note>
            </measure>
            <measure number="2">
              <harmony>
                <root><root-step>F</root-step></root>
                <kind>none</kind>
              </harmony>
              <note>
                <pitch><step>A</step><octave>4</octave></pitch>
                <duration>4</duration>
              </note>
              <harmony>
                <root><root-step>F</root-step></root>
                <kind>other</kind>
              </harmony>
              <note>
                <pitch><step>F</step><octave>4</octave></pitch>
                <duration>4</duration>
              </note>
            </measure>
          </part>
        </score-partwise>
        );

        let src = simple_xml_format(src);
        insta::assert_debug_snapshot!(parse(&src, false).unwrap());
        insta::assert_debug_snapshot!("harmony_musescore_compat", parse(&src, true).unwrap());
    }

    #[test]
    fn measure_rest() {
        let src = xml!(
        <score-partwise version="4.0">
          <part-list>
            <score-part id="P1">
              <part-name>Piano</part-name>
            </score-part>
          </part-list>
          <part id="P1">
            <measure number="1">
              <attributes>
                <divisions>1</divisions>
                <time>
                  <beats>4</beats>
                  <beat-type>4</beat-type>
                </time>
              </attributes>
              <note>
                <rest measure="yes"/>
                <duration>4</duration>
              </note>
            </measure>
            <measure number="2">
              <note>
                <rest measure="yes"/>
                <duration>8</duration>
              </note>
            </measure>
            <measure number="3">
              <note>
                <pitch><step>C</step><octave>4</octave></pitch>
                <duration>4</duration>
              </note>
            </measure>
          </part>
        </score-partwise>
        );

        let src = simple_xml_format(src);
        let midi = parse(&src, false).unwrap();
        insta::assert_debug_snapshot!(midi);
    }

    /// Grace notes are skipped without moving other notes, cue notes are silent but take time
    #[test]
    fn grace_cue() {
        let src = r#"
        <score-partwise version="4.0">
          <part-list>
            <score-part id="P1"><part-name>Piano</part-name></score-part>
          </part-list>
          <part id="P1">
            <measure number="1">
              <attributes>
                <divisions>2</divisions>
                <time><beats>4</beats><beat-type>4</beat-type></time>
              </attributes>
              <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>
              <note><grace/><pitch><step>D</step><octave>5</octave></pitch><type>eighth</type></note>
              <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>
              <note><grace slash="yes"/><pitch><step>F</step><octave>5</octave></pitch><type>eighth</type></note>
              <note><grace slash="yes"/><pitch><step>G</step><octave>5</octave></pitch><type>eighth</type></note>
              <note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>
              <note><grace/><pitch><step>B</step><octave>5</octave></pitch><type>16th</type></note>
              <note><pitch><step>A</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>
                <notations><ornaments><trill-mark/></ornaments></notations></note>
            </measure>
            <measure number="2">
              <note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration><type>half</type></note>
              <note><grace steal-time-previous="50"/><pitch><step>D</step><octave>5</octave></pitch><type>eighth</type></note>
              <note><grace/><pitch><step>C</step><octave>6</octave></pitch><type>16th</type></note>
              <note><grace/><pitch><step>E</step><octave>6</octave></pitch><type>16th</type></note>
              <note><grace/><chord/><pitch><step>G</step><octave>6</octave></pitch><type>16th</type></note>
              <note><pitch><step>E</step><octave>5</octave></pitch><duration>3</duration><type>quarter</type><dot/></note>
              <note><grace/><pitch><step>F</step><octave>5</octave></pitch><type>eighth</type></note>
              <note><grace/><pitch><step>G</step><octave>5</octave></pitch><type>eighth</type></note>
            </measure>
            <measure number="3">
              <note><grace/><cue/><pitch><step>G</step><octave>5</octave></pitch><type>eighth</type></note>
              <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>
              <note><grace/><pitch><step>A</step><octave>5</octave></pitch><type>eighth</type></note>
              <note><cue/><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>
              <note><grace/><pitch><step>B</step><octave>5</octave></pitch><type>quarter</type></note>
              <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type>
                <notations><slur type="start"/></notations></note>
              <note><grace/><pitch><step>F</step><octave>4</octave></pitch><type>eighth</type>
                <notations><slur type="stop"/></notations></note>
              <note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration><type>quarter</type></note>
            </measure>
          </part>
        </score-partwise>
        "#;

        insta::assert_debug_snapshot!(parse(src, false).unwrap());
    }
}
