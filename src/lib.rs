#![allow(clippy::single_match, clippy::let_unit_value)]

mod chord_symbol;
mod musicxml;

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

use musicxml::MeasureItem;
use quick_xml::events::{BytesStart, Event};

mod parser;

pub use parser::Error as ParseError;

#[cfg(test)]
mod test_support;

const TICKS_PER_QUARTER_NOTE: u16 = 480;
const TICKS_PER_QUARTER_NOTE_F64: f64 = TICKS_PER_QUARTER_NOTE as f64;

const SECOND: Duration = Duration::from_secs(1);

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

type Tick = usize;

#[derive(Debug, Clone)]
struct TimelineNote {
    start: Tick,
    end: Tick,
    key: u8,
    velocity: u8,
    off_velocity: u8,
    tie_start: bool,
    tie_stop: bool,
    staff: u8,
    channel: u8,
}

#[derive(Debug)]
struct Timeline {
    notes: Vec<TimelineNote>,
    /// Beats per second, from the tick on
    tempos: BTreeMap<Tick, f64>,
    /// The highest staff of the notes, one track each
    staves: u8,
}

impl Timeline {
    fn push(&mut self, note: TimelineNote) {
        self.staves = self.staves.max(note.staff);
        self.notes.push(note);
    }
}

struct RawScore {
    timeline: Timeline,
    /// The longest fermata at each tick
    fermatas: BTreeMap<Tick, f64>,
    /// Note and rest start ticks
    onsets: BTreeSet<Tick>,
    /// Resolved chord symbols, by staff and start
    chord_symbols: BTreeMap<u8, BTreeMap<Tick, Vec<u8>>>,
    /// Last note end tick
    end: Tick,
}

fn timeline(v: &musicxml::ScorePartwise, musescore_compat: bool) -> Timeline {
    resolve(collect(v, musescore_compat))
}

fn collect(v: &musicxml::ScorePartwise, musescore_compat: bool) -> RawScore {
    assert_eq!(v.part.len(), 1);

    let measures = v.part.iter().flat_map(|part| &part.measure);

    let mut divisions = 1.0;
    let mut position = 0.0f64;
    let mut staves = 1;
    // (start, length) of the last note or rest without `<chord/>`, chord notes share it
    let mut chord = (0.0f64, 0.0f64);

    let mut collected = RawScore {
        timeline: Timeline {
            notes: Vec::new(),
            tempos: BTreeMap::new(),
            staves: 1,
        },
        fermatas: BTreeMap::new(),
        onsets: BTreeSet::new(),
        chord_symbols: BTreeMap::new(),
        end: 0,
    };

    for measure in measures {
        for item in &measure.content {
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
                    let Some(msg) = full_note(note) else {
                        // Grace notes take no time, and aren't played, cause I don't care, it brings too much complexity
                        continue;
                    };

                    if !msg.chord {
                        chord = (position, msg.duration / divisions);
                        collected.onsets.insert(to_ticks(position));
                        match msg.kind {
                            musicxml::NoteKindStatePitchKind::Pitch(_)
                            | musicxml::NoteKindStatePitchKind::Rest(_) => position += chord.1,
                            _ => {}
                        }
                    }
                    let (start, quarters) = chord;

                    if let Some(stretch) = fermata_stretch(note) {
                        let longest = collected.fermatas.entry(to_ticks(start)).or_insert(stretch);
                        *longest = longest.max(stretch);
                    }

                    if let musicxml::NoteKindStatePitchKind::Pitch(pitch) = msg.kind
                        && msg.play
                    {
                        let (tie_start, tie_stop) = tie_kinds(msg.tie);
                        let end = to_ticks(start + quarters);
                        collected.end = collected.end.max(end);
                        collected.timeline.push(TimelineNote {
                            start: to_ticks(start),
                            end,
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
                        });
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
                    if let Some(keys) = chord_symbol::resolve(harmony) {
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

                        collected
                            .chord_symbols
                            .entry(staff)
                            .or_default()
                            .entry(to_ticks((position + offset).max(0.0)))
                            .or_default()
                            .extend(keys);
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

                        collected.timeline.tempos.insert(
                            to_ticks((position + offset).max(0.0)),
                            round_tempo(*tempo / 60.0),
                        );
                    }
                }
            }
        }
    }

    collected.end = collected.end.max(to_ticks(position));
    collected
}

