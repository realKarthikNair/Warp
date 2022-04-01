use crate::globals::TRANSMIT_CODE_FIND_REGEX;

pub mod error;
pub mod future;

pub fn extract_transmit_code(str: &str) -> Option<String> {
    TRANSMIT_CODE_FIND_REGEX
        .find(str)
        .map(|m| m.as_str().to_string())
}
