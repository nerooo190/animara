use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Process-unique identifier for document objects (layers, elements, symbols, assets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Id(pub u64);

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Allocate a new unique id.
pub fn fresh_id() -> Id {
    Id(NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Ensure future ids are greater than `max` (called after loading a document).
pub fn bump_ids_above(max: u64) {
    NEXT.fetch_max(max + 1, Ordering::Relaxed);
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
