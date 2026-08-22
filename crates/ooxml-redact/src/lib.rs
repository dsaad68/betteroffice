mod media;
mod xml;

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::BuildHasher;
use std::time::{SystemTime, UNIX_EPOCH};

use quick_xml::Reader;
use quick_xml::events::Event;
use thiserror::Error;

use crate::media::replace_media;
use crate::xml::{RunState, redact_xml};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Format {
    #[default]
    Auto,
    Docx,
    Xlsx,
    Pptx,
}

impl Format {
    pub fn extension(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Docx => Some("docx"),
            Self::Xlsx => Some("xlsx"),
            Self::Pptx => Some("pptx"),
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => formatter.write_str("OOXML"),
            Self::Docx => formatter.write_str("DOCX"),
            Self::Xlsx => formatter.write_str("XLSX"),
            Self::Pptx => formatter.write_str("PPTX"),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RedactionReport {
    pub format: Format,
    pub text_nodes: usize,
    pub characters: usize,
    pub attributes: usize,
    pub media_parts: usize,
    pub binary_parts: usize,
    pub xml_comments: usize,
}

#[derive(Debug, Error)]
pub enum RedactError {
    #[error("invalid OOXML package: {0}")]
    Container(String),
    #[error("could not detect DOCX, XLSX, or PPTX content")]
    UnknownFormat,
    #[error("requested {requested}, but package is {detected}")]
    FormatMismatch { requested: Format, detected: Format },
    #[error("invalid XML in {part}: {message}")]
    Xml { part: String, message: String },
    #[error("could not replace image {part}: {message}")]
    Image { part: String, message: String },
}

pub fn detect_format(bytes: &[u8]) -> Result<Format, RedactError> {
    let parts = ooxml_opc::unzip_parts(bytes).map_err(RedactError::Container)?;
    detect_parts(&parts)
}

pub fn redact(bytes: &[u8], format: Format) -> Result<Vec<u8>, RedactError> {
    redact_with_report(bytes, format).map(|(bytes, _)| bytes)
}

pub fn redact_with_report(
    bytes: &[u8],
    requested: Format,
) -> Result<(Vec<u8>, RedactionReport), RedactError> {
    let mut parts = ooxml_opc::unzip_parts(bytes).map_err(RedactError::Container)?;
    let detected = detect_parts(&parts)?;
    if requested != Format::Auto && requested != detected {
        return Err(RedactError::FormatMismatch {
            requested,
            detected,
        });
    }

    let mut report = RedactionReport {
        format: detected,
        ..RedactionReport::default()
    };
    let content_types = PackageContentTypes::parse(&parts);
    let mut mappings = IdMappings {
        salt: Some(run_salt()),
        ..IdMappings::default()
    };
    for (path, data) in &mut parts {
        let lower = path.to_ascii_lowercase();
        if media::is_replaceable_part(&lower) {
            *data = replace_media(path, data, &mut report)?;
            continue;
        }
        let content_type = content_types.resolve(&lower);
        let classified_xml =
            is_xml_part(&lower) || content_type.as_deref().is_some_and(is_xml_content_type);
        if classified_xml {
            let mut state = RunState {
                report: &mut report,
                mappings: &mut mappings,
            };
            *data = redact_xml(detected, path, content_type.as_deref(), data, &mut state)?;
        } else if is_sensitive_binary(&lower) {
            data.clear();
            report.binary_parts += 1;
        }
    }

    let output = ooxml_opc::rezip_parts(&parts).map_err(RedactError::Container)?;
    Ok((output, report))
}

fn is_xml_content_type(content_type: &str) -> bool {
    content_type.ends_with("+xml") || content_type.ends_with("/xml")
}

/// Per-run entropy so FNV-derived pseudonyms cannot be dictionary-tested
/// across documents produced by separate redaction runs.
fn run_salt() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos() as u64);
    let random = RandomState::new().hash_one(nanos);
    let address = &nanos as *const u64 as usize as u64;
    nanos ^ random ^ address
}

pub(crate) fn stable_hash(value: &str) -> u32 {
    let mut hash: u32 = 0x811C_9DC5;
    for byte in value.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

#[derive(Default)]
pub(crate) struct IdMappings {
    salt: Option<u64>,
    hex_ids: HashMap<String, String>,
    ref_names: HashMap<String, String>,
    guids: HashMap<String, String>,
    taken: HashSet<String>,
}

impl IdMappings {
    pub(crate) fn hex_id(&mut self, original: &str) -> String {
        Self::assign(&mut self.hex_ids, &mut self.taken, original, self.salt)
    }

    pub(crate) fn ref_name(&mut self, original: &str) -> String {
        format!(
            "r{}",
            Self::assign(&mut self.ref_names, &mut self.taken, original, self.salt)
        )
    }

    pub(crate) fn guid(&mut self, original: &str) -> String {
        if let Some(assigned) = self.guids.get(original) {
            return assigned.clone();
        }
        let mut candidate = format_guid(&self.guid_digest(original));
        let mut salt: u32 = 0;
        while self.taken.contains(&candidate) {
            salt += 1;
            candidate = format_guid(&self.guid_digest(&format!("{original}|{salt}")));
        }
        self.taken.insert(candidate.clone());
        self.guids.insert(original.to_owned(), candidate.clone());
        candidate
    }

    fn digest(salt: Option<u64>, input: &str) -> u32 {
        match salt {
            Some(salt) => stable_hash(&format!("{salt:016x}{input}")),
            None => stable_hash(input),
        }
    }

    fn assign(
        map: &mut HashMap<String, String>,
        taken: &mut HashSet<String>,
        original: &str,
        salt: Option<u64>,
    ) -> String {
        if let Some(assigned) = map.get(original) {
            return assigned.clone();
        }
        let mut candidate = format!("{:08X}", Self::digest(salt, original));
        let mut bump: u32 = 0;
        while taken.contains(&candidate) {
            bump += 1;
            candidate = format!("{:08X}", Self::digest(salt, &format!("{original}|{bump}")));
        }
        taken.insert(candidate.clone());
        map.insert(original.to_owned(), candidate.clone());
        candidate
    }

    fn guid_digest(&self, input: &str) -> [u8; 16] {
        let mut bytes = [0u8; 16];
        for (index, chunk) in bytes.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            chunk.copy_from_slice(
                &Self::digest(self.salt, &format!("{input}@{index}")).to_be_bytes(),
            );
        }
        bytes
    }
}

