use unicode_normalization::UnicodeNormalization;

pub fn clean_unicode(text: &str) -> String {
    let stripped: String = text
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{200B}'..='\u{200D}' // Zero-width spaces
                | '\u{FEFF}' // BOM
                | '\u{202A}'..='\u{202E}' // Bidi override
                | '\u{2066}'..='\u{2069}' // Bidi isolate
                | '\u{E0000}'..='\u{E007F}' // Tags
                | '\u{00AD}' // Soft hyphen
                | '\u{2060}' // Word joiner / zero-width no-break space
            )
        })
        .collect();

    stripped.nfkc().collect()
}
