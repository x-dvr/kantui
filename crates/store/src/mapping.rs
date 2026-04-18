//! Conversions between SQLite column values and domain types.
//!
//! SQLite stores IDs as lowercase hex-32 TEXT (matching [`EntityId`]'s `Display`)
//! and timestamps as INTEGER milliseconds since the Unix epoch.

use std::time::{Duration, UNIX_EPOCH};

use kantui_core::{Color, Complexity, CoreError, CoreResult, EntityId, Priority, Timestamp};

#[must_use]
pub fn id_to_text(id: EntityId) -> String {
    id.to_string()
}

pub fn id_from_text(s: &str) -> CoreResult<EntityId> {
    if s.len() != 32 {
        return Err(CoreError::storage(
            format!("malformed id: expected 32 hex chars, got {}", s.len()),
            io_err("malformed id length"),
        ));
    }
    let mut bytes = [0u8; 16];
    for i in 0..16 {
        let hex = &s[i * 2..i * 2 + 2];
        bytes[i] = u8::from_str_radix(hex, 16)
            .map_err(|e| CoreError::storage(format!("malformed id byte {i}"), e))?;
    }
    Ok(EntityId::from_bytes(bytes))
}

#[must_use]
pub fn ts_to_millis(ts: Timestamp) -> i64 {
    let d = ts
        .to_system_time()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

#[must_use]
pub fn ts_from_millis(m: i64) -> Timestamp {
    let m_u = u64::try_from(m.max(0)).unwrap_or(0);
    Timestamp::from_system_time(UNIX_EPOCH + Duration::from_millis(m_u))
}

#[must_use]
pub fn priority_to_text(p: Priority) -> &'static str {
    match p {
        Priority::Low => "low",
        Priority::Normal => "normal",
        Priority::High => "high",
        Priority::Critical => "critical",
    }
}

pub fn priority_from_text(s: &str) -> CoreResult<Priority> {
    match s {
        "low" => Ok(Priority::Low),
        "normal" => Ok(Priority::Normal),
        "high" => Ok(Priority::High),
        "critical" => Ok(Priority::Critical),
        other => Err(CoreError::storage(
            format!("unknown priority: {other}"),
            io_err("unknown priority"),
        )),
    }
}

#[must_use]
pub fn complexity_to_text(c: Complexity) -> &'static str {
    match c {
        Complexity::Light => "light",
        Complexity::Deep => "deep",
    }
}

pub fn complexity_from_text(s: &str) -> CoreResult<Complexity> {
    match s {
        "light" => Ok(Complexity::Light),
        "deep" => Ok(Complexity::Deep),
        other => Err(CoreError::storage(
            format!("unknown complexity: {other}"),
            io_err("unknown complexity"),
        )),
    }
}

#[must_use]
pub fn color_to_text(c: Color) -> String {
    match c {
        Color::Red => "red".into(),
        Color::Green => "green".into(),
        Color::Yellow => "yellow".into(),
        Color::Blue => "blue".into(),
        Color::Magenta => "magenta".into(),
        Color::Cyan => "cyan".into(),
        Color::White => "white".into(),
        Color::Gray => "gray".into(),
        Color::Custom([r, g, b]) => format!("#{r:02x}{g:02x}{b:02x}"),
    }
}

pub fn color_from_text(s: &str) -> CoreResult<Color> {
    match s {
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "white" => Ok(Color::White),
        "gray" => Ok(Color::Gray),
        hex if hex.len() == 7 && hex.starts_with('#') => {
            let parse_byte = |start: usize| -> CoreResult<u8> {
                u8::from_str_radix(&hex[start..start + 2], 16)
                    .map_err(|e| CoreError::storage(format!("malformed color hex: {hex}"), e))
            };
            Ok(Color::Custom([
                parse_byte(1)?,
                parse_byte(3)?,
                parse_byte(5)?,
            ]))
        }
        other => Err(CoreError::storage(
            format!("unknown color: {other}"),
            io_err("unknown color"),
        )),
    }
}

fn io_err(msg: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_roundtrip() {
        let id = EntityId::from_bytes([0xab; 16]);
        let text = id_to_text(id);
        assert_eq!(text, "abababababababababababababababab");
        let back = id_from_text(&text).unwrap();
        assert_eq!(back.as_bytes(), id.as_bytes());
    }

    #[test]
    fn id_rejects_bad_length() {
        assert!(id_from_text("abc").is_err());
    }

    #[test]
    fn color_custom_roundtrip() {
        let c = Color::Custom([0x12, 0x34, 0x56]);
        assert_eq!(color_to_text(c), "#123456");
        assert_eq!(color_from_text("#123456").unwrap(), c);
    }
}
