//! Standard base64 (RFC 4648, with `=` padding), for file bytes passed
//! inside JSON.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes `bytes` with padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn value(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some(u32::from(c - b'A')),
        b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decodes base64; white space is skipped and padding is optional.
pub fn decode(text: &str) -> Result<Vec<u8>, String> {
    let digits: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let body = digits
        .iter()
        .position(|&c| c == b'=')
        .map_or(&digits[..], |p| &digits[..p]);
    if digits[body.len()..].iter().any(|&c| c != b'=') || digits.len() - body.len() > 2 {
        return Err("base64: padding in the middle".to_owned());
    }
    if body.len() % 4 == 1 {
        return Err("base64: wrong length".to_owned());
    }
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    for chunk in body.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            let v = value(c).ok_or_else(|| format!("base64: bad character {:?}", c as char))?;
            n |= v << (18 - 6 * i);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(&bytes[..chunk.len() - 1]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_length() {
        let data: Vec<u8> = (0..=255u8).collect();
        for n in 0..20 {
            let e = encode(&data[..n]);
            assert_eq!(e.len() % 4, 0);
            assert_eq!(decode(&e).unwrap(), &data[..n]);
        }
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
        assert_eq!(decode("TWE").unwrap(), b"Ma");
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode("T$Fu").is_err());
        assert!(decode("TW=u").is_err());
        assert!(decode("TWFuT").is_err());
    }
}
