pub fn encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn decode(input: &str) -> Option<Vec<u8>> {
    if !input.bytes().all(|byte| byte.is_ascii_hexdigit()) || !input.len().is_multiple_of(2) {
        return None;
    }
    (0..input.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&input[index..index + 2], 16).ok())
        .collect()
}
