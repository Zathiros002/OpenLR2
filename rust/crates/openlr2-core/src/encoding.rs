//! Encoding utilities.
//!
//! LR2 / BMS files mix several encodings:
//! - UTF-8 (modern files, OpenLR2 default)
//! - CP932 / Shift_JIS (legacy Japanese BMS files)
//! - UTF-16 (Windows paths internally)
//! - UCS-2 (older Windows API)
//!
//! This module will provide conversion routines as needed.
//! Initial implementation: stub for UTF-8 identity; CP932 decoding will be
//! added when BMS parser encounters legacy files.

/// Placeholder for CP932 → UTF-8 conversion.
///
/// In the C++ codebase this is handled by `utf2ansi` / `ansi2utf` in
/// `En_fileutil.cpp`.
pub fn cp932_to_utf8(input: &[u8]) -> Result<String, crate::OpenLr2Error> {
    // TODO: implement proper CP932 → UTF-8 via encoding_rs or similar.
    // For now, pass through as UTF-8 (valid for modern BMS files).
    String::from_utf8(input.to_vec())
        .map_err(|e| crate::OpenLr2Error::Encoding(format!("invalid utf-8: {}", e)))
}

/// Placeholder for UTF-8 → CP932 conversion.
pub fn utf8_to_cp932(input: &str) -> Result<Vec<u8>, crate::OpenLr2Error> {
    // TODO: implement proper conversion.
    Ok(input.as_bytes().to_vec())
}