fn resolve(raw: RawScore) -> Timeline {
    let RawScore {
        mut timeline,
        fermatas,
        onsets,
        chord_symbols,
        end,
    } = raw;

    // A fermata divides the tempo by its time stretch, and the tempo goes back
    // just before the next note or rest
    for (start, stretch) in fermatas {
        let bps = timeline
            .tempos
            .range(..=start)
            .next_back()
            .map_or(f64::from(DEFAULT_BPM) / 60.0, |(_, bps)| *bps);
        let stop = onsets
            .range(start + 1..)
            .next()
            .map_or(end, |onset| (*onset).min(end));

        timeline.tempos.insert(start, round_tempo(bps / stretch));
        timeline.tempos.insert(stop.saturating_sub(1), bps);
    }

    // if no tempo, 120 BPM default
    let first = timeline.tempos.entry(0).or_insert(0.0);
    if *first <= 0.0 {
        *first = f64::from(DEFAULT_BPM) / 60.0;
    }

    // Like MuseScore, chord symbols last until the next one on the same staff, or the end
    for (staff, symbols) in chord_symbols {
        let mut symbols = symbols.into_iter().peekable();
        while let Some((start, keys)) = symbols.next() {
            let next = symbols.peek().map_or(end, |(next, _)| *next);

            for key in keys {
                timeline.push(TimelineNote {
                    start,
                    end: next.max(start),
                    key,
                    velocity: DEFAULT_VELOCITY,
                    off_velocity: 0,
                    tie_start: false,
                    tie_stop: false,
                    staff,
                    channel: HARMONY_CHANNEL,
                });
            }
        }
    }

    timeline
}

type TrackEvents = BTreeMap<usize, Vec<midly::TrackEventKind<'static>>>;

fn to_smf(timeline: &Timeline) -> midly::Smf<'static> {
    let mut tracks: Vec<TrackEvents> = vec![TrackEvents::new(); usize::from(timeline.staves)];

    for (&tick, &bps) in &timeline.tempos {
        if bps <= 0.0 {
            continue;
        }
        let microseconds_per_quarter_note = (SECOND.as_micros() as f64 / bps).round() as u32;

        tracks[0]
            .entry(tick)
            .or_default()
            .push(midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(
                microseconds_per_quarter_note.into(),
            )));
    }

    for note in &timeline.notes {
        let events = &mut tracks[usize::from(note.staff.max(1)) - 1];

        if !note.tie_stop {
            events
                .entry(note.start)
                .or_default()
                .push(midly::TrackEventKind::Midi {
                    channel: note.channel.into(),
                    message: midly::MidiMessage::NoteOn {
                        key: note.key.into(),
                        vel: note.velocity.into(),
                    },
                });
        }

        if !note.tie_start {
            events
                .entry(note.end)
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

/// Like MuseScore, tempos are rounded to 6 decimal places
fn round_tempo(bps: f64) -> f64 {
    let scale = 10f64.powi(6);
    (bps * scale + 0.5).floor() / scale
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

/// How much longer a note or rest with a fermata lasts
fn fermata_stretch(note: &musicxml::Note) -> Option<f64> {
    use musicxml::FermataShape;

    note.notations
        .iter()
        .flat_map(|notations| &notations.fermata)
        .map(|fermata| match fermata.shape {
            FermataShape::DoubleAngled => 1.25,
            FermataShape::Angled | FermataShape::HalfCurve => 1.5,
            FermataShape::Normal | FermataShape::Curlew => 2.0,
            FermataShape::Square | FermataShape::DoubleDot => 3.0,
            FermataShape::DoubleSquare => 4.0,
        })
        .reduce(f64::max)
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
    fn fermata() {
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
              <direction>
                <direction-type><words>Adagio</words></direction-type>
                <sound tempo="60"/>
              </direction>
              <note>
                <pitch><step>C</step><octave>4</octave></pitch>
                <duration>2</duration>
                <notations><fermata>normal</fermata></notations>
              </note>
              <note>
                <rest/>
                <duration>1</duration>
                <notations><fermata>angled</fermata></notations>
              </note>
              <note>
                <pitch><step>D</step><octave>4</octave></pitch>
                <duration>1</duration>
              </note>
              <note>
                <chord/>
                <pitch><step>F</step><octave>4</octave></pitch>
                <duration>1</duration>
                <notations><fermata>square</fermata></notations>
              </note>
            </measure>
          </part>
        </score-partwise>
        );

        let src = simple_xml_format(src);
        insta::assert_debug_snapshot!(parse(&src, false).unwrap());
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
