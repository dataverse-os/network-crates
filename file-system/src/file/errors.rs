#[derive(Debug)]
pub enum FileError {
    NoControllerError,

}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileError::NoControllerError => write!(f, "No Controller"),
        }
    }
}

impl std::error::Error for FileError {}

