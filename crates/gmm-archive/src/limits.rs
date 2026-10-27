// SPDX-License-Identifier: MIT OR Apache-2.0

/// Extraction limits. Defaults follow `docs/security.md`.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Total decompressed bytes across all entries.
    pub max_total_bytes: u64,
    pub max_entries: usize,
    /// Maximum decompressed/compressed ratio, per entry (zip) and for the whole archive.
    pub max_ratio: u64,
    /// Below this many decompressed bytes the ratio is not enforced, since small text files
    /// legitimately compress very well.
    pub ratio_floor_bytes: u64,
    /// Characters in an entry's full relative path.
    pub max_path_len: usize,
    /// Folder depth of an entry.
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_total_bytes: 4 * 1024 * 1024 * 1024,
            max_entries: 20_000,
            max_ratio: 200,
            // Mostly blank FX/mask textures (8192² BC7 is 64 MiB plus a header) can compress
            // past 300:1 and must still install.
            ratio_floor_bytes: 128 * 1024 * 1024,
            max_path_len: 240,
            max_depth: 16,
        }
    }
}

impl Limits {
    /// Most bytes allowed to come out of `compressed` bytes of input.
    pub(crate) fn ratio_cap(&self, compressed: u64) -> u64 {
        compressed
            .saturating_mul(self.max_ratio)
            .max(self.ratio_floor_bytes)
    }
}
