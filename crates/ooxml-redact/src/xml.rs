use quick_xml::events::{BytesCData, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer, XmlVersion};

use crate::{Format, IdMappings, RedactError, RedactionReport};

const DATE_PLACEHOLDER: &str = "1970-01-01T00:00:00Z";

pub(crate) struct RunState<'a> {
    pub(crate) report: &'a mut RedactionReport,
    pub(crate) mappings: &'a mut IdMappings,
}

struct ScrubContext<'a> {
    format: Format,
    path: &'a str,
    content_type: Option<&'a str>,
}

impl ScrubContext<'_> {
    fn classifies(&self, needle: &str) -> bool {
        self.path.contains(needle) || self.content_type.is_some_and(|ct| ct.contains(needle))
    }

    fn is_chart(&self) -> bool {
        self.classifies("/charts/") || self.content_type.is_some_and(|ct| ct.contains("chart+xml"))
    }

    fn is_connections(&self) -> bool {
        self.path == "xl/connections.xml"
            || self
                .content_type
                .is_some_and(|ct| ct.contains("connections+xml"))
    }

    fn is_person_part(&self) -> bool {
        self.classifies("/persons/")
            || self.classifies("/people")
            || self
                .content_type
                .is_some_and(|ct| ct.contains("person+xml") || ct.contains("people+xml"))
    }
}

pub(crate) fn redact_xml(
    format: Format,
    path: &str,
    content_type: Option<&str>,
    bytes: &[u8],
    state: &mut RunState<'_>,
) -> Result<Vec<u8>, RedactError> {
    let path_lower = path.to_ascii_lowercase();
    let content_type_lower = content_type.map(|value| value.to_ascii_lowercase());
    let context = ScrubContext {
        format,
        path: &path_lower,
        content_type: content_type_lower.as_deref(),
    };
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::with_capacity(bytes.len()));
    let mut stack = Vec::new();
    let mut cell_type: Option<String> = None;

    loop {
        let event = reader
            .read_event()
            .map_err(|error| xml_error(path, error))?;
        match event {
            Event::Start(start) => {
                let local = local_name(start.name().local_name().as_ref());
                let rewritten = rewrite_start(
                    &context,
                    path,
                    &reader,
                    start,
                    &local,
                    state,
                    &mut cell_type,
                )?;
                stack.push(local);
                writer
                    .write_event(Event::Start(rewritten))
                    .map_err(|error| xml_error(path, error))?;
            }
            Event::Empty(start) => {
                let local = local_name(start.name().local_name().as_ref());
                let rewritten = rewrite_start(
                    &context,
                    path,
                    &reader,
                    start,
                    &local,
                    state,
                    &mut cell_type,
                )?;
                writer
                    .write_event(Event::Empty(rewritten))
                    .map_err(|error| xml_error(path, error))?;
            }
            Event::End(end) => {
                if stack.last().is_some_and(|name| name == "c") {
                    cell_type = None;
                }
                stack.pop();
                writer
                    .write_event(Event::End(end))
                    .map_err(|error| xml_error(path, error))?;
            }
            Event::Text(text) => {
                if let Some(kind) = replacement_kind(&context, &stack, cell_type.as_deref()) {
                    let decoded = text.decode().map_err(|error| xml_error(path, error))?;
                    let unescaped = quick_xml::escape::unescape(&decoded)
                        .map_err(|error| xml_error(path, error))?;
                    let replacement = replace_text(&unescaped, kind, state.mappings);
                    charge_text(state.report, &unescaped);
                    writer
                        .write_event(Event::Text(BytesText::new(&replacement)))
                        .map_err(|error| xml_error(path, error))?;
                } else {
                    writer
                        .write_event(Event::Text(text))
                        .map_err(|error| xml_error(path, error))?;
                }
            }
            Event::CData(text) => {
                if let Some(kind) = replacement_kind(&context, &stack, cell_type.as_deref()) {
                    let decoded = text.decode().map_err(|error| xml_error(path, error))?;
                    let replacement = replace_text(&decoded, kind, state.mappings);
                    charge_text(state.report, &decoded);
                    writer
                        .write_event(Event::CData(BytesCData::new(&replacement)))
                        .map_err(|error| xml_error(path, error))?;
                } else {
                    writer
                        .write_event(Event::CData(text))
                        .map_err(|error| xml_error(path, error))?;
                }
            }
            Event::GeneralRef(reference) => {
                if replacement_kind(&context, &stack, cell_type.as_deref()).is_some() {
                    state.report.text_nodes += 1;
                    state.report.characters += 1;
                    writer
                        .write_event(Event::Text(BytesText::new("x")))
                        .map_err(|error| xml_error(path, error))?;
                } else {
                    writer
                        .write_event(Event::GeneralRef(reference))
                        .map_err(|error| xml_error(path, error))?;
                }
            }
            Event::Comment(_) | Event::PI(_) => {
                state.report.xml_comments += 1;
            }
            Event::DocType(_) => {
                return Err(RedactError::Xml {
                    part: path.to_owned(),
                    message: "DTD/entity declarations are forbidden".to_owned(),
                });
            }
            Event::Eof => break,
            other => writer
                .write_event(other)
                .map_err(|error| xml_error(path, error))?,
        }
    }

    Ok(writer.into_inner())
}

