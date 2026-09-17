#![allow(clippy::single_match, clippy::let_unit_value)]

mod musicxml;

use std::{borrow::Cow, collections::BTreeMap, time::Duration};

use musicxml::MeasureItem;
use quick_xml::events::{BytesStart, Event};

mod parser;

#[cfg(test)]
mod test_support;

const TICKS_PER_QUARTER_NOTE: u16 = 480;
const TICKS_PER_QUARTER_NOTE_F64: f64 = TICKS_PER_QUARTER_NOTE as f64;

const MINUTE: Duration = Duration::from_secs(60);

// 1s = 1_000_000µ
// 1m = 60_000_000µ

// BPM = 60_000_000 / MicrosecondsPerQuarterNote
// BPM * MicrosecondsPerQuarterNote = 60_000_000
// MicrosecondsPerQuarterNote = 60_000_000 / BPM

fn main() -> miette::Result<()> {
    env_logger::init();
    // let bytes = std::fs::read("/home/poly/Downloads/ODDTAXI.mid").unwrap();
    // let smf = midly::Smf::parse(&bytes).unwrap();
    //
    // dbg!(smf);
    //
    // return;

    // let src = std::fs::read_to_string("./schema/1.musicxml").unwrap();
    let src = std::fs::read_to_string("./schema/ODDTAXI.musicxml").unwrap();
    let smf = parse(&src)?;
    smf.save("out.mid").unwrap();
    Ok(())
}

