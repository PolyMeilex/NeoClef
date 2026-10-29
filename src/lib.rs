#![allow(clippy::single_match, clippy::let_unit_value)]

mod parsing;
mod passes;

#[cfg(test)]
mod test_support;

use parsing::{musicxml, parser};
use passes::Policy;

pub use parser::Error as ParseError;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),
    #[error("unsupported score: {0}")]
    Unsupported(String),
}

pub fn parse(src: &str, musescore_compat: bool) -> Result<midly::Smf<'static>, Error> {
    let score: musicxml::ScorePartwise =
        parser::XmlStream::new("score.musicxml", src).required(b"score-partwise")?;
    log::debug!("{score:#?}");

    let part = single_part(&score)?;
    let policy = Policy::new(musescore_compat);

    let laid = passes::layout(part, &policy);
    let order = passes::playback_order(&laid);

    let mut perf = passes::unroll(&laid, &order);

    passes::tempo(&mut perf);
    passes::fermata(&mut perf);
    let mut notes = passes::sounding_notes(&perf);
    passes::chord_symbols(&perf, &mut notes);

    Ok(passes::encode(&perf.tempo, &notes))
}

fn single_part(score: &musicxml::ScorePartwise) -> Result<&musicxml::Part, Error> {
    match score.part.as_slice() {
        [part] => Ok(part),
        parts => Err(Error::Unsupported(format!(
            "expected exactly one part, found {}",
            parts.len()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use quick_xml::events::Event;

    use super::*;
    use crate::parsing::Reader;

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

    macro_rules! xml {
        ( $($t:tt)* ) => {
            simple_xml_format(stringify!($($t)*))
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

        test_support::musescore_diff(&src);

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

        let midi = parse(&src, false).unwrap();
        insta::assert_debug_snapshot!(midi);
    }

    #[test]
    fn oddtaxi() {
        let src = std::fs::read_to_string("./schema/ODDTAXI.musicxml").unwrap();
        test_support::musescore_diff(&src);

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

        test_support::musescore_diff(&src);

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

        test_support::musescore_diff(&src);

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

        test_support::musescore_diff(&src);

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

        test_support::musescore_diff(&src);

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

        test_support::musescore_diff(&src);

        let midi = parse(&src, false).unwrap();
        insta::assert_debug_snapshot!(midi);
    }

    /// Grace notes are skipped without moving other notes, cue notes are silent but take time
    #[test]
    fn grace_cue() {
        let src = xml!(
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
        );

        insta::assert_debug_snapshot!(parse(&src, false).unwrap());
    }
}
