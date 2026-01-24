mod musicxml;

use std::{collections::BTreeMap, time::Duration};

use musicxml::MeasureItem;

const TICKS_PER_QUARTER_NOTE: u16 = 480;
const TICKS_PER_QUARTER_NOTE_F64: f64 = TICKS_PER_QUARTER_NOTE as f64;

const MINUTE: Duration = Duration::from_secs(60);

// 1s = 1_000_000µ
// 1m = 60_000_000µ

// BPM = 60_000_000 / MicrosecondsPerQuarterNote
// BPM * MicrosecondsPerQuarterNote = 60_000_000
// MicrosecondsPerQuarterNote = 60_000_000 / BPM

fn main() {
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
                    divisions = d.trim().parse().unwrap();
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

                    let pitch = midi_note_number(
                        pitch.step,
                        pitch.octave.parse().unwrap(),
                        pitch.alter.unwrap_or(0.0),
                    );

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
                                pitch.octave.parse().unwrap(),
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
                let duration: f64 = backup.duration.parse().unwrap();

                let ticks = (duration / divisions) * TICKS_PER_QUARTER_NOTE_F64;
                position = position.saturating_sub(ticks as usize)
            }
            MeasureItem::Print(_) => {}
            MeasureItem::Barline(_) => {}
            MeasureItem::Direction(direction) => {
                if let Some(sound) = direction.sound.as_ref() {
                    if let Some(tempo) = sound.tempo.as_ref() {
                        let tempo: f64 = tempo.parse().unwrap();
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

#[cfg(test)]
mod tests {
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
}
