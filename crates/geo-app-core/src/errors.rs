//! Polish wording for the kernel's English script errors.

/// Turns a kernel error message into a short Polish one. Messages that have
/// no translation keep their English text after a Polish prefix.
pub fn polish(message: &str) -> String {
    let tick = |s: &str| -> Option<String> {
        let start = s.find('`')?;
        let end = s[start + 1..].find('`')? + start + 1;
        Some(s[start + 1..end].to_owned())
    };
    if message.starts_with("expected `name = definition`") {
        return "Oczekiwano postaci nazwa = definicja".into();
    }
    if let Some(name) = message.strip_prefix("unknown object ").and_then(tick) {
        return format!("Nieznany obiekt `{name}`");
    }
    if let Some(name) = message.strip_prefix("unknown command ").and_then(tick) {
        return format!("Nieznane polecenie `{name}`");
    }
    if message.ends_with("is not a valid object name") {
        let name = tick(message).unwrap_or_default();
        return format!("`{name}` nie jest poprawną nazwą obiektu");
    }
    if message.ends_with("is not a number") {
        let value = tick(message).unwrap_or_default();
        return format!("`{value}` nie jest liczbą");
    }
    if message.contains("does not take") && message.contains("argument") {
        let name = tick(message).unwrap_or_default();
        return format!("Zła liczba argumentów polecenia `{name}`");
    }
    if message.starts_with("a point needs two coordinates") {
        return "Punkt wymaga dwóch współrzędnych: (x, y)".into();
    }
    if message.starts_with("cannot intersect") {
        return "Tych obiektów nie można przeciąć".into();
    }
    if message.ends_with("already exists") {
        let name = tick(message).unwrap_or_default();
        return format!("Nazwa `{name}` jest już zajęta");
    }
    format!("Błąd: {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_messages_are_translated() {
        assert_eq!(polish("unknown object `Q`"), "Nieznany obiekt `Q`");
        assert_eq!(polish("unknown command `Foo`"), "Nieznane polecenie `Foo`");
        assert_eq!(
            polish("expected `name = definition`"),
            "Oczekiwano postaci nazwa = definicja"
        );
        assert_eq!(
            polish("`1x` is not a valid object name"),
            "`1x` nie jest poprawną nazwą obiektu"
        );
        assert_eq!(polish("`abc` is not a number"), "`abc` nie jest liczbą");
        assert_eq!(
            polish("`Line` does not take 3 argument(s)"),
            "Zła liczba argumentów polecenia `Line`"
        );
        assert_eq!(
            polish("object `A` already exists"),
            "Nazwa `A` jest już zajęta"
        );
        assert_eq!(
            polish("cannot intersect a point with a line"),
            "Tych obiektów nie można przeciąć"
        );
    }

    #[test]
    fn unknown_messages_keep_their_text() {
        assert_eq!(polish("something odd"), "Błąd: something odd");
    }
}