fn rewrite_start(
    context: &ScrubContext<'_>,
    path: &str,
    reader: &Reader<&[u8]>,
    start: BytesStart<'_>,
    element: &str,
    state: &mut RunState<'_>,
    cell_type: &mut Option<String>,
) -> Result<BytesStart<'static>, RedactError> {
    let mut attributes = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| xml_error(path, error))?;
        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(|error| xml_error(path, error))?
            .into_owned();
        attributes.push((key, value));
    }

    let external = element.eq_ignore_ascii_case("Relationship")
        && attributes.iter().any(|(key, value)| {
            attribute_local(key).eq_ignore_ascii_case("TargetMode")
                && value.eq_ignore_ascii_case("External")
        });
    if context.format == Format::Xlsx && element == "c" {
        *cell_type = None;
    }
    let mut output = start.into_owned();
    output.clear_attributes();
    for (key, value) in attributes {
        let local = attribute_local(&key);
        let replacement = if external && local.eq_ignore_ascii_case("Target") {
            Some("https://example.com".to_owned())
        } else if !key.starts_with("xmlns") {
            attribute_scrub(context, element, local, &value)
                .map(|scrub| apply_attribute_scrub(scrub, &value, state.mappings))
        } else {
            None
        };
        if let Some(replacement) = replacement {
            if replacement != value {
                state.report.attributes += 1;
            }
            output.push_attribute((key.as_str(), replacement.as_str()));
        } else {
            output.push_attribute((key.as_str(), value.as_str()));
        }
        if context.format == Format::Xlsx && element == "c" && local == "t" {
            *cell_type = Some(value);
        }
    }
    Ok(output)
}

#[derive(Clone, Copy)]
enum Replacement {
    Text,
    Number,
    Formula,
    Date,
    Boolean,
    Error,
    CompoundRefName,
    Checksum,
    LegacyAuthor,
}

