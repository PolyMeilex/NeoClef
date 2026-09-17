use std::{borrow::Cow, ops::Range, str::FromStr, sync::Arc};

use super::Reader;
use quick_xml::events::{BytesStart, Event};

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("{kind}")]
pub struct Error {
    pub kind: ErrorKind,
    #[label("here")]
    pub span: miette::SourceSpan,
    #[source_code]
    pub src: miette::NamedSource<Arc<str>>,
}

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error(transparent)]
    Xml(quick_xml::Error),
    #[error("unexpected event: {0}")]
    UnexpectedEvent(String),
    #[error("unexpected value: {0}")]
    UnexpectedValue(String),
    #[error("missing element: {0}")]
    MissingElement(String),
    #[error("failed to parse value: {0}")]
    ParseError(String),
}

/// A wrapper around quick_xml::Reader that allows peeking at the next event.
pub struct XmlStream<'a> {
    reader: Reader<'a>,
    peeked: Option<Event<'a>>,
    last_span: Range<usize>,
    src: Arc<str>,
    name: String,
}

impl<'a> XmlStream<'a> {
    pub fn new(name: impl Into<String>, src: &'a str) -> Self {
        let mut reader = Reader::from_str(src);
        reader.config_mut().expand_empty_elements = true;
        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;
        Self {
            reader,
            peeked: None,
            last_span: 0..0,
            src: Arc::from(src),
            name: name.into(),
        }
    }

    /// Builds an error carrying the span of the most recently observed event.
    pub fn error(&self, kind: ErrorKind) -> Error {
        let start = self.last_span.start;
        let len = self.last_span.end - start;
        Error {
            kind,
            span: (start, len).into(),
            src: miette::NamedSource::new(self.name.clone(), self.src.clone()),
        }
    }

    /// Reads the next raw event straight from the underlying reader, recording its span.
    fn read_raw(&mut self) -> Result<Event<'a>, Error> {
        let start = self.reader.buffer_position() as usize;
        self.last_span = start..start;
        let e = self.reader.read_event();
        let end = self.reader.buffer_position() as usize;
        self.last_span = start..end;
        e.map_err(|e| self.error(ErrorKind::Xml(e)))
    }

    /// Consumes and returns the next event
    pub fn next_event(&mut self) -> Result<Event<'a>, Error> {
        if let Some(e) = self.peeked.take() {
            return Ok(e);
        }
        self.read_raw()
    }

    /// Looks at the next event without consuming it.
    pub fn peek(&mut self) -> Result<&Event<'a>, Error> {
        if self.peeked.is_none() {
            self.peeked = Some(self.read_raw()?);
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

    /// Safely skips to the end tag of the current element
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
                    return Err(self.error(ErrorKind::UnexpectedEvent(
                        "EOF while waiting for end tag".into(),
                    )));
                }
                _ => {}
            }
        }
    }

    pub fn required<T: ParseContent<'a>>(&mut self, tag: &[u8]) -> Result<T, Error> {
        if let Some(val) = self.optional(tag)? {
            Ok(val)
        } else {
            Err(self.error(ErrorKind::MissingElement(
                String::from_utf8_lossy(tag).into_owned(),
            )))
        }
    }

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
            Err(self.error(ErrorKind::MissingElement(
                String::from_utf8_lossy(tag).into_owned(),
            )))
        } else {
            Ok(results)
        }
    }

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

pub trait ParseContent<'a>: Sized {
    fn parse(stream: &mut XmlStream<'a>, tag: &[u8], start: &BytesStart<'a>)
    -> Result<Self, Error>;
}

impl<'a> ParseContent<'a> for String {
    fn parse(
        stream: &mut XmlStream<'a>,
        tag: &[u8],
        _start: &BytesStart<'a>,
    ) -> Result<Self, Error> {
        let mut content = String::new();
        loop {
            match stream.next_event()? {
                Event::Text(e) => content
                    .push_str(&e.decode().map_err(|e| {
                        stream.error(ErrorKind::Xml(quick_xml::Error::Encoding(e)))
                    })?),
                Event::End(e) if e.name().as_ref() == tag => break,
                Event::Eof => {
                    return Err(stream.error(ErrorKind::UnexpectedEvent("EOF in text".into())));
                }
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
            Event::Text(e) => {
                out = e
                    .decode()
                    .map_err(|e| stream.error(ErrorKind::Xml(quick_xml::Error::Encoding(e))))?
            }
            Event::End(e) if e.name().as_ref() == tag => break,
            Event::Eof => {
                return Err(stream.error(ErrorKind::UnexpectedEvent("EOF in text".into())));
            }
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
        s.trim().parse().map_err(|_| {
            stream.error(ErrorKind::ParseError(format!(
                "Invalid {}: {s:?}",
                std::any::type_name::<T>()
            )))
        })
    }
}

macro_rules! impl_parse_from_str {
    ($($t:ty),*) => {
        $(impl AutoFromStrParse for $t {})*
    };
}
impl_parse_from_str!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64, bool);
