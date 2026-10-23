// SPDX-License-Identifier: MIT OR Apache-2.0

//! Corrupted archives must fail with an error, never panic, and never write outside the root.
//! A cheap stand-in for coverage-guided fuzzing (cargo-fuzz targets are a later CI job).

#![allow(clippy::disallowed_methods, clippy::unwrap_used)]

mod common;

use std::sync::OnceLock;

use common::{E, Env, sevenz_bytes, zip_bytes};
use gmm_archive::Limits;
use proptest::prelude::*;
use zip::CompressionMethod;

fn seeds() -> &'static [Vec<u8>; 2] {
    static SEEDS: OnceLock<[Vec<u8>; 2]> = OnceLock::new();
    SEEDS.get_or_init(|| {
        let entries = [
            E::Dir("Mod/"),
            E::File("Mod/mod.ini", b"[TextureOverrideA]\nhash = 1\n"),
            E::File("Mod/tex/a.dds", &[5u8; 3000]),
            E::File("Mod/b.buf", b"0123456789abcdef0123456789abcdef"),
        ];
        [
            zip_bytes(&entries, CompressionMethod::Deflated),
            sevenz_bytes(&entries),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn mutated_archives_never_panic_or_escape(
        which in 0usize..2,
        edits in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 1..12),
        truncate in prop::option::of(any::<prop::sample::Index>()),
    ) {
        let mut bytes = seeds()[which].clone();
        for (index, value) in edits {
            let i = index.index(bytes.len());
            bytes[i] = value;
        }
        if let Some(cut) = truncate {
            let len = cut.index(bytes.len()).max(1);
            bytes.truncate(len);
        }
        let env = Env::new();
        // Ok or Err are both fine; Env::extract asserts cleanup and that nothing escaped.
        let _ = env.extract(&bytes, &Limits::default());
    }
}
