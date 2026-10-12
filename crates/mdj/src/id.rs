use base64::Engine;

use crate::Id;

/// Generates element ids in the shape StarUML writes:
/// base64 of `timestamp_ms (8 bytes BE) + counter (2) + random (2)`.
///
/// Ids are only ever compared, so the shape matters for familiarity, not
/// correctness.
#[derive(Debug, Clone)]
pub struct IdGen {
    counter: u16,
    state: u64,
}

impl Default for IdGen {
    fn default() -> Self {
        let nanos = now_ms().wrapping_mul(1_000_003) ^ 0x9E37_79B9_7F4A_7C15;
        Self {
            counter: (nanos >> 17) as u16,
            state: nanos | 1,
        }
    }
}

impl IdGen {
    pub fn next_id(&mut self) -> Id {
        self.counter = self.counter.wrapping_add(1);
        // xorshift64: enough to keep ids made in the same millisecond apart.
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;

        let mut bytes = [0u8; 12];
        bytes[..8].copy_from_slice(&now_ms().to_be_bytes());
        bytes[8..10].copy_from_slice(&self.counter.to_be_bytes());
        bytes[10..].copy_from_slice(&(self.state as u16).to_be_bytes());
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }
}

fn now_ms() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_staruml_shaped() {
        let mut ids = IdGen::default();
        let seen: std::collections::HashSet<_> = (0..10_000).map(|_| ids.next_id()).collect();
        assert_eq!(seen.len(), 10_000);
        assert!(seen.iter().all(|id| id.len() == 16));
    }
}
