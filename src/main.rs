mod musicxml;

use std::collections::BTreeMap;

use musicxml::MeasureItem;

const TICKS_PER_QUARTER_NOTE: u16 = 480;
const TICKS_PER_QUARTER_NOTE_F64: f64 = TICKS_PER_QUARTER_NOTE as f64;

fn main() {
    let src = std::fs::read_to_string("./schema/1.musicxml").unwrap();
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

                assert_eq!(
                    attributes.time,
                    vec![musicxml::Time {
                        beats: "4".into(),
                        beat_type: "4".into(),
                    }],
                );
            }
            MeasureItem::Note(note) => {
                let duration: f64 = note.duration.parse().unwrap();

                let ticks = ((duration / divisions) * TICKS_PER_QUARTER_NOTE_F64) as u32;

                if let Some(pitch) = note.pitch.as_ref() {
                    assert!(pitch.alter.is_none());
                    assert!(note.chord.is_none());

                    let pitch = midi_note_number(
                        pitch.step.chars().next().unwrap(),
                        pitch.octave.parse().unwrap(),
                        0, // pitch.alter,
                    );

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

                    let mut off = vec![];
                    let mut peek_iter = iter.clone();
                    while let Some(MeasureItem::Note(note)) = peek_iter.next() {
                        if let Some(pitch) = note.chord.as_ref().and(note.pitch.as_ref()) {
                            iter.next();

                            let pitch = midi_note_number(
                                pitch.step.chars().next().unwrap(),
                                pitch.octave.parse().unwrap(),
                                0, // pitch.alter,
                            );

                            off.push(pitch);
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
                        } else {
                            break;
                        }
                    }

                    position = position.saturating_add(ticks as usize);

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

fn midi_note_number(step: char, octave: u8, alter: u8) -> u8 {
    let base = match step {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => 0,
    };

    (octave + 1) * 12 + base + alter
}

#[cfg(test)]
mod tests {
    use super::*;

    // This is for xml, but html! ident has html syntax highlighting defined
    macro_rules! html {
        ( $($t:tt)* ) => {
            stringify!($($t)*)
        };
    }

    #[test]
    fn test_name() {
        let src = html!(
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
}
