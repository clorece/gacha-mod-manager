// SPDX-License-Identifier: MIT OR Apache-2.0

/// Archive formats, identified by content, never by file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zip,
    SevenZ,
    Rar,
}

const ZIP_LOCAL: &[u8] = b"PK\x03\x04";
const ZIP_EMPTY: &[u8] = b"PK\x05\x06";
const SEVEN_Z: &[u8] = b"7z\xBC\xAF\x27\x1C";
const RAR: &[u8] = b"Rar!\x1A\x07";

/// Identifies an archive from its first bytes (8 are enough).
pub fn detect_format(header: &[u8]) -> Option<Format> {
    if header.starts_with(ZIP_LOCAL) || header.starts_with(ZIP_EMPTY) {
        Some(Format::Zip)
    } else if header.starts_with(SEVEN_Z) {
        Some(Format::SevenZ)
    } else if header.starts_with(RAR) {
        Some(Format::Rar)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_by_magic() {
        assert_eq!(detect_format(b"PK\x03\x04rest"), Some(Format::Zip));
        assert_eq!(
            detect_format(b"7z\xBC\xAF\x27\x1C\x00\x04"),
            Some(Format::SevenZ)
        );
        assert_eq!(detect_format(b"Rar!\x1A\x07\x01\x00"), Some(Format::Rar));
        assert_eq!(detect_format(b"MZ\x90\x00"), None);
        assert_eq!(detect_format(b""), None);
    }
}
