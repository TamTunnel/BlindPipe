use bytes::{Buf, BytesMut};
use std::str;

pub struct SseBuffer {
    buffer: BytesMut,
}

impl SseBuffer {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::new(),
        }
    }

    pub fn extend(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Yields complete SSE events (separated by \n\n).
    /// Safe against partial UTF-8 sequences as long as we split on \n\n (which are single-byte ASCII).
    pub fn next_event(&mut self) -> Option<String> {
        // Find the index of "\n\n"
        let mut split_idx = None;
        for i in 0..self.buffer.len().saturating_sub(1) {
            if self.buffer[i] == b'\n' && self.buffer[i + 1] == b'\n' {
                split_idx = Some(i + 2);
                break;
            }
        }

        if let Some(idx) = split_idx {
            let event_bytes = self.buffer.split_to(idx);
            // Convert to string lossy, handling any invalid UTF-8 gracefully
            let text = String::from_utf8_lossy(&event_bytes).into_owned();
            Some(text)
        } else {
            None
        }
    }
}