fn parse(src: &str) -> Result<midly::Smf<'static>, parser::Error> {
    let mut stream = parser::XmlStream::new("ODDTAXI.musicxml", src);

    let v: musicxml::ScorePartwise = stream.required(b"score-partwise")?;
    dbg!(&v);

    println!("===========");

    assert_eq!(v.part.len(), 1);

    let mut iter = v
        .part
        .iter()
        .flat_map(|part| &part.measure)
        .flat_map(|measure| &measure.content);

    let mut divisions = 1.0;
    let mut position = 0usize;

    let mut events: BTreeMap<usize, Vec<midly::TrackEvent>> = BTreeMap::new();

    while let Some(item) = iter.next() {
        println!("{item:#?}");

        match item {
            MeasureItem::Attributes(attributes) => {
                if let Some(d) = attributes.divisions.as_ref() {
                    divisions = *d;
                }

                // assert_eq!(
                //     attributes.time,
                //     vec![musicxml::Time {
                //         beats: "4".into(),
                //         beat_type: "4".into(),
                //     }],
                // );
            }
            MeasureItem::Note(note) => {
                let musicxml::NoteKindState::Regular(msg) = &note.kind else {
                    todo!()
                };
                assert!(msg.chord.is_none());

                let duration = msg.duration;
                let ticks = ((duration / divisions) * TICKS_PER_QUARTER_NOTE_F64) as u32;

                if let musicxml::NoteKindStatePitchKind::Pitch(pitch) = &msg.kind {
                    let pitch =
                        midi_note_number(pitch.step, pitch.octave, pitch.alter.unwrap_or(0.0));

                    let ignore = msg
                        .tie
                        .first()
                        .map(|tie| tie.kind == musicxml::StartStop::Stop)
                        .unwrap_or(false);

                    if !ignore {
                        events.entry(position).or_default().push(midly::TrackEvent {
                            delta: 0.into(),
                            kind: midly::TrackEventKind::Midi {
                                channel: 0.into(),
                                message: midly::MidiMessage::NoteOn {
                                    key: pitch.into(),
                                    vel: 127.into(),
                                },
                            },
                        });
                    }

                    let mut off = vec![];
                    let mut peek_iter = iter.clone();
                    while let Some(MeasureItem::Note(note)) = peek_iter.next() {
                        let musicxml::NoteKindState::Regular(next) = &note.kind else {
                            break;
                        };
                        let Some(pitch) = next.chord.as_ref().and(match &next.kind {
                            musicxml::NoteKindStatePitchKind::Pitch(pitch) => Some(pitch),
                            _ => None,
                        }) else {
                            break;
                        };

                        iter.next();

                        let pitch =
                            midi_note_number(pitch.step, pitch.octave, pitch.alter.unwrap_or(0.0));

                        off.push(pitch);

                        let ignore = next
                            .tie
                            .first()
                            .map(|tie| tie.kind == musicxml::StartStop::Stop)
                            .unwrap_or(false);

                        if !ignore {
                            events.entry(position).or_default().push(midly::TrackEvent {
                                delta: 0.into(),
                                kind: midly::TrackEventKind::Midi {
                                    channel: 0.into(),
                                    message: midly::MidiMessage::NoteOn {
                                        key: pitch.into(),
                                        vel: 127.into(),
                                    },
                                },
                            });
                        }
                    }

                    position = position.saturating_add(ticks as usize);

                    if !ignore {
                        events.entry(position).or_default().push(midly::TrackEvent {
                            delta: 0.into(),
                            kind: midly::TrackEventKind::Midi {
                                channel: 0.into(),
                                message: midly::MidiMessage::NoteOff {
                                    key: pitch.into(),
                                    vel: 0.into(),
                                },
                            },
                        });
                    }

                    for pitch in off {
                        events.entry(position).or_default().push(midly::TrackEvent {
                            delta: 0.into(),
                            kind: midly::TrackEventKind::Midi {
                                channel: 0.into(),
                                message: midly::MidiMessage::NoteOff {
                                    key: pitch.into(),
                                    vel: 0.into(),
                                },
                            },
                        });
                    }
                } else if matches!(msg.kind, musicxml::NoteKindStatePitchKind::Rest(_)) {
                    // TODO: is_measure
                    position = position.saturating_add(ticks as usize);
                }
            }
            MeasureItem::Backup(backup) => {
                let duration: f64 = backup.duration;

                let ticks = (duration / divisions) * TICKS_PER_QUARTER_NOTE_F64;
                position = position.saturating_sub(ticks as usize)
            }
            MeasureItem::Forward(forward) => {
                let duration: f64 = forward.duration;

                let ticks = (duration / divisions) * TICKS_PER_QUARTER_NOTE_F64;
                position = position.saturating_add(ticks as usize)
            }
            MeasureItem::Print(_) => {}
            MeasureItem::Barline(_) => {}
            MeasureItem::Harmony(_) => {}
            MeasureItem::Direction(direction) => {
                if let Some(sound) = direction.sound.as_ref()
                    && let Some(tempo) = sound.tempo.as_ref()
                {
                    let tempo = tempo.round() as u64;

                    let microseconds_per_quarter_note = MINUTE.as_micros() as u64 / tempo;
                    let microseconds_per_quarter_note = microseconds_per_quarter_note as u32;

                    events.entry(position).or_default().push(midly::TrackEvent {
                        delta: 0.into(),
                        kind: midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(
                            microseconds_per_quarter_note.into(),
                        )),
                    });
                }
            }
        }
    }

    let mut track = vec![];

    let mut prev = 0;
    for (position, events) in events {
        let mut delta = position - prev;
        prev = position;

        for mut event in events {
            event.delta = (delta as u32).into();
            track.push(event);
            delta = 0;
        }
    }

    Ok(midly::Smf {
        header: midly::Header {
            format: midly::Format::SingleTrack,
            timing: midly::Timing::Metrical(midly::num::u15::new(TICKS_PER_QUARTER_NOTE)),
        },
        tracks: vec![track],
    })
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

    /// Runs the real `musicxml::ScorePartwise` parser over every score in
    /// the `test-files/lib` submodule.
    #[test]
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
        let midi = parse(&src).unwrap();
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

        let midi = parse(src).unwrap();
        insta::assert_debug_snapshot!(midi);
    }

    #[test]
    fn oddtaxi() {
        let src = std::fs::read_to_string("./schema/ODDTAXI.musicxml").unwrap();

        let mut stream = parser::XmlStream::new("ODDTAXI.musicxml", &src);

        let v: musicxml::ScorePartwise = stream.required(b"score-partwise").unwrap();

        assert_eq!(v.part.len(), 1);
        assert_eq!(v.part_list.score_part.len(), 1);

        let midi = parse(&src).unwrap();
        insta::assert_debug_snapshot!(midi);
    }

    #[test]
    fn grace_cue() {
        let src = xml!(
        <score-partwise>
          <work>
          </work>
          <credit/>
          <credit/>
          <part-list>
          </part-list>
          <part id="P1">
            <measure>
              <note>
                <grace slash="yes"/>
                <cue/>
                <pitch><step>D</step><octave>5</octave></pitch>
                <duration>1</duration>
              </note>
            </measure>
          </part>
          <!-- <part id="P1"> -->
          <!--   <measure> -->
          <!--     <note> -->
          <!--       <grace slash="yes"/> -->
          <!--       <cue/> -->
          <!--       <pitch><step>D</step><octave>5</octave></pitch> -->
          <!--       <duration>1</duration> -->
          <!--     </note> -->
          <!--   </measure> -->
          <!-- </part> -->
        </score-partwise>
        );

        // let span = &src[108..224];
        // println!("{span}");

        // parse2(src);
    }
}