#[derive(Clone, Copy)]
enum AttributeScrub {
    Mask,
    HexId,
    RefName,
    Guid,
    Fixed(&'static str),
}

fn apply_attribute_scrub(scrub: AttributeScrub, value: &str, mappings: &mut IdMappings) -> String {
    match scrub {
        AttributeScrub::Mask => placeholder(value),
        AttributeScrub::HexId => mappings.hex_id(value),
        AttributeScrub::RefName => mappings.ref_name(value),
        AttributeScrub::Guid => mappings.guid(value),
        AttributeScrub::Fixed(literal) => literal.to_owned(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MetadataSection {
    Core,
    App,
    Custom,
}

fn metadata_section(context: &ScrubContext<'_>) -> Option<MetadataSection> {
    let content_type = context.content_type;
    if context.path == "docprops/core.xml"
        || content_type.is_some_and(|ct| ct.contains("core-properties"))
    {
        Some(MetadataSection::Core)
    } else if context.path == "docprops/app.xml"
        || content_type.is_some_and(|ct| ct.contains("extended-properties"))
    {
        Some(MetadataSection::App)
    } else if context.path == "docprops/custom.xml"
        || content_type
            .is_some_and(|ct| ct.contains("custom-properties") || ct.contains("customproperties"))
    {
        Some(MetadataSection::Custom)
    } else {
        None
    }
}

fn replacement_kind(
    context: &ScrubContext<'_>,
    stack: &[String],
    cell_type: Option<&str>,
) -> Option<Replacement> {
    let element = stack.last().map(String::as_str)?;
    if let Some(section) = metadata_section(context) {
        return match section {
            MetadataSection::Core => match element {
                "created" | "modified" | "lastPrinted" => Some(Replacement::Date),
                "revision" => Some(Replacement::Number),
                "title" | "subject" | "creator" | "keywords" | "description" | "lastModifiedBy"
                | "category" | "contentStatus" | "identifier" | "language" | "version"
                | "contentType" => Some(Replacement::Text),
                _ => None,
            },
            MetadataSection::App => matches!(
                element,
                "Application"
                    | "AppVersion"
                    | "Company"
                    | "Manager"
                    | "Template"
                    | "HyperlinkBase"
                    | "lpstr"
                    | "lpwstr"
                    | "bstr"
            )
            .then_some(Replacement::Text),
            MetadataSection::Custom => match element {
                "i1" | "i2" | "i4" | "i8" | "int" | "uint" | "ui1" | "ui2" | "ui4" | "ui8"
                | "r4" | "r8" | "decimal" => Some(Replacement::Number),
                "bool" => Some(Replacement::Boolean),
                "date" | "filetime" => Some(Replacement::Date),
                _ => Some(Replacement::Text),
            },
        };
    }
    if context.path.starts_with("customxml/") && !context.path.contains("itemprops") {
        return Some(Replacement::Text);
    }
    if context.is_chart() {
        return match element {
            "f" => Some(Replacement::Formula),
            "name" if stack.iter().any(|name| name == "pivotSource") => {
                Some(Replacement::CompoundRefName)
            }
            "v" if stack.iter().any(|name| name == "strCache" || name == "tx") => {
                Some(Replacement::Text)
            }
            "v" => Some(Replacement::Number),
            "t" => Some(Replacement::Text),
            _ => None,
        };
    }
    if context.classifies("pivotcacherecords") {
        return Some(Replacement::Text);
    }
    if context.classifies("threadedcomment") {
        return match element {
            "text" => Some(Replacement::Text),
            "checksum" => Some(Replacement::Checksum),
            _ => None,
        };
    }
    if context.path == "word/settings.xml" {
        return matches!(
            element,
            "table" | "query" | "connectString" | "addressFieldName" | "mailSubject" | "udl"
        )
        .then_some(Replacement::Text);
    }

    match context.format {
        Format::Docx => matches!(element, "t" | "delText" | "instrText" | "delInstrText")
            .then_some(if matches!(element, "instrText" | "delInstrText") {
                Replacement::Formula
            } else {
                Replacement::Text
            }),
        Format::Pptx => matches!(element, "t" | "text").then_some(Replacement::Text),
        Format::Xlsx => match element {
            "author" if context.path.starts_with("xl/comments") => Some(Replacement::LegacyAuthor),
            "t" | "author" | "oddHeader" | "oddFooter" | "evenHeader" | "evenFooter"
            | "firstHeader" | "firstFooter" => Some(Replacement::Text),
            "f" | "formula1" | "formula2" | "definedName" | "formula" => Some(Replacement::Formula),
            "v" if cell_type == Some("s") => None,
            "v" if matches!(cell_type, Some("str" | "inlineStr")) => Some(Replacement::Text),
            "v" if cell_type == Some("e") => Some(Replacement::Error),
            "v" => Some(Replacement::Number),
            _ => None,
        },
        Format::Auto => None,
    }
}

fn replace_text(text: &str, kind: Replacement, mappings: &mut IdMappings) -> String {
    if text.trim().is_empty() {
        return text.to_owned();
    }
    match kind {
        Replacement::Text => placeholder(text),
        Replacement::Number => numeric_placeholder(text),
        Replacement::Formula => "0".to_owned(),
        Replacement::Date => DATE_PLACEHOLDER.to_owned(),
        Replacement::Boolean => "false".to_owned(),
        Replacement::Error => "#N/A".to_owned(),
        Replacement::Checksum => "0".to_owned(),
        Replacement::CompoundRefName => compound_ref_name(text, mappings),
        Replacement::LegacyAuthor => match text.strip_prefix("tc=") {
            Some(guid) => format!("tc={}", mappings.guid(guid)),
            None => placeholder(text),
        },
    }
}

fn compound_ref_name(text: &str, mappings: &mut IdMappings) -> String {
    let mut rest = text;
    let mut rebuilt = String::new();
    if let Some(inner) = rest.strip_prefix('[')
        && let Some((book, tail)) = inner.split_once(']')
    {
        rebuilt.push('[');
        rebuilt.push_str(book);
        rebuilt.push(']');
        rest = tail;
    }
    match rest.rsplit_once('!') {
        Some((sheet, table)) => {
            rebuilt.push_str(&placeholder(sheet));
            rebuilt.push('!');
            rebuilt.push_str(&mappings.ref_name(table));
        }
        None => rebuilt.push_str(&mappings.ref_name(rest)),
    }
    rebuilt
}

fn attribute_scrub(
    context: &ScrubContext<'_>,
    element: &str,
    attribute: &str,
    value: &str,
) -> Option<AttributeScrub> {
    let format = context.format;
    if metadata_section(context) == Some(MetadataSection::Custom)
        && matches!(attribute, "name" | "linkTarget")
    {
        return Some(AttributeScrub::Mask);
    }
    if context.path.starts_with("customxml/")
        && !context.path.contains("itemprops")
        && !matches!(attribute, "id" | "Id")
    {
        return Some(AttributeScrub::Mask);
    }
    if matches!(element, "docPr" | "cNvPr") && matches!(attribute, "name" | "descr" | "title") {
        return Some(AttributeScrub::Mask);
    }
    if element == "textpath" && attribute == "string" {
        return Some(AttributeScrub::Mask);
    }
    if context.classifies("pivotcache") && attribute == "refreshedBy" {
        return Some(AttributeScrub::Mask);
    }
    if context.classifies("pivotcache") && attribute == "refreshedDateIso" {
        return Some(AttributeScrub::Fixed(DATE_PLACEHOLDER));
    }
    if context.classifies("pivotcache") && attribute == "refreshedDate" {
        return Some(AttributeScrub::Fixed("45000"));
    }
    if context.classifies("pivotcache")
        && element == "rangeSet"
        && matches!(attribute, "sheet" | "name")
    {
        return Some(AttributeScrub::Mask);
    }
    if context.classifies("pivotcache")
        && element == "worksheetSource"
        && matches!(attribute, "name" | "sheet")
    {
        return Some(AttributeScrub::Mask);
    }
    if context.classifies("pivotcache")
        && element == "cacheHierarchy"
        && matches!(
            attribute,
            "caption"
                | "uniqueName"
                | "dimensionUniqueName"
                | "allCaption"
                | "allUniqueName"
                | "defaultMemberUniqueName"
                | "displayFolder"
        )
    {
        return Some(AttributeScrub::Mask);
    }
    if (context.classifies("pivotcache") || context.classifies("pivottable"))
        && element == "pageField"
        && matches!(attribute, "name" | "cap")
    {
        return Some(AttributeScrub::Mask);
    }
    if (context.classifies("pivotcache") || context.classifies("pivottable"))
        && element == "pageItem"
        && attribute == "name"
    {
        return Some(AttributeScrub::RefName);
    }
    if context.classifies("pivotcache") && element == "cacheField" {
        return match attribute {
            "name" | "caption" => Some(AttributeScrub::RefName),
            "propertyName" | "formula" => Some(AttributeScrub::Mask),
            _ => None,
        };
    }
    if context.classifies("pivotcache") && attribute == "v" {
        return match element {
            "x" => None,
            "n" | "b" => Some(AttributeScrub::Fixed("0")),
            "d" => Some(AttributeScrub::Fixed(DATE_PLACEHOLDER)),
            "e" => Some(AttributeScrub::Fixed("#N/A")),
            _ => Some(AttributeScrub::Mask),
        };
    }
    if context.classifies("pivottable")
        && matches!(element, "item" | "pivotItem")
        && attribute == "n"
    {
        return Some(AttributeScrub::RefName);
    }
    if context.classifies("pivottable") && element.eq_ignore_ascii_case("pivotTableDefinition") {
        if attribute == "name" {
            return Some(AttributeScrub::RefName);
        }
        if attribute.to_ascii_lowercase().contains("caption") {
            return Some(AttributeScrub::Mask);
        }
    }
    if context.classifies("pivottable")
        && element.eq_ignore_ascii_case("pivotField")
        && matches!(attribute, "name" | "subtotalCaption")
    {
        return Some(AttributeScrub::Mask);
    }
    if context.classifies("pivottable")
        && element.eq_ignore_ascii_case("dataField")
        && attribute == "name"
    {
        return Some(AttributeScrub::Mask);
    }
    if context.classifies("slicer")
        && matches!(attribute, "name" | "cache" | "sourceName" | "caption")
    {
        return Some(AttributeScrub::RefName);
    }
    if context.path.starts_with("word/comments")
        && (attribute.eq_ignore_ascii_case("dateUtc")
            || (element.eq_ignore_ascii_case("comment") && attribute.eq_ignore_ascii_case("date")))
    {
        return Some(AttributeScrub::Fixed(DATE_PLACEHOLDER));
    }
    if context.path.starts_with("xl/comments")
        && element.eq_ignore_ascii_case("comment")
        && attribute.eq_ignore_ascii_case("uid")
    {
        return Some(AttributeScrub::Guid);
    }
    if context.classifies("threadedcomment")
        && (attribute.eq_ignore_ascii_case("dt") || matches!(attribute, "date" | "dateUtc"))
    {
        return Some(AttributeScrub::Fixed(DATE_PLACEHOLDER));
    }
    if context.classifies("threadedcomment") && element == "hyperlink" && attribute == "url" {
        return Some(AttributeScrub::Mask);
    }
    if context.classifies("threadedcomment") || context.is_person_part() {
        if matches!(
            attribute,
            "userId" | "displayName" | "providerId" | "initials"
        ) {
            return Some(AttributeScrub::Mask);
        }
        let identity_attribute = attribute.eq_ignore_ascii_case("personId")
            || (element.eq_ignore_ascii_case("person") && attribute.eq_ignore_ascii_case("id"))
            || (element.eq_ignore_ascii_case("threadedComment")
                && matches!(attribute, "id" | "parentId"))
            || (element.eq_ignore_ascii_case("mention")
                && matches!(attribute, "mentionpersonId" | "mentionId"));
        if identity_attribute {
            return Some(AttributeScrub::Guid);
        }
    }
    if context.is_connections() {
        if element == "connection" && attribute == "name" {
            return Some(AttributeScrub::RefName);
        }
        if element == "parameter" {
            return match attribute {
                "name" => Some(AttributeScrub::RefName),
                "string" | "prompt" | "cell" => Some(AttributeScrub::Mask),
                "double" | "integer" | "boolean" => Some(AttributeScrub::Fixed("0")),
                _ => None,
            };
        }
        if matches!(
            (element, attribute),
            ("dbPr", "connection" | "command" | "serverCommand")
                | ("olapPr", "localConnection")
                | ("webPr", "url" | "post" | "editPage")
                | (
                    "connection",
                    "odcFile" | "sourceFile" | "singleSignOnId" | "description"
                )
                | ("textPr", "sourceFile")
                | ("s", "v")
        ) {
            return Some(AttributeScrub::Mask);
        }
    }
    if context.path == "word/settings.xml"
        && matches!(
            element,
            "table" | "query" | "connectString" | "addressFieldName" | "mailSubject" | "udl"
        )
        && attribute == "val"
    {
        return Some(AttributeScrub::Mask);
    }
    if context.path == "word/styles.xml" && element == "name" && attribute == "val" {
        return Some(AttributeScrub::Mask);
    }
    match format {
        Format::Docx => docx_attribute_scrub(element, attribute),
        Format::Xlsx => xlsx_attribute_scrub(element, attribute, value),
        Format::Pptx => pptx_attribute_scrub(element, attribute),
        Format::Auto => None,
    }
}

fn docx_attribute_scrub(element: &str, attribute: &str) -> Option<AttributeScrub> {
    let lower_element = element.to_ascii_lowercase();
    if matches!(attribute, "author" | "initials") {
        return Some(AttributeScrub::Mask);
    }
    if attribute.to_ascii_lowercase().starts_with("rsid") {
        return Some(AttributeScrub::HexId);
    }
    if (lower_element == "rsid" || lower_element == "rsidroot")
        && attribute.eq_ignore_ascii_case("val")
    {
        return Some(AttributeScrub::HexId);
    }
    if (matches!(element, "ins" | "del")
        || lower_element.ends_with("change")
        || lower_element.starts_with("move")
        || matches!(
            lower_element.as_str(),
            "cellins"
                | "celldel"
                | "cellmerge"
                | "customxmlins"
                | "customxmldel"
                | "customxmlmove"
                | "customxmlinsrangestart"
                | "customxmldelrangestart"
                | "customxmlmoverangestart"
                | "customxmlinsrangeend"
                | "customxmldelrangeend"
                | "customxmlmoverangeend"
        ))
        && matches!(attribute, "date" | "dateUtc")
    {
        return Some(AttributeScrub::Fixed(DATE_PLACEHOLDER));
    }
    if matches!(element, "documentProtection" | "writeProtection")
        && matches!(attribute, "hash" | "salt" | "hashValue" | "saltValue")
    {
        return Some(AttributeScrub::Mask);
    }
    if element == "fldSimple" && attribute == "instr" {
        return Some(AttributeScrub::Mask);
    }
    if element == "hyperlink" && matches!(attribute, "tooltip" | "tgtFrame") {
        return Some(AttributeScrub::Mask);
    }
    if matches!(element, "alias" | "tag" | "docVar") && matches!(attribute, "name" | "val") {
        return Some(AttributeScrub::Mask);
    }
    None
}

fn xlsx_attribute_scrub(element: &str, attribute: &str, value: &str) -> Option<AttributeScrub> {
    if element == "sheet" && attribute == "name" {
        return Some(AttributeScrub::Mask);
    }
    if element == "definedName" && attribute == "name" && !value.starts_with("_xlnm.") {
        return Some(AttributeScrub::Mask);
    }
    if matches!(element, "table" | "tableColumn") && matches!(attribute, "name" | "displayName") {
        return Some(AttributeScrub::Mask);
    }
    if element == "dataValidation"
        && matches!(attribute, "prompt" | "promptTitle" | "error" | "errorTitle")
    {
        return Some(AttributeScrub::Mask);
    }
    if element == "hyperlink" && matches!(attribute, "display" | "tooltip" | "location") {
        return Some(AttributeScrub::Mask);
    }
    if element == "filter" && attribute == "val" {
        return Some(AttributeScrub::Mask);
    }
    None
}

fn pptx_attribute_scrub(element: &str, attribute: &str) -> Option<AttributeScrub> {
    if (element == "cSld" && attribute == "name")
        || (element == "cmAuthor" && matches!(attribute, "name" | "initials"))
        || (element == "tag" && matches!(attribute, "name" | "val"))
        || (element == "custShow" && attribute == "name")
        || (element == "section" && attribute == "name")
    {
        return Some(AttributeScrub::Mask);
    }
    None
}

fn placeholder(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_whitespace() {
                character
            } else {
                'x'
            }
        })
        .collect()
}

fn numeric_placeholder(text: &str) -> String {
    let mut valid = true;
    let replacement: String = text
        .chars()
        .map(|character| match character {
            '0'..='9' => '8',
            '-' | '+' | '.' | 'e' | 'E' | ' ' | '\t' | '\r' | '\n' => character,
            _ => {
                valid = false;
                'x'
            }
        })
        .collect();
    if valid {
        replacement
    } else {
        placeholder(text)
    }
}

fn charge_text(report: &mut RedactionReport, text: &str) {
    if !text.trim().is_empty() {
        report.text_nodes += 1;
        report.characters += text
            .chars()
            .filter(|character| !character.is_whitespace())
            .count();
    }
}

fn local_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

fn attribute_local(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, local)| local)
}

fn xml_error(path: &str, error: impl fmt::Display) -> RedactError {
    RedactError::Xml {
        part: path.to_owned(),
        message: error.to_string(),
    }
}

use std::fmt;
