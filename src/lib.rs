pub mod parser;

pub type Reader<'a> = quick_xml::reader::Reader<&'a [u8]>;
