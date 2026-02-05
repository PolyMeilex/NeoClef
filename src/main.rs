#![allow(clippy::single_match)]

mod musicxml;

use std::{collections::BTreeMap, str::FromStr, time::Duration};

use log::error;
use musicxml::MeasureItem;
use quick_xml::{events::Event, name::QName};

const TICKS_PER_QUARTER_NOTE: u16 = 480;
const TICKS_PER_QUARTER_NOTE_F64: f64 = TICKS_PER_QUARTER_NOTE as f64;

const MINUTE: Duration = Duration::from_secs(60);

// 1s = 1_000_000µ
// 1m = 60_000_000µ

// BPM = 60_000_000 / MicrosecondsPerQuarterNote
// BPM * MicrosecondsPerQuarterNote = 60_000_000
// MicrosecondsPerQuarterNote = 60_000_000 / BPM

fn main() {
    env_logger::init();
    // let bytes = std::fs::read("/home/poly/Downloads/ODDTAXI.mid").unwrap();
    // let smf = midly::Smf::parse(&bytes).unwrap();
    //
    // dbg!(smf);
    //
    // return;

    // let src = std::fs::read_to_string("./schema/1.musicxml").unwrap();
    let src = std::fs::read_to_string("./schema/ODDTAXI.musicxml").unwrap();
    let smf = parse(&src);
    smf.save("out.mid").unwrap();
}

