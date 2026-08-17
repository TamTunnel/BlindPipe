pub fn disrupt_watermark(text: &str) -> String {
    // Deterministic transitional synonym swapping and punctuation normalization
    // A simple, lightweight implementation
    let mut result = text.to_string();
    result = result.replace(" moreover,", " furthermore,");
    result = result.replace(" however,", " nevertheless,");
    result = result.replace(" in conclusion,", " to conclude,");
    // Simple punctuation normalization
    result = result.replace("  ", " ");
    result
}
