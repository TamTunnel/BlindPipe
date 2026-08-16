use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref PATTERNS: Vec<(&'static str, Regex)> = vec![
        ("API_KEY_OPENAI", Regex::new(r"sk-[a-zA-Z0-9]{48}").unwrap()),
        ("API_KEY_AWS", Regex::new(r"(A3T[A-Z0-9]|AKIA|AGPA|AIDA|AROA|AIPA|ANPA|ANVA|ASIA)[A-Z0-9]{16}").unwrap()),
        ("API_KEY_GITHUB", Regex::new(r"(ghp|gho|ghu|ghs|ghr)_[a-zA-Z0-9]{36}").unwrap()),
        ("CREDIT_CARD", Regex::new(r"\b(?:\d[ -]*?){13,16}\b").unwrap()),
        ("SSN", Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap()),
        ("IPV4_ADDRESS", Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap()),
        ("UUID", Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b").unwrap()),
    ];
}

#[derive(Debug)]
pub struct RegexEntity {
    pub label: String,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

pub struct RegexEngine;

impl RegexEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn extract(&self, text: &str) -> Vec<RegexEntity> {
        let mut entities = Vec::new();

        for (label, pattern) in PATTERNS.iter() {
            for mat in pattern.find_iter(text) {
                let match_str = mat.as_str();

                if *label == "CREDIT_CARD" && !is_luhn_valid(match_str) {
                    continue;
                }

                if *label == "SSN" && !is_ssn_valid(match_str) {
                    continue;
                }

                entities.push(RegexEntity {
                    label: label.to_string(),
                    text: match_str.to_string(),
                    start: mat.start(),
                    end: mat.end(),
                });
            }
        }

        entities
    }
}

pub fn is_ssn_valid(ssn: &str) -> bool {
    let parts: Vec<&str> = ssn.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    let area = parts[0];
    let group = parts[1];
    let serial = parts[2];

    if area == "000" || area == "666" || area.starts_with('9') {
        return false;
    }
    if group == "00" {
        return false;
    }
    if serial == "0000" {
        return false;
    }
    true
}

pub fn is_luhn_valid(cc: &str) -> bool {
    let digits: Vec<u32> = cc.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.is_empty() {
        return false;
    }

    let mut sum = 0;
    for (i, d) in digits.iter().rev().enumerate() {
        let mut val = *d;
        if i % 2 == 1 {
            val *= 2;
            if val > 9 {
                val -= 9;
            }
        }
        sum += val;
    }
    sum % 10 == 0
}
