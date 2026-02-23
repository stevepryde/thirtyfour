use std::{
    fmt::{self, Display},
    ops::Add,
};

#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub enum Key {
    Null,
    Cancel,
    Help,
    Backspace,
    Tab,
    Clear,
    Return,
    Enter,
    Shift,
    Control,
    Alt,
    Pause,
    Escape,
    Space,
    PageUp,
    PageDown,
    End,
    Home,
    Left,
    Up,
    Right,
    Down,
    Insert,
    Delete,
    Semicolon,
    Equals,
    NumPad0,
    NumPad1,
    NumPad2,
    NumPad3,
    NumPad4,
    NumPad5,
    NumPad6,
    NumPad7,
    NumPad8,
    NumPad9,
    Multiply,
    Add,
    Separator,
    Subtract,
    Decimal,
    Divide,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Meta,
    Command,
}

impl Key {
    /// Get the char value of the key.
    #[must_use]
    pub fn value(&self) -> char {
        match self {
            Key::Null => '\u{e000}',
            Key::Cancel => '\u{e001}',
            Key::Help => '\u{e002}',
            Key::Backspace => '\u{e003}',
            Key::Tab => '\u{e004}',
            Key::Clear => '\u{e005}',
            Key::Return => '\u{e006}',
            Key::Enter => '\u{e007}',
            Key::Shift => '\u{e008}',
            Key::Control => '\u{e009}',
            Key::Alt => '\u{e00a}',
            Key::Pause => '\u{e00b}',
            Key::Escape => '\u{e00c}',
            Key::Space => '\u{e00d}',
            Key::PageUp => '\u{e00e}',
            Key::PageDown => '\u{e00f}',
            Key::End => '\u{e010}',
            Key::Home => '\u{e011}',
            Key::Left => '\u{e012}',
            Key::Up => '\u{e013}',
            Key::Right => '\u{e014}',
            Key::Down => '\u{e015}',
            Key::Insert => '\u{e016}',
            Key::Delete => '\u{e017}',
            Key::Semicolon => '\u{e018}',
            Key::Equals => '\u{e019}',
            Key::NumPad0 => '\u{e01a}',
            Key::NumPad1 => '\u{e01b}',
            Key::NumPad2 => '\u{e01c}',
            Key::NumPad3 => '\u{e01d}',
            Key::NumPad4 => '\u{e01e}',
            Key::NumPad5 => '\u{e01f}',
            Key::NumPad6 => '\u{e020}',
            Key::NumPad7 => '\u{e021}',
            Key::NumPad8 => '\u{e022}',
            Key::NumPad9 => '\u{e023}',
            Key::Multiply => '\u{e024}',
            Key::Add => '\u{e025}',
            Key::Separator => '\u{e026}',
            Key::Subtract => '\u{e027}',
            Key::Decimal => '\u{e028}',
            Key::Divide => '\u{e029}',
            Key::F1 => '\u{e031}',
            Key::F2 => '\u{e032}',
            Key::F3 => '\u{e033}',
            Key::F4 => '\u{e034}',
            Key::F5 => '\u{e035}',
            Key::F6 => '\u{e036}',
            Key::F7 => '\u{e037}',
            Key::F8 => '\u{e038}',
            Key::F9 => '\u{e039}',
            Key::F10 => '\u{e03a}',
            Key::F11 => '\u{e03b}',
            Key::F12 => '\u{e03c}',
            Key::Meta | Key::Command => '\u{e03d}',
        }
    }
}

impl<S> Add<S> for Key
where
    S: Into<TypingData>,
{
    type Output = TypingData;

    fn add(self, rhs: S) -> Self::Output {
        let data = vec![self.value()];
        Self::Output {
            data,
        } + rhs
    }
}

impl From<Key> for char {
    fn from(k: Key) -> Self {
        k.value()
    }
}

/// `TypingData` is a wrapper around a `Vec<char>` that can be used to send Key to the browser.
#[derive(Debug)]
pub struct TypingData {
    data: Vec<char>,
}

impl TypingData {
    /// Get the underlying `Vec<char>`.
    #[must_use]
    pub fn as_vec(&self) -> Vec<char> {
        self.data.clone()
    }
}

impl Display for TypingData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.data.iter().collect::<String>())
    }
}

impl<S> From<S> for TypingData
where
    S: Into<String>,
{
    fn from(value: S) -> Self {
        TypingData {
            data: value.into().chars().collect(),
        }
    }
}

impl From<Key> for TypingData {
    fn from(value: Key) -> Self {
        TypingData {
            data: vec![value.value()],
        }
    }
}

impl<S> Add<S> for TypingData
where
    S: Into<TypingData>,
{
    type Output = TypingData;

    fn add(self, rhs: S) -> Self::Output {
        let mut data = self.data;
        data.extend(rhs.into().data.iter());
        Self::Output {
            data,
        }
    }
}

impl Add<Key> for &str {
    type Output = TypingData;

    fn add(self, rhs: Key) -> Self::Output {
        TypingData::from(self) + rhs
    }
}

impl Add<Key> for String {
    type Output = TypingData;

    fn add(self, rhs: Key) -> Self::Output {
        TypingData::from(self) + rhs
    }
}

impl Add<TypingData> for &str {
    type Output = TypingData;

    fn add(self, rhs: TypingData) -> Self::Output {
        TypingData::from(self) + rhs
    }
}

impl Add<TypingData> for String {
    type Output = TypingData;

    fn add(self, rhs: TypingData) -> Self::Output {
        TypingData::from(self) + rhs
    }
}
