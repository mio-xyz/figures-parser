//! The kinds of figure `extract` returns. The weekday check reads pairs
//! rather than figures, so it is a comparison with no kind.

use std::fmt;
use std::str::FromStr;

/// A kind of figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A number written in digits, in any script.
    Number,
    /// An http(s) link.
    Url,
    /// An email address.
    Email,
    /// An identifier: a UUID, a hash, a tracker key, a prefixed id, a long
    /// mixed token.
    Id,
    /// A length of time.
    Duration,
}

impl Kind {
    /// Every kind, in the order `extract` reads them.
    pub const ALL: [Self; 5] = [
        Self::Number,
        Self::Url,
        Self::Email,
        Self::Id,
        Self::Duration,
    ];

    /// The name a caller selects the kind by.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::Url => "url",
            Self::Email => "email",
            Self::Id => "id",
            Self::Duration => "duration",
        }
    }
}

/// A kind name the library does not know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownKind(pub String);

impl fmt::Display for UnknownKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let known: Vec<&str> = Kind::ALL.iter().map(|k| k.name()).collect();
        write!(f, "unknown kind {:?}; known: {}", self.0, known.join(", "))
    }
}

impl std::error::Error for UnknownKind {}

impl FromStr for Kind {
    type Err = UnknownKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|k| k.name() == s)
            .ok_or_else(|| UnknownKind(s.to_owned()))
    }
}