fn format_guid(bytes: &[u8; 16]) -> String {
    let mut shaped = *bytes;
    shaped[6] = (shaped[6] & 0x0F) | 0x40;
    shaped[8] = (shaped[8] & 0x3F) | 0x80;
    let hex: String = shaped.iter().map(|byte| format!("{byte:02X}")).collect();
    format!(
        "{{{}-{}-{}-{}-{}}}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// Content types declared by `[Content_Types].xml`, via `Override` (per part)
/// and `Default` (per extension). Only XML content types resolve.
#[derive(Default)]
struct PackageContentTypes {
    overrides: HashMap<String, String>,
    defaults: HashMap<String, String>,
}

impl PackageContentTypes {
    fn parse(parts: &[(String, Vec<u8>)]) -> Self {
        let Some((_, bytes)) = parts
            .iter()
            .find(|(path, _)| path.eq_ignore_ascii_case("[Content_Types].xml"))
        else {
            return Self::default();
        };
        let mut reader = Reader::from_reader(bytes.as_slice());
        let mut declarations = Self::default();
        loop {
            match reader.read_event() {
                Ok(Event::Start(start)) | Ok(Event::Empty(start))
                    if matches!(start.name().local_name().as_ref(), b"Override" | b"Default") =>
                {
                    let mut key = None;
                    let mut content_type = None;
                    for attribute in start.attributes().flatten() {
                        let name = String::from_utf8_lossy(attribute.key.local_name().as_ref())
                            .to_lowercase();
                        match name.as_str() {
                            "partname" | "extension" => {
                                key = Some(String::from_utf8_lossy(&attribute.value).into_owned());
                            }
                            "contenttype" => {
                                content_type =
                                    Some(String::from_utf8_lossy(&attribute.value).into_owned());
                            }
                            _ => {}
                        }
                    }
                    if let Some((key, content_type)) = key.zip(content_type) {
                        let lower = content_type.to_ascii_lowercase();
                        if start.name().local_name().as_ref() == b"Override" {
                            declarations
                                .overrides
                                .insert(key.trim_start_matches('/').to_ascii_lowercase(), lower);
                        } else if !key.is_empty() {
                            declarations
                                .defaults
                                .insert(key.to_ascii_lowercase(), lower);
                        }
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                Ok(_) => {}
            }
        }
        declarations
    }

    fn resolve(&self, path_lower: &str) -> Option<String> {
        let content_type = self.overrides.get(path_lower).cloned().or_else(|| {
            let extension = path_lower
                .rsplit_once('.')
                .map(|(_, extension)| extension)?;
            self.defaults.get(extension).cloned()
        })?;
        is_xml_content_type(&content_type).then_some(content_type)
    }
}

fn detect_parts(parts: &[(String, Vec<u8>)]) -> Result<Format, RedactError> {
    if let Some((_, content_types)) = parts
        .iter()
        .find(|(path, _)| path.eq_ignore_ascii_case("[Content_Types].xml"))
    {
        let text = String::from_utf8_lossy(content_types).to_ascii_lowercase();
        if text.contains("wordprocessingml.document.main+xml")
            || text.contains("ms-word.document.macroenabled.main+xml")
        {
            return Ok(Format::Docx);
        }
        if text.contains("spreadsheetml.sheet.main+xml")
            || text.contains("ms-excel.sheet.macroenabled.main+xml")
        {
            return Ok(Format::Xlsx);
        }
        if text.contains("presentationml.presentation.main+xml")
            || text.contains("ms-powerpoint.presentation.macroenabled.main+xml")
        {
            return Ok(Format::Pptx);
        }
    }

    let has = |expected: &str| {
        parts
            .iter()
            .any(|(path, _)| path.eq_ignore_ascii_case(expected))
    };
    if has("word/document.xml") {
        Ok(Format::Docx)
    } else if has("xl/workbook.xml") {
        Ok(Format::Xlsx)
    } else if has("ppt/presentation.xml") {
        Ok(Format::Pptx)
    } else {
        Err(RedactError::UnknownFormat)
    }
}

fn is_xml_part(path: &str) -> bool {
    path.ends_with(".xml") || path.ends_with(".rels") || path.ends_with(".vml")
}

fn is_sensitive_binary(path: &str) -> bool {
    path.ends_with("vbaproject.bin")
        || path.contains("/embeddings/")
        || path.contains("/activex/") && path.ends_with(".bin")
        || path.contains("/printersettings/")
}

#[cfg(test)]
mod tests;