fn parse(src: &str) -> midly::Smf {
    {
        let mut reader = quick_xml::Reader::from_str(src);

        loop {
            match reader.read_event().unwrap() {
                Event::Start(b) => match b.name().as_ref() {
                    b"score-partwise" => {
                        let score = musicxml::ScorePartwise::parse(&mut reader, &b);
                        dbg!(score);
                    }
                    _ => {
                        todo!();
                        reader.read_to_end(b.name()).unwrap();
                    }
                },
                Event::End(b) => {
                    assert_eq!(b.name().as_ref(), b"score-partwise");
                    break;
                }
                Event::Eof => {
                    break;
                }
                _ => {}
            }
        }

        std::process::exit(0);
    }

    let v: musicxml::ScorePartwise = quick_xml::de::from_str(src).unwrap();

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
                let duration: f64 = note.duration.parse().unwrap();

                let ticks = ((duration / divisions) * TICKS_PER_QUARTER_NOTE_F64) as u32;

                if let Some(pitch) = note.pitch.as_ref() {
                    assert!(note.chord.is_none());

                    let pitch =
                        midi_note_number(pitch.step, pitch.octave, pitch.alter.unwrap_or(0.0));

                    let ignore = note
                        .tie
                        .as_ref()
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
                        if let Some(pitch) = note.chord.as_ref().and(note.pitch.as_ref()) {
                            iter.next();

                            let pitch = midi_note_number(
                                pitch.step,
                                pitch.octave,
                                pitch.alter.unwrap_or(0.0),
                            );

                            off.push(pitch);

                            let ignore = note
                                .tie
                                .as_ref()
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
                        } else {
                            break;
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
                } else if note.rest.is_some() {
                    // TODO: is_measure
                    position = position.saturating_add(ticks as usize);
                }
            }
            MeasureItem::Backup(backup) => {
                let duration: f64 = backup.duration;

                let ticks = (duration / divisions) * TICKS_PER_QUARTER_NOTE_F64;
                position = position.saturating_sub(ticks as usize)
            }
            MeasureItem::Print(_) => {}
            MeasureItem::Barline(_) => {}
            MeasureItem::Direction(direction) => {
                if let Some(sound) = direction.sound.as_ref() {
                    if let Some(tempo) = sound.tempo.as_ref() {
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

    midly::Smf {
        header: midly::Header {
            format: midly::Format::SingleTrack,
            timing: midly::Timing::Metrical(midly::num::u15::new(TICKS_PER_QUARTER_NOTE)),
        },
        tracks: vec![track],
    }
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

trait ReaderExt<'b> {
    fn read_text_and_parse<T: FromStr>(&mut self, end: QName) -> Option<T>
    where
        T::Err: std::fmt::Display;
}

impl<'b> ReaderExt<'b> for quick_xml::reader::Reader<&'b [u8]> {
    fn read_text_and_parse<T: FromStr>(&mut self, end: QName) -> Option<T>
    where
        T::Err: std::fmt::Display,
    {
        self.read_text(end)
            .inspect_err(|err| log::error!("{err}"))
            .ok()
            .and_then(|text| {
                text.parse::<T>()
                    .inspect_err(|err| log::error!("{err}"))
                    .ok()
            })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    macro_rules! xml {
        ( $($t:tt)* ) => {
            stringify!($($t)*)
        };
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

        let midi = parse(src);
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

        let midi = parse(src);
        insta::assert_debug_snapshot!(midi);
    }

    use quick_xml::{
        events::{BytesEnd, BytesStart, BytesText, Event},
        name::QName,
    };

    #[derive(thiserror::Error, Debug)]
    pub enum MusicXmlError {
        #[error("Unexpected end of file")]
        Eof,
    }

    type Result<T, E = MusicXmlError> = std::result::Result<T, E>;

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum MyEvent<'a> {
        /// Start tag (with attributes) `<tag attr="value">`.
        Start(BytesStart<'a>),
        /// End tag `</tag>`.
        End(BytesEnd<'a>),
        /// Escaped character data between tags.
        Text(BytesText<'a>),
        /// End of XML document.
        Eof,
    }

    impl<'a> MyEvent<'a> {
        pub fn new(event: quick_xml::events::Event<'a>) -> Option<Self> {
            Some(match event {
                Event::Start(b) => Self::Start(b),
                Event::End(b) => Self::End(b),
                Event::Text(b) => Self::Text(b),
                Event::Eof => Self::Eof,
                _ => return None,
            })
        }
    }

    trait ReaderExt<'b> {
        fn read_start(&mut self) -> Result<BytesStart<'b>>;
    }

    impl<'b> ReaderExt<'b> for quick_xml::reader::Reader<&'b [u8]> {
        fn read_start(&mut self) -> Result<BytesStart<'b>> {
            loop {
                match self.read_event() {
                    Ok(Event::Start(e)) => break Ok(e),
                    Ok(Event::End(_)) => todo!("got end instead of start"),
                    Ok(Event::Eof) => break Err(MusicXmlError::Eof),
                    _ => {}
                }
            }
        }
    }

    struct ReadUtils<'a, 'b> {
        reader: &'a mut Reader<'b>,
        start: &'a mut BytesStart<'b>,
    }

    impl<'a, 'b> ReadUtils<'a, 'b> {}

    #[derive(Debug)]
    pub struct ScorePartwise {
        pub work: Option<()>,
        pub movement_number: Option<()>,
        pub movement_title: Option<()>,
        pub identification: Option<()>,
        pub defaults: Option<()>,
        pub credit: Vec<()>,
        pub part_list: (),
        pub part: Vec<()>,
    }

    trait State: Sized + 'static + Copy + Ord {
        const VARIANTS: &[Self];
        const LAST: Self = Self::VARIANTS[Self::VARIANTS.len() - 1];

        fn from_xml(start: &BytesStart<'_>) -> Option<Self>;

        fn as_usize(&self) -> usize;

        fn next(&self) -> Self {
            Self::VARIANTS
                .get(self.as_usize() + 1)
                .copied()
                .unwrap_or(Self::LAST)
        }

        fn previous(&self) -> Self {
            Self::VARIANTS[self.as_usize() - 1]
        }

        fn allow_for_one_more(&mut self) {
            *self = self.previous();
        }

        fn is_allowed(&self, got: Self) -> bool {
            got >= *self
        }
    }

    macro_rules! gen_state {
        ($(#[$meta:meta])* $vis:vis enum $name:ident {
            $($variant:ident),+ $(,)?
        }) => {
            $(#[$meta])*
            $vis enum $name {
                $($variant),+
            }

            impl State for $name {
                const VARIANTS: &[Self] = &[
                    $( Self::$variant ),+
                ];

                fn from_xml(start: &BytesStart<'_>) -> Option<Self> {
                    match start.name().as_ref() {
                        $(b if {
                            let kebab = const_str::convert_ascii_case!(kebab, stringify!($variant));
                            b == kebab.as_bytes()
                        } => Some(Self::$variant),)+
                        _ => None,
                    }
                }

                fn as_usize(&self) -> usize {
                    *self as usize
                }
            }
        };
    }

    impl ScorePartwise {
        pub fn parse(reader: &mut Reader) -> Self {
            let mut work = None;
            let mut movement_number = None;
            let mut movement_title = None;
            let mut identification = None;
            let mut defaults = None;
            let mut credit = vec![];
            let mut part_list = ();
            let mut part = vec![];

            gen_state!(
                #[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
                enum Field {
                    Work,
                    MovementNumber,
                    MovementTitle,
                    Identification,
                    Defaults,
                    Credit,
                    PartList,
                    Part,
                }
            );

            assert_eq!(
                reader.read_start().unwrap().name().as_ref(),
                b"score-partwise",
                "TODO: Non partwise",
            );

            let mut expected_field = Field::Work;

            dbg!(expected_field);

            loop {
                let Some(event) = MyEvent::new(reader.read_event().unwrap()) else {
                    continue;
                };

                match event {
                    MyEvent::Start(b) => {
                        let field = Field::from_xml(&b).unwrap();

                        assert!(expected_field.is_allowed(field), "Out of order field");
                        expected_field = field.next();

                        match field {
                            Field::Work => {
                                work = Some(());
                            }
                            Field::MovementNumber => {
                                movement_number = Some(());
                            }
                            Field::MovementTitle => {
                                movement_title = Some(());
                            }
                            Field::Identification => {
                                identification = Some(());
                            }
                            Field::Defaults => {
                                defaults = Some(());
                            }
                            Field::Credit => {
                                credit.push(());
                                expected_field.allow_for_one_more();
                            }
                            Field::PartList => {
                                part_list = ();
                            }
                            Field::Part => {
                                part.push(());
                                expected_field.allow_for_one_more();

                                continue;
                            }
                        }

                        reader.read_to_end(b.name()).unwrap();
                        dbg!(expected_field);
                    }
                    MyEvent::End(b) if b.name().as_ref() == b"score-partwise" => break,
                    MyEvent::End(b) => {
                        todo!("Unexpected end")
                    }
                    MyEvent::Text(b) => todo!("Text"),
                    MyEvent::Eof => todo!("Eof"),
                }
            }

            // let mut util = ReadUtils {
            //     reader,
            //     start: &mut start,
            // };
            //
            // util.expect_tag(b"score-partwise");

            // util.optional_ignore_children(b"work", |_| {
            //     work = Some(());
            // });
            //
            // util.optional_ignore_children(b"movement-number", |_| {
            //     movement_number = Some(());
            // });
            //
            // util.optional_ignore_children(b"movement-title", |_| {
            //     movement_title = Some(());
            // });
            //
            // util.optional_ignore_children(b"identification", |_| {
            //     identification = Some(());
            // });
            //
            // util.optional_ignore_children(b"defaults", |_| {
            //     defaults = Some(());
            // });
            //
            // util.zero_or_more(b"credit", |r| {
            //     credit.push(());
            //     r.ignore_children();
            // });
            //
            // util.required(b"part-list", |r| {
            //     part_list = ();
            //     r.ignore_children();
            // });
            //
            // assert_eq!(util.start.name().as_ref(), b"part", "TODO");
            //
            // util.one_or_more(b"part", |r| {
            //     part.push(());
            //     r.ignore_children();
            // });

            // while start.name().as_ref() == b"part" {
            //     part.push(());
            //     reader.read_to_end(start.to_end().name()).expect("TODO");
            //     if let Some(next) = reader.read_start_maybe() {
            //         start = next;
            //     } else {
            //         break;
            //     }
            // }

            Self {
                work,
                movement_number,
                movement_title,
                identification,
                defaults,
                credit,
                part_list,
                part,
            }
        }
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
              </note>
            </measure>
          </part>
          <part id="P1">
            <measure>
              <note>
                <grace slash="yes"/>
                <cue/>
                <pitch><step>D</step><octave>5</octave></pitch>
              </note>
            </measure>
          </part>
        </score-partwise>
        );

        {
            let mut reader = Reader::from_str(src);
            reader.config_mut().trim_text(true);
            reader.config_mut().expand_empty_elements = true;

            let score = ScorePartwise::parse(&mut reader);

            dbg!(score);

            // reader.read_start_named(b"score-partwise");
            //
            // loop {
            //     match dbg!(reader.read_event()) {
            //         Err(e) => panic!("Error at position {}: {:?}", reader.error_position(), e),
            //         // exits the loop when reaching end of file
            //         Ok(Event::Eof) => break,
            //
            //         Ok(Event::Start(e)) => match e.name().as_ref() {
            //             b"tag1" => println!(
            //                 "attributes values: {:?}",
            //                 e.attributes().map(|a| a.unwrap().value).collect::<Vec<_>>()
            //             ),
            //             b"tag2" => count += 1,
            //             _ => (),
            //         },
            //         Ok(Event::Text(e)) => txt.push(e.decode().unwrap().into_owned()),
            //
            //         // There are several other `Event`s we do not consider here
            //         _ => (),
            //     }
            // }
        }

        // let v: musicxml::ScorePartwise = quick_xml::de::from_str(src).unwrap();
        //
        // dbg!(v);

        // insta::assert_debug_snapshot!(midi);
    }
}
