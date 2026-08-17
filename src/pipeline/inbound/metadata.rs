pub fn strip_metadata(text: &str) -> String {
    // Basic stripping of known metadata patterns (like base64 chunks or XML blocks)
    // For now, this just passes through the text as we focus on textual generation.
    // In a fully-featured proxy, this might use regex or an XML parser to remove chunks.
    text.to_string()
}
