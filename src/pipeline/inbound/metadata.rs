use std::io::{Cursor, Read, Write};
use zip::{ZipArchive, ZipWriter};
use zip::write::SimpleFileOptions;
use lopdf::{Document, Object};
use std::borrow::Cow;
use base64::{Engine as _, engine::general_purpose::STANDARD as b64};

lazy_static::lazy_static! {
    static ref SVG_META_RE: regex::bytes::Regex = regex::bytes::Regex::new(r"(?is)<metadata.*?</metadata>").unwrap();
    static ref SVG_RDF_RE: regex::bytes::Regex = regex::bytes::Regex::new(r"(?is)<rdf:RDF.*?</rdf:RDF>").unwrap();
    static ref XML_COMMENT_RE: regex::bytes::Regex = regex::bytes::Regex::new(r"(?is)<!--.*?-->").unwrap();
    static ref HTML_META_GEN_RE: regex::bytes::Regex = regex::bytes::Regex::new(r"(?is)<meta\s+name=['\x22]?(?:generator|author)['\x22]?[^>]*>").unwrap();
    static ref HTML_DATA_AI_RE: regex::bytes::Regex = regex::bytes::Regex::new(r"(?is)\s+data-ai-[a-zA-Z0-9\-]+=(?:'[^']*'|\x22[^\x22]*\x22|[^\s>]+)").unwrap();
    static ref MD_FRONTMATTER_RE: regex::bytes::Regex = regex::bytes::Regex::new(r"(?s)\A(?:---[\r\n]+.*?[\r\n]+---|^\+\+\+[\r\n]+.*?[\r\n]+\+\+\+)[\r\n]*").unwrap();
}

pub struct MetadataStripper;

impl MetadataStripper {
    pub fn strip_binary(bytes: &[u8], content_type: &str) -> Vec<u8> {
        let magic = if bytes.len() > 8 { &bytes[0..8] } else { bytes };
        
        if content_type.contains("image/png") || magic.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Self::strip_png(bytes).unwrap_or_else(|| bytes.to_vec());
        } else if content_type.contains("image/jpeg") || magic.starts_with(b"\xFF\xD8\xFF") {
            return Self::strip_jpeg(bytes).unwrap_or_else(|| bytes.to_vec());
        } else if content_type.contains("image/webp") || (magic.starts_with(b"RIFF") && bytes.len() > 11 && &bytes[8..12] == b"WEBP") {
            return Self::strip_webp(bytes).unwrap_or_else(|| bytes.to_vec());
        } else if content_type.contains("image/gif") || magic.starts_with(b"GIF8") {
            return Self::strip_gif(bytes).unwrap_or_else(|| bytes.to_vec());
        } else if content_type.contains("image/tiff") || magic.starts_with(b"II*\x00") || magic.starts_with(b"MM\x00*") {
            return Self::strip_tiff(bytes).unwrap_or_else(|| bytes.to_vec());
        } else if content_type.contains("image/bmp") || magic.starts_with(b"BM") {
            return Self::strip_bmp(bytes).unwrap_or_else(|| bytes.to_vec());
        } else if content_type.contains("image/svg") || content_type.contains("xml") || magic.starts_with(b"<?xml") || magic.starts_with(b"<svg") {
            return Self::strip_svg(bytes);
        } else if content_type.contains("application/pdf") || magic.starts_with(b"%PDF") {
            return Self::strip_pdf(bytes).unwrap_or_else(|| bytes.to_vec());
        } else if content_type.contains("application/zip") || content_type.contains("application/epub") || content_type.contains("application/vnd.openxmlformats") || magic.starts_with(b"PK\x03\x04") {
            return Self::strip_zip(bytes).unwrap_or_else(|| bytes.to_vec());
        } else if content_type.contains("text/html") || magic.starts_with(b"<!DOC") || magic.starts_with(b"<html") {
            return Self::strip_html(bytes);
        } else if content_type.contains("text/markdown") || content_type.contains("text/plain") {
            return Self::strip_markdown(bytes);
        }
        
