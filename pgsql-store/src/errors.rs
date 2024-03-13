use std::error::Error;

#[derive(Debug)]
pub enum ConnectionPoolError {
    PoolInitializationError(String)
}

impl std::fmt::Display for ConnectionPoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionPoolError::PoolInitializationError(err) => 
                write!(f, "PoolInitializationError: {}", err),
        }
    }
}

impl Error for ConnectionPoolError {}






