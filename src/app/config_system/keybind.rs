//! Keyboard/mouse bind representation for module keybinds.
//!
//! Keybinds are stored in `config.toml` as plain strings (the variant name,
//! e.g. `"F"` or `"MouseRight"`), so they round-trip through the existing
//! `string`/`enum` config plumbing. This enum is the strongly-typed Rust
//! representation: parse a stored value with [`Key::from_name`] and render the
//! selectable list in the menu with [`Key::variants`].

use std::fmt;

/// A bindable key or mouse button. `None` means "unbound".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Key {
    #[default]
    None,
    // Mouse
    MouseLeft,
    MouseRight,
    MouseMiddle,
    Mouse4,
    Mouse5,
    // Letters
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    // Digits
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    // Function
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    // Navigation / editing
    Up, Down, Left, Right,
    Space, Enter, Tab, Escape, Backspace,
    Insert, Delete, Home, End, PageUp, PageDown,
    LeftShift, RightShift, LeftControl, RightControl, LeftAlt, RightAlt,
}

impl Key {
    /// Every variant, in menu order. The `&'static str` is the stable name used
    /// both as the config value and the label shown in the UI.
    pub fn variants() -> &'static [Key] {
        use Key::*;
        &[
            None,
            MouseLeft, MouseRight, MouseMiddle, Mouse4, Mouse5,
            A, B, C, D, E, F, G, H, I, J, K, L, M,
            N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
            Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
            F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
            Up, Down, Left, Right,
            Space, Enter, Tab, Escape, Backspace,
            Insert, Delete, Home, End, PageUp, PageDown,
            LeftShift, RightShift, LeftControl, RightControl, LeftAlt, RightAlt,
        ]
    }

    /// Stable name used as the config value and UI label.
    pub fn name(self) -> &'static str {
        use Key::*;
        match self {
            None => "None",
            MouseLeft => "MouseLeft",
            MouseRight => "MouseRight",
            MouseMiddle => "MouseMiddle",
            Mouse4 => "Mouse4",
            Mouse5 => "Mouse5",
            A => "A", B => "B", C => "C", D => "D", E => "E", F => "F", G => "G",
            H => "H", I => "I", J => "J", K => "K", L => "L", M => "M", N => "N",
            O => "O", P => "P", Q => "Q", R => "R", S => "S", T => "T", U => "U",
            V => "V", W => "W", X => "X", Y => "Y", Z => "Z",
            Num0 => "Num0", Num1 => "Num1", Num2 => "Num2", Num3 => "Num3", Num4 => "Num4",
            Num5 => "Num5", Num6 => "Num6", Num7 => "Num7", Num8 => "Num8", Num9 => "Num9",
            F1 => "F1", F2 => "F2", F3 => "F3", F4 => "F4", F5 => "F5", F6 => "F6",
            F7 => "F7", F8 => "F8", F9 => "F9", F10 => "F10", F11 => "F11", F12 => "F12",
            Up => "Up", Down => "Down", Left => "Left", Right => "Right",
            Space => "Space", Enter => "Enter", Tab => "Tab", Escape => "Escape",
            Backspace => "Backspace", Insert => "Insert", Delete => "Delete",
            Home => "Home", End => "End", PageUp => "PageUp", PageDown => "PageDown",
            LeftShift => "LeftShift", RightShift => "RightShift",
            LeftControl => "LeftControl", RightControl => "RightControl",
            LeftAlt => "LeftAlt", RightAlt => "RightAlt",
        }
    }

    /// Parse a stored config value back into a `Key`. Unknown names → `None`.
    pub fn from_name(name: &str) -> Key {
        Key::variants()
            .iter()
            .copied()
            .find(|k| k.name().eq_ignore_ascii_case(name))
            .unwrap_or(Key::None)
    }

    /// All variant names, for building the menu's selectable list.
    pub fn variant_names() -> Vec<&'static str> {
        Key::variants().iter().map(|k| k.name()).collect()
    }

    pub fn is_bound(self) -> bool {
        self != Key::None
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for Key {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Key::from_name(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_by_name() {
        for key in Key::variants() {
            assert_eq!(Key::from_name(key.name()), *key);
        }
    }

    #[test]
    fn unknown_and_case_insensitive() {
        assert_eq!(Key::from_name("mouseright"), Key::MouseRight);
        assert_eq!(Key::from_name("nonsense"), Key::None);
        assert_eq!(Key::variant_names().first().copied(), Some("None"));
    }
}
