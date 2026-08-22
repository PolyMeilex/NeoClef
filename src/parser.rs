use std::{borrow::Cow, str::FromStr};

use super::Reader;
use quick_xml::events::{BytesStart, Event};

#[derive(Debug)]
pub enum Error {
    Xml(quick_xml::Error),
    UnexpectedEvent(String),
    UnexpectedValue(String),
    MissingElement(String),
    ParseError(String),
}

impl From<quick_xml::Error> for Error {
    fn from(e: quick_xml::Error) -> Self {
        Error::Xml(e)
    }
}

/// A wrapper around quick_xml::Reader that allows peeking at the next event.
pub struct XmlStream<'a> {
    reader: Reader<'a>,
    peeked: Option<Event<'a>>,
}

impl<'a> XmlStream<'a> {
    pub fn new(mut reader: Reader<'a>) -> Self {
        reader.config_mut().expand_empty_elements = true;
        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;
        Self {
            reader,
            peeked: None,
        }
    }

    /// Consumes and returns the next event as owned data.
    pub fn next_event(&mut self) -> Result<Event<'a>, Error> {
        if let Some(e) = self.peeked.take() {
            return Ok(e);
        }
        let e = self.reader.read_event()?;
        Ok(e)
    }

    /// Looks at the next event without consuming it.
    pub fn peek(&mut self) -> Result<&Event<'a>, Error> {
        if self.peeked.is_none() {
            self.peeked = Some(self.reader.read_event()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    /// Ignores whitespace, comments, and XML declarations.
    pub fn skip_whitespace(&mut self) -> Result<(), Error> {
        loop {
            match self.peek()? {
                Event::Text(e) if e.iter().all(|b| b.is_ascii_whitespace()) => {
                    self.next_event()?;
                }
                Event::Comment(_) | Event::Decl(_) | Event::DocType(_) => {
                    self.next_event()?;
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Safely skips to the end tag of the current element, handling nested elements with the same tag name.
    /// This is great for future-proofing against new/unsupported MusicXML extensions.
    pub fn skip_to_end(&mut self, tag: &[u8]) -> Result<(), Error> {
        let mut depth = 0;
        loop {
            match self.next_event()? {
                Event::Start(e) if e.name().as_ref() == tag => depth += 1,
                Event::End(e) if e.name().as_ref() == tag => {
                    if depth == 0 {
                        return Ok(());
                    }
                    depth -= 1;
                }
                Event::Eof => {
                    return Err(Error::UnexpectedEvent(
                        "EOF while waiting for end tag".into(),
                    ));
                }
                _ => {}
            }
        }
    }

    // ==========================================
    // HIGHER LEVEL COMBINATORS
    // ==========================================

    /// Parses exactly one instance of a specific element.
    pub fn required<T: ParseContent<'a>>(&mut self, tag: &[u8]) -> Result<T, Error> {
        if let Some(val) = self.optional(tag)? {
            Ok(val)
        } else {
            Err(Error::MissingElement(
                String::from_utf8_lossy(tag).into_owned(),
            ))
        }
    }

    /// Parses an element if the next meaningful tag matches.
    pub fn optional<T: ParseContent<'a>>(&mut self, tag: &[u8]) -> Result<Option<T>, Error> {
        self.skip_whitespace()?;
        match self.peek()? {
            Event::Start(e) if e.name().as_ref() == tag => {
                let e = match self.next_event()? {
                    Event::Start(e) => e,
                    _ => unreachable!(),
                };
                Ok(Some(T::parse(self, tag, &e)?))
            }
            Event::Empty(_) => unreachable!("disabled in the config"),
            _ => Ok(None),
        }
    }

    pub fn zero_or_more<T: ParseContent<'a>>(&mut self, tag: &[u8]) -> Result<Vec<T>, Error> {
        let mut results = Vec::new();
        while let Some(item) = self.optional(tag)? {
            results.push(item);
        }
        Ok(results)
    }

    pub fn one_or_more<T: ParseContent<'a>>(&mut self, tag: &[u8]) -> Result<Vec<T>, Error> {
        let results = self.zero_or_more(tag)?;
        if results.is_empty() {
            Err(Error::MissingElement(
                String::from_utf8_lossy(tag).into_owned(),
            ))
        } else {
            Ok(results)
        }
    }

    /// Returns the name of the next XML tag without consuming it.
    /// This makes `<choice>` elements trivial via a standard Rust `match` block.
    pub fn peek_tag(&mut self) -> Result<Option<&[u8]>, Error> {
        self.skip_whitespace()?;
        match self.peek()? {
            Event::Start(e) | Event::Empty(e) => Ok(Some(e.name().0)),
            _ => Ok(None),
        }
    }

    pub fn flatten<T: ParseContentFlat<'a>>(&mut self) -> Result<T, Error> {
        T::parse(self)
    }
}

pub trait ParseContentFlat<'a>: Sized {
    fn parse(stream: &mut XmlStream<'a>) -> Result<Self, Error>;
}

/// A trait for anything that can be deserialized from XML contents.
/// The `start` event is provided so you can read attributes.
pub trait ParseContent<'a>: Sized {
    fn parse(stream: &mut XmlStream<'a>, tag: &[u8], start: &BytesStart<'a>)
    -> Result<Self, Error>;
}

// === Implementations for Primitives & Helpers ===

/// Implementation for text nodes `<type>quarter</type>` -> `String`
impl<'a> ParseContent<'a> for String {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, Error> {
        let mut content = String::new();
        loop {
            match stream.next_event()? {
                Event::Text(e) => content.push_str(&e.decode().expect("TODO")),
                Event::End(e) if e.name().as_ref() == tag => break,
                Event::Eof => return Err(Error::UnexpectedEvent("EOF in text".into())),
                _ => {}
            }
        }
        Ok(content)
    }
}

impl<'a> ParseContent<'a> for Cow<'a, str> {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, Error> {
        parse_next_text(stream, tag, start)
    }
}

/// Unit type `()` acts as a simple presence flag (e.g., `<grace/>`)
impl<'a> ParseContent<'a> for () {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, Error> {
        stream.skip_to_end(tag)?;
        Ok(())
    }
}

// This works only for one-word strings and multi word strings that are not separated by comments
fn parse_next_text<'a>(
    stream: &mut XmlStream<'a>,
    tag: &[u8],
    _start: &BytesStart<'a>,
) -> Result<Cow<'a, str>, Error> {
    let mut out = Cow::Borrowed("");
    loop {
        match stream.next_event()? {
            Event::Text(e) => out = e.decode().expect("TODO"),
            Event::End(e) if e.name().as_ref() == tag => break,
            Event::Eof => return Err(Error::UnexpectedEvent("EOF in text".into())),
            _ => {}
        }
    }
    Ok(out)
}

trait AutoFromStrParse: FromStr {}

impl<'a, T: AutoFromStrParse> ParseContent<'a> for T {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        start: &BytesStart<'a>,
    ) -> Result<Self, Error> {
        let s = parse_next_text(stream, tag, start)?;
        s.trim()
            .parse()
            .map_err(|_| Error::ParseError(format!("Invalid {}", stringify!($t))))
    }
}

macro_rules! impl_parse_from_str {
    ($($t:ty),*) => {
        $(impl AutoFromStrParse for $t {})*
    };
}
impl_parse_from_str!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64, bool);
