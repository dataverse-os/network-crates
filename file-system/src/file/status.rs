use int_enum::IntEnum;
use serde::{Deserialize, Serialize};

/// Error type for file operations.
#[repr(i64)]
#[derive(Debug, Clone, Copy, Deserialize, Serialize, IntEnum)]
pub enum Status {
	Validated = 1,
	None = 0,
	NakedStream = -1,
	CACAOExpired = -2,
	BrokenContent = -3,
}

impl Default for Status {
	fn default() -> Self {
		Self::None
	}
}
