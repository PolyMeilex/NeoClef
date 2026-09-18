use std::collections::HashMap;

use anyhow::Result;

use neo_clef::parser::{self, ParseContent, XmlStream};
use proc_macro2::TokenStream;
use quick_xml::events::BytesStart;
use quote::quote;
use syn::{Ident, LitByteStr};

const INCLUDE: &[&[u8]] = &[
    b"work",
    b"opus",
    // b"score-header",
    b"defaults",
    b"identification",
    b"layout",
    b"page-layout",
];

fn main() -> Result<()> {
    env_logger::init();

    let src = std::fs::read_to_string("./schema/musicxml.xsd").unwrap();

    let mut stream = XmlStream::new("musicxml.xsd", &src);

    let _schema = stream.required::<Schema>(b"xs:schema")?;

    Ok(())
}

#[derive(Debug)]
enum XsTypeKind {
    ComplexType(ComplexType),
    Group(Group),
}

struct Schema {}

impl ParseContent<'_> for Schema {
    fn parse(
        stream: &mut XmlStream<'_>,
        tag: &[u8],
        start: &BytesStart<'_>,
    ) -> Result<Self, parser::Error> {
        let mut types = HashMap::new();

        while let Some(start) = stream.peek_tag_full()? {
            if matches!(start.name().as_ref(), b"xs:annotation" | b"xs:import") {
                let tag = start.name().as_ref().to_vec();
                stream.required::<()>(&tag)?;
                continue;
            }

            let name = start
                .attributes()
                .map(|a| a.unwrap())
                .find(|a| a.key.as_ref() == b"name")
                .map(|a| str::from_utf8(a.value.as_ref()).unwrap().to_string())
                .unwrap();

            match start.name().as_ref() {
                b"xs:complexType" => {
                    types.insert(
                        name,
                        XsTypeKind::ComplexType(stream.required::<ComplexType>(b"xs:complexType")?),
                    );
                }
                b"xs:group" => {
                    types.insert(
                        name,
                        XsTypeKind::Group(stream.required::<Group>(b"xs:group")?),
                    );
                }
                tag => {
                    let tag = tag.to_vec();
                    stream.required::<()>(&tag)?;
                }
            }
        }

        stream.skip_to_end(tag)?;

        for (name, desc) in types {
            if !INCLUDE.contains(&name.as_bytes()) {
                continue;
            }

            match desc {
                XsTypeKind::ComplexType(ty) => {
                    let ident = quote::format_ident!("{}", kebkab_to_camel(&name));

                    let sequence = &ty.sequence;

                    let fields = sequence.elements.iter().map(|e| match e {
                        SequenceChild::Element(e) => {
                            let key = quote::format_ident!("{}", kebkab_to_snake(&e.name));
                            let ty = quote::format_ident!("{}", to_rs_type(&e.type_));

                            match MinMaxOccur::new(e.min_occurs, e.max_occurs) {
                                MinMaxOccur::Optional => quote! { #key: Option<#ty> },
                                MinMaxOccur::Required => quote! { #key: #ty },
                                MinMaxOccur::ZeroOrMore | MinMaxOccur::OneOrMore => {
                                    quote! { #key: Vec<#ty> }
                                }
                            }
                        }
                        SequenceChild::Group(grp) => {
                            let key = quote::format_ident!("{}", kebkab_to_snake(grp));
                            let ty = quote::format_ident!("{}", to_rs_type(grp));
                            quote! { #key: #ty }
                        }
                    });

                    let doc = format!(
                        " https://musicxml.formats.music/musicxml-reference/elements/{name}"
                    );

                    let src = quote! {
                        #[doc = #doc]
                        #[derive(Debug, Clone)]
                        pub struct #ident {
                            #(pub #fields),*
                        }
                    };
                    let src = src.to_string();

                    let syntax_tree = syn::parse_file(&src).unwrap();
                    let src = prettyplease::unparse(&syntax_tree);

                    println!("{src}");

                    let imp = gen_seq_parse_content(&ident, &sequence.elements);
                    println!("{imp}");
                }
                XsTypeKind::Group(ty) => {
                    let ident = quote::format_ident!("{}", kebkab_to_camel(&name));

                    let sequence = &ty.sequence;

                    let fields = sequence.elements.iter().map(|e| match e {
                        SequenceChild::Element(e) => {
                            let key = quote::format_ident!("{}", kebkab_to_snake(&e.name));
                            let ty = quote::format_ident!("{}", to_rs_type(&e.type_));

                            match MinMaxOccur::new(e.min_occurs, e.max_occurs) {
                                MinMaxOccur::Optional => quote! { #key: Option<#ty> },
                                MinMaxOccur::Required => quote! { #key: #ty },
                                MinMaxOccur::ZeroOrMore | MinMaxOccur::OneOrMore => {
                                    quote! { #key: Vec<#ty> }
                                }
                            }
                        }
                        SequenceChild::Group(grp) => {
                            let key = quote::format_ident!("{}", kebkab_to_snake(grp));
                            let ty = quote::format_ident!("{}", to_rs_type(grp));
                            quote! { #key: #ty }
                        }
                    });

                    let doc = format!(
                        " https://musicxml.formats.music/musicxml-reference/elements/{name}"
                    );

                    let src = quote! {
                        #[doc = #doc]
                        #[derive(Debug, Clone)]
                        pub struct #ident {
                            #(pub #fields),*
                        }
                    };
                    let src = src.to_string();

                    let syntax_tree = syn::parse_file(&src).unwrap();
                    let src = prettyplease::unparse(&syntax_tree);

                    println!("{src}");

                    let imp = gen_seq_parse_content_flat(&ident, &sequence.elements);
                    println!("{imp}");
                }
            }
        }

        Ok(Self {})
    }
}

pub fn kebkab_to_snake(input: &str) -> String {
    input.replace('-', "_")
}

pub fn kebkab_to_camel(input: &str) -> String {
    input
        .split('-')
        .flat_map(|s| {
            let mut first = true;
            s.chars().map(move |c| {
                if first {
                    first = false;
                    c.to_ascii_uppercase()
                } else {
                    c
                }
            })
        })
        .collect::<String>()
}

pub fn to_rs_type(input: &str) -> String {
    match input {
        "xs:string" => String::from("String"),
        ty => kebkab_to_camel(ty),
    }
}

#[derive(Debug, Clone, Copy)]
enum MinMaxOccur {
    Optional,
    Required,
    ZeroOrMore,
    OneOrMore,
}

impl MinMaxOccur {
    fn new(min: u32, max: u32) -> Self {
        if min == 0 && max == 1 {
            Self::Optional
        } else if min == 1 && max == 1 {
            Self::Required
        } else if min == 0 && max >= 1 {
            Self::ZeroOrMore
        } else if min == 1 && max >= 1 {
            Self::OneOrMore
        } else {
            todo!()
        }
    }
}

struct SequenceElementRs {
    key: LitByteStr,
    ty: Ident,
    field: Ident,
    min_max: MinMaxOccur,
    flat: bool,
}

impl SequenceElementRs {
    fn new(e: &Element) -> Self {
        let key = LitByteStr::new(e.name.as_bytes(), proc_macro2::Span::call_site());
        let ty = Ident::new(&to_rs_type(&e.type_), proc_macro2::Span::call_site());
        let field = Ident::new(&kebkab_to_snake(&e.name), proc_macro2::Span::call_site());
        let min_max = MinMaxOccur::new(e.min_occurs, e.max_occurs);

        Self {
            key,
            ty,
            field,
            min_max,
            flat: false,
        }
    }

    fn new_grp(grp: &str) -> Self {
        let key = LitByteStr::new(grp.as_bytes(), proc_macro2::Span::call_site());
        let ty = Ident::new(&to_rs_type(grp), proc_macro2::Span::call_site());
        let field = Ident::new(&kebkab_to_snake(grp), proc_macro2::Span::call_site());
        let min_max = MinMaxOccur::new(1, 1);

        Self {
            key,
            ty,
            field,
            min_max,
            flat: true,
        }
    }

    fn gen_parse(&self) -> TokenStream {
        let field = &self.field;
        let ty = &self.ty;
        let key = &self.key;

        if self.flat {
            return quote! {
                let #field: #ty = stream.flatten()?;
            };
        }

        match self.min_max {
            MinMaxOccur::Optional => {
                quote! {
                    let #field: Option<#ty> = stream.optional(#key)?;
                }
            }
            MinMaxOccur::Required => {
                quote! {
                    let #field: #ty = stream.required(#key)?;
                }
            }
            MinMaxOccur::ZeroOrMore => {
                quote! {
                    let #field: Vec<#ty> = stream.zero_or_more(#key)?;
                }
            }
            MinMaxOccur::OneOrMore => {
                todo!()
            }
        }
    }
}

fn gen_seq_parse_content(name: &Ident, elements: &[SequenceChild]) -> String {
    let elements: Vec<_> = elements
        .iter()
        .map(|e| match e {
            SequenceChild::Element(element) => SequenceElementRs::new(element),
            SequenceChild::Group(grp) => SequenceElementRs::new_grp(grp),
        })
        .collect();
    let vars = elements.iter().map(|e| e.gen_parse());
    let fields = elements.iter().map(|e| &e.field);

    let src = quote! {
        impl<'a> ParseContent<'a> for #name {
            fn parse(
                stream: &mut XmlStream<'a>,
                tag: &[u8],
                _start: &BytesStart<'a>,
            ) -> Result<Self, parser::Error> {
                #(#vars);*;

                stream.skip_to_end(tag)?;

                Ok(Self { #(#fields),* })
            }
        }
    };

    let syntax_tree = syn::parse_file(&src.to_string()).unwrap();
    prettyplease::unparse(&syntax_tree)
}

fn gen_seq_parse_content_flat(name: &Ident, elements: &[SequenceChild]) -> String {
    let elements: Vec<_> = elements
        .iter()
        .map(|e| match e {
            SequenceChild::Element(element) => SequenceElementRs::new(element),
            SequenceChild::Group(grp) => SequenceElementRs::new_grp(grp),
        })
        .collect();
    let vars = elements.iter().map(|e| e.gen_parse());
    let fields = elements.iter().map(|e| &e.field);

    let src = quote! {
        impl<'a> ParseContentFlat<'a> for #name {
            fn parse(stream: &mut XmlStream<'a>,) -> Result<Self, parser::Error> {
                #(#vars);*;
                Ok(Self { #(#fields),* })
            }
        }
    };

    let syntax_tree = syn::parse_file(&src.to_string()).unwrap();
    prettyplease::unparse(&syntax_tree)
}

#[derive(Debug)]
struct ComplexType {
    sequence: Sequence,
}

impl ParseContent<'_> for ComplexType {
    fn parse(
        stream: &mut XmlStream<'_>,
        tag: &[u8],
        start: &BytesStart<'_>,
    ) -> Result<Self, parser::Error> {
        let name = start
            .attributes()
            .map(|a| a.unwrap())
            .find(|a| a.key.as_ref() == b"name")
            .map(|a| a.value)
            .unwrap();

        let include = INCLUDE.contains(&name.as_ref());

        let mut sequence = Sequence::default();

        if include {
            loop {
                match stream.peek_tag()? {
                    Some(b"xs:annotation") => {
                        stream.required::<()>(b"xs:annotation")?;
                    }
                    Some(b"xs:attribute") => {
                        stream.required::<()>(b"xs:attribute")?;
                    }
                    Some(b"xs:attributeGroup") => {
                        stream.required::<()>(b"xs:attributeGroup")?;
                    }
                    Some(b"xs:sequence") => {
                        sequence = stream.required::<Sequence>(b"xs:sequence")?;
                    }
                    Some(tag) => {
                        let tag = tag.to_vec();
                        println!("{}", str::from_utf8(&tag).unwrap());
                        stream.required::<()>(&tag)?
                    }
                    None => break,
                }
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self { sequence })
    }
}

#[derive(Debug)]
struct Group {
    sequence: Sequence,
}

impl ParseContent<'_> for Group {
    fn parse(
        stream: &mut XmlStream<'_>,
        tag: &[u8],
        start: &BytesStart<'_>,
    ) -> Result<Self, parser::Error> {
        let name = start
            .attributes()
            .map(|a| a.unwrap())
            .find(|a| a.key.as_ref() == b"name")
            .map(|a| a.value)
            .unwrap();

        let include = INCLUDE.contains(&name.as_ref());

        let mut sequence = Sequence::default();

        if include {
            loop {
                match stream.peek_tag()? {
                    Some(b"xs:annotation") => {
                        stream.required::<()>(b"xs:annotation")?;
                    }
                    Some(b"xs:attribute") => {
                        stream.required::<()>(b"xs:attribute")?;
                    }
                    Some(b"xs:attributeGroup") => {
                        stream.required::<()>(b"xs:attributeGroup")?;
                    }
                    Some(b"xs:sequence") => {
                        sequence = stream.required::<Sequence>(b"xs:sequence")?;
                    }
                    Some(tag) => {
                        let tag = tag.to_vec();
                        println!("{}", str::from_utf8(&tag).unwrap());
                        stream.required::<()>(&tag)?
                    }
                    None => break,
                }
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self { sequence })
    }
}

#[derive(Default, Debug)]
struct Sequence {
    elements: Vec<SequenceChild>,
}

#[derive(Debug, Clone)]
enum SequenceChild {
    Element(Element),
    Group(String),
}

impl ParseContent<'_> for Sequence {
    fn parse(
        stream: &mut XmlStream<'_>,
        tag: &[u8],
        start: &BytesStart<'_>,
    ) -> Result<Self, parser::Error> {
        let mut elements = Vec::new();

        while let Some(peak) = stream.peek_tag_full()? {
            match peak.name().as_ref() {
                b"xs:element" => {
                    elements.push(SequenceChild::Element(
                        stream.required::<Element>(b"xs:element")?,
                    ));
                }
                b"xs:group" => {
                    let r = peak
                        .attributes()
                        .map(|a| a.unwrap())
                        .find(|a| a.key.as_ref() == b"ref")
                        .map(|a| str::from_utf8(&a.value).unwrap().to_string())
                        .unwrap();

                    elements.push(SequenceChild::Group(r));
                    stream.required::<()>(b"xs:group")?
                }
                tag => {
                    let tag = tag.to_vec();
                    // println!("{}", str::from_utf8(&tag).unwrap());
                    stream.required::<()>(&tag)?
                }
            }
        }

        stream.skip_to_end(tag)?;

        Ok(Self { elements })
    }
}

#[derive(Debug, Clone)]
struct Element {
    name: String,
    type_: String,
    min_occurs: u32,
    max_occurs: u32,
}

impl ParseContent<'_> for Element {
    fn parse(
        stream: &mut XmlStream<'_>,
        tag: &[u8],
        start: &BytesStart<'_>,
    ) -> Result<Self, parser::Error> {
        let mut name = None;
        let mut type_ = None;
        let mut min_occurs = 1u32;
        let mut max_occurs = 1u32;

        for attr in start.attributes().map(|a| a.unwrap()) {
            if attr.key.as_ref() == b"name" {
                name = Some(str::from_utf8(&attr.value).unwrap().to_string());
            } else if attr.key.as_ref() == b"type" {
                type_ = Some(str::from_utf8(&attr.value).unwrap().to_string());
            } else if attr.key.as_ref() == b"minOccurs" {
                min_occurs = str::from_utf8(&attr.value).unwrap().parse().unwrap();
            } else if attr.key.as_ref() == b"maxOccurs" {
                max_occurs = str::from_utf8(&attr.value)
                    .unwrap()
                    .parse()
                    .unwrap_or(u32::MAX);
            }
        }

        let name = name.unwrap();
        let type_ = type_.unwrap();

        stream.skip_to_end(tag)?;

        Ok(Self {
            name,
            type_,
            min_occurs,
            max_occurs,
        })
    }
}
