use std::fmt::Display;

use serde::{Deserialize, Serialize};

/// Error type for file operations.
#[repr(i64)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Status {
    NakedStream(String) = -1,
    None = 0,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Status::NakedStream(s) => s,
            Status::None => "None",
        };
        write!(f, "{}", s)
    }
}

impl Default for Status {
    fn default() -> Self {
        Self::None
    }
}
