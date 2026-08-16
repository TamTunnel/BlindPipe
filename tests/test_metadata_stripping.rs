use blindpipe::pipeline::inbound::metadata::MetadataStripper;
use blindpipe::utils::json_walker::StringProcessor;
use serde_json::json;
use std::io::Write;
use zip::write::SimpleFileOptions;
use lopdf::{Document, Dictionary, Object, StringFormat};

#[test]
fn test_png_stripping() {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    
    // IHDR chunk (fake)
    png.extend_from_slice(&13u32.to_be_bytes()); // length
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&[0; 13]); // data
    png.extend_from_slice(&[0; 4]); // crc
    
    // tEXt chunk (metadata to strip)
    let text_data = b"SomeMetadata";
    png.extend_from_slice(&(text_data.len() as u32).to_be_bytes());
    png.extend_from_slice(b"tEXt");
    png.extend_from_slice(text_data);
    png.extend_from_slice(&[0; 4]); // crc
    
    // IEND chunk
    png.extend_from_slice(&0u32.to_be_bytes());
    png.extend_from_slice(b"IEND");
    png.extend_from_slice(&[0; 4]); // crc
    
    let stripped = MetadataStripper::strip_binary(&png, "image/png");
    
    // Check it still has IHDR and IEND but no tEXt
    assert!(stripped.windows(4).any(|w| w == b"IHDR"));
    assert!(stripped.windows(4).any(|w| w == b"IEND"));
    assert!(!stripped.windows(4).any(|w| w == b"tEXt"));
}

#[test]
fn test_jpeg_stripping() {
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(b"\xFF\xD8"); // SOI
    
    // APP1 (metadata to strip)
    jpeg.extend_from_slice(b"\xFF\xE1");
    jpeg.extend_from_slice(&4u16.to_be_bytes());
    jpeg.extend_from_slice(b"ab");
    
    // SOF0 (keep)
    jpeg.extend_from_slice(b"\xFF\xC0");
    jpeg.extend_from_slice(&4u16.to_be_bytes());
    jpeg.extend_from_slice(b"cd");
    
    // SOS (keep and everything after)
    jpeg.extend_from_slice(b"\xFF\xDA");
    jpeg.extend_from_slice(b"randomscandata\xFF\xD9");
    
    let stripped = MetadataStripper::strip_binary(&jpeg, "image/jpeg");
    
    assert!(stripped.windows(2).any(|w| w == b"\xFF\xC0"));
    assert!(stripped.windows(2).any(|w| w == b"\xFF\xDA"));
    assert!(!stripped.windows(2).any(|w| w == b"\xFF\xE1"));
}

#[test]
fn test_docx_stripping() {
    let mut zip_buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
        let options = SimpleFileOptions::default();
        zip.start_file("docProps/core.xml", options).unwrap();
        zip.write_all(b"<core>metadata</core>").unwrap();
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(b"<document>text</document>").unwrap();
        zip.finish().unwrap();
    }
    
    let stripped = MetadataStripper::strip_binary(&zip_buf, "application/vnd.openxmlformats");
    
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&stripped)).unwrap();
    let file_names: Vec<String> = (0..archive.len()).map(|i| archive.by_index(i).unwrap().name().to_string()).collect();
    
    assert!(!file_names.contains(&"docProps/core.xml".to_string()));
    assert!(file_names.contains(&"word/document.xml".to_string()));
}

#[test]
fn test_pdf_stripping() {
    let mut doc = Document::with_version("1.4");
    
    let mut info = Dictionary::new();
    info.set(b"Author".to_vec(), Object::String(b"SecretAuthor".to_vec(), StringFormat::Literal));
    let info_id = doc.add_object(info);
    doc.trailer.set(b"Info".to_vec(), Object::Reference(info_id));
    
    let mut root = Dictionary::new();
    root.set(b"Type".to_vec(), Object::Name(b"Catalog".to_vec()));
    root.set(b"Metadata".to_vec(), Object::String(b"fake_metadata_stream".to_vec(), StringFormat::Literal));
    let root_id = doc.add_object(root);
    
    doc.trailer.set(b"Root".to_vec(), Object::Reference(root_id));
    
    let mut buf = Vec::new();
    doc.save_to(&mut buf).unwrap();
    
    let stripped = MetadataStripper::strip_binary(&buf, "application/pdf");
    
    let clean_doc = Document::load_mem(&stripped).unwrap();
    if let Ok(info_dict) = clean_doc.trailer.get(b"Info").and_then(|obj| obj.as_reference()).and_then(|id| clean_doc.get_dictionary(id)) {
        if let Ok(Object::String(author, _)) = info_dict.get(b"Author") {
            assert_eq!(author, b"");
        }
    }
    
    let clean_root = clean_doc.get_dictionary(clean_doc.catalog().unwrap()).unwrap();
    assert!(!clean_root.has(b"Metadata"));
}

#[tokio::test]
async fn test_base64_json_walker() {
    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    // IHDR chunk (fake)
    png.extend_from_slice(&13u32.to_be_bytes()); // length
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&[0; 13]); // data
    png.extend_from_slice(&[0; 4]); // crc
    // tEXt chunk
    let text_data = b"SomeMetadata";
    png.extend_from_slice(&(text_data.len() as u32).to_be_bytes());
    png.extend_from_slice(b"tEXt");
    png.extend_from_slice(text_data);
    png.extend_from_slice(&[0; 4]); // crc
    // padding to >100 bytes so the guard activates
    png.extend_from_slice(&[0; 100]); 
    
    use base64::{Engine as _, engine::general_purpose::STANDARD as b64};
    let b64_str = b64.encode(&png);
    
    let mut payload = json!({
        "image": b64_str
    });
    
    struct TestProcessor;
    impl StringProcessor for TestProcessor {
        fn process<'a>(&'a self, s: &'a str) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>> {
            Box::pin(async move {
                MetadataStripper::strip_metadata(s)
            })
        }
    }
    
    let processor = TestProcessor;
    blindpipe::utils::json_walker::walk_json(&mut payload, &processor).await;
    
    let new_b64 = payload["image"].as_str().unwrap();
    let decoded = b64.decode(new_b64).unwrap();
    
    assert!(decoded.windows(4).any(|w| w == b"IHDR"));
    assert!(!decoded.windows(4).any(|w| w == b"tEXt"));
}