        bytes.to_vec()
    }

    pub fn strip_metadata(text: &str) -> String {
        // Fast Base64 Filter guard
        if text.len() > 100 && !text.contains(' ') {
            // Check prefixes
            let is_b64_file = text.starts_with("iVBOR") || // PNG
                              text.starts_with("/9j/") || // JPEG
                              text.starts_with("JVBER") || // PDF
                              text.starts_with("UEsDB") || // ZIP
                              text.starts_with("UklGR") || // WEBP
                              text.starts_with("R0lGOD") || // GIF
                              text.starts_with("PHN2Zy");  // SVG
            if is_b64_file {
                if let Ok(decoded) = b64.decode(text) {
                    let stripped = Self::strip_binary(&decoded, "");
                    return b64.encode(stripped);
                }
            }
        }
        text.to_string()
    }

    // ---------------------------------------------
    // PNG
    // ---------------------------------------------
    fn strip_png(bytes: &[u8]) -> Option<Vec<u8>> {
        if bytes.len() < 8 || &bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
            return None;
        }
        let mut out = Vec::with_capacity(bytes.len());
        out.extend_from_slice(&bytes[0..8]);
        
        let mut pos = 8;
        while pos + 8 <= bytes.len() {
            let len = u32::from_be_bytes([bytes[pos], bytes[pos+1], bytes[pos+2], bytes[pos+3]]) as usize;
            let chunk_type = &bytes[pos+4..pos+8];
            let chunk_total = 12 + len;
            if pos + chunk_total > bytes.len() {
                break;
            }
            // Drop metadata chunks
            match chunk_type {
                b"caTX" | b"iTXt" | b"tEXt" | b"zTXt" | b"eXIf" => {
                    // skip
                },
                _ => {
                    out.extend_from_slice(&bytes[pos..pos+chunk_total]);
                }
            }
            if chunk_type == b"IEND" {
                break;
            }
            pos += chunk_total;
        }
        Some(out)
    }

    // ---------------------------------------------
    // JPEG
    // ---------------------------------------------
    fn strip_jpeg(bytes: &[u8]) -> Option<Vec<u8>> {
        if bytes.len() < 2 || &bytes[0..2] != b"\xFF\xD8" {
            return None;
        }
        let mut out = Vec::with_capacity(bytes.len());
        out.extend_from_slice(b"\xFF\xD8");
        
        let mut pos = 2;
        while pos + 4 <= bytes.len() {
            if bytes[pos] != 0xFF {
                out.extend_from_slice(&bytes[pos..]);
                break;
            }
            let marker = bytes[pos+1];
            
            if marker == 0x01 || (marker >= 0xD0 && marker <= 0xD9) {
                out.extend_from_slice(&bytes[pos..pos+2]);
                pos += 2;
                if marker == 0xD9 { break; }
                continue;
            }
            
            let len = u16::from_be_bytes([bytes[pos+2], bytes[pos+3]]) as usize;
            let marker_total = 2 + len;
            
            if pos + marker_total > bytes.len() {
                out.extend_from_slice(&bytes[pos..]);
                break;
            }
            
            match marker {
                0xE1 | 0xEB => {}, // Drop APP1 and APP11
                0xDA => {
                    out.extend_from_slice(&bytes[pos..]);
                    break;
                }
                _ => {
                    out.extend_from_slice(&bytes[pos..pos+marker_total]);
                }
            }
            
            pos += marker_total;
        }
        Some(out)
    }

    // ---------------------------------------------
    // WebP
    // ---------------------------------------------
    fn strip_webp(bytes: &[u8]) -> Option<Vec<u8>> {
        if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
            return None;
        }
        
        let mut chunks_data = Vec::new();
        let mut pos = 12;
        
        while pos + 8 <= bytes.len() {
            let chunk_id = &bytes[pos..pos+4];
            let len = u32::from_le_bytes([bytes[pos+4], bytes[pos+5], bytes[pos+6], bytes[pos+7]]) as usize;
            let padded_len = if len % 2 == 1 { len + 1 } else { len };
            let chunk_total = 8 + padded_len;
            if pos + chunk_total > bytes.len() { break; }
            
            if chunk_id != b"EXIF" && chunk_id != b"XMP " {
                chunks_data.extend_from_slice(&bytes[pos..pos+chunk_total]);
            }
            
            pos += chunk_total;
        }
        
        let total_size = (4 + chunks_data.len()) as u32;
        let mut out = Vec::with_capacity(8 + total_size as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&total_size.to_le_bytes());
        out.extend_from_slice(b"WEBP");
        out.extend_from_slice(&chunks_data);
        Some(out)
    }

    // ---------------------------------------------
    // GIF
    // ---------------------------------------------
    fn strip_gif(bytes: &[u8]) -> Option<Vec<u8>> {
        if bytes.len() < 6 || !bytes.starts_with(b"GIF8") { return None; }
        
        let mut out = Vec::with_capacity(bytes.len());
        let mut pos = 0;
        
        while pos < bytes.len() {
            if pos + 2 <= bytes.len() && bytes[pos] == 0x21 {
                let ext_type = bytes[pos+1];
                if ext_type == 0xFE || ext_type == 0xFF {
                    let start = pos;
                    pos += 2;
                    while pos < bytes.len() && bytes[pos] != 0 {
                        let block_len = bytes[pos] as usize;
                        pos += 1 + block_len;
                    }
                    if pos < bytes.len() { pos += 1; }
                    
                    if ext_type == 0xFF {
                        let ext_data = &bytes[start..pos];
                        if !ext_data.windows(8).any(|w| w == b"XMP Data") {
                            out.extend_from_slice(ext_data);
                        }
                    }
                    continue;
                }
            }
            out.push(bytes[pos]);
            pos += 1;
        }
        Some(out)
    }

    // ---------------------------------------------
    // BMP & TIFF
    // ---------------------------------------------
    fn strip_tiff(bytes: &[u8]) -> Option<Vec<u8>> { Some(bytes.to_vec()) }
    fn strip_bmp(bytes: &[u8]) -> Option<Vec<u8>> { Some(bytes.to_vec()) }

    // ---------------------------------------------
    // SVG
    // ---------------------------------------------
    fn strip_svg(bytes: &[u8]) -> Vec<u8> {
        let mut result = Cow::Borrowed(bytes);
        result = Cow::Owned(SVG_META_RE.replace_all(&result, &b""[..]).into_owned());
        result = Cow::Owned(SVG_RDF_RE.replace_all(&result, &b""[..]).into_owned());
        result = Cow::Owned(XML_COMMENT_RE.replace_all(&result, &b""[..]).into_owned());
        result.into_owned()
    }

    // ---------------------------------------------
    // ZIP / DOCX / EPUB / ODT
    // ---------------------------------------------
    fn strip_zip(bytes: &[u8]) -> Option<Vec<u8>> {
        let reader = Cursor::new(bytes);
        let mut archive = ZipArchive::new(reader).ok()?;
        let mut out_buf = Vec::new();
        
        {
            let mut writer = ZipWriter::new(Cursor::new(&mut out_buf));
            let options = SimpleFileOptions::default();
            
            for i in 0..archive.len() {
                if let Ok(mut file) = archive.by_index(i) {
                    let name = file.name().to_string();
                    let is_metadata = name.contains("docProps/core.xml") ||
                                      name.contains("docProps/app.xml") ||
                                      name.contains("docProps/custom.xml") ||
                                      name.contains("meta.xml") ||
                                      name.contains("c2pa/");
                    
                    if !is_metadata {
                        if writer.start_file(&name, options).is_ok() {
                            let mut buf = Vec::new();
                            if file.read_to_end(&mut buf).is_ok() {
                                let _ = writer.write_all(&buf);
                            }
                        }
                    }
                }
            }
            writer.finish().ok()?;
        }
        Some(out_buf)
    }

    // ---------------------------------------------
    // PDF
    // ---------------------------------------------
    fn strip_pdf(bytes: &[u8]) -> Option<Vec<u8>> {
        let mut doc = match Document::load_mem(bytes) {
            Ok(d) => d,
            Err(_) => return None,
        };
        
        if let Some(root_id) = doc.catalog() {
            if let Ok(catalog) = doc.get_dictionary_mut(root_id) {
                catalog.remove(b"Metadata");
                catalog.remove(b"PieceInfo");
            }
        }
        
        if let Some(info_id) = doc.trailer.get(b"Info").and_then(|obj| obj.as_reference().ok()) {
            if let Ok(info) = doc.get_dictionary_mut(info_id) {
                let keys = [b"Author", b"Creator", b"Producer", b"CreationDate", b"ModDate"];
                for &key in &keys {
                    if info.has(key) {
                        info.set(key.to_vec(), Object::String(b"".to_vec(), lopdf::StringFormat::Literal));
                    }
                }
            }
        }
        
        let mut out = Cursor::new(Vec::new());
        if doc.save_to(&mut out).is_ok() {
            Some(out.into_inner())
        } else {
            None
        }
    }

    // ---------------------------------------------
    // HTML
    // ---------------------------------------------
    fn strip_html(bytes: &[u8]) -> Vec<u8> {
        let mut result = Cow::Borrowed(bytes);
        result = Cow::Owned(HTML_META_GEN_RE.replace_all(&result, &b""[..]).into_owned());
        result = Cow::Owned(HTML_DATA_AI_RE.replace_all(&result, &b""[..]).into_owned());
        result = Cow::Owned(XML_COMMENT_RE.replace_all(&result, &b""[..]).into_owned());
        result.into_owned()
    }

    // ---------------------------------------------
    // Markdown
    // ---------------------------------------------
    fn strip_markdown(bytes: &[u8]) -> Vec<u8> {
        let mut result = Cow::Borrowed(bytes);
        result = Cow::Owned(MD_FRONTMATTER_RE.replace_all(&result, &b""[..]).into_owned());
        result = Cow::Owned(XML_COMMENT_RE.replace_all(&result, &b""[..]).into_owned());
        result.into_owned()
    }
}
