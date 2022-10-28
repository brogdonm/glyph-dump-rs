use std::{error, fmt, io};

/// Application errors
#[derive(Debug)]
pub enum AppError {
    /// The std::io error
    Io(io::Error),
    /// Image error from the image library
    ImageError(image::ImageError),
    /// General error
    General(&'static str),
    /// Formatted message error
    FormattedMessage(String),
}

/// Display format implementation for our custom error
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (module, e) = match self {
            AppError::Io(e) => ("IO", e.to_string()),
            AppError::ImageError(e) => ("ImageError", e.to_string()),
            AppError::General(e) => ("app", format!("Error: {}", e.to_string())),
            AppError::FormattedMessage(e) => ("app", e.to_string()),
        };
        write!(f, "error in {}: {}", module, e)
    }
}

impl error::Error for AppError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(match self {
            AppError::Io(e) => e,
            AppError::ImageError(e) => e,
            AppError::General(_e) => return None,
            AppError::FormattedMessage(_e) => return None,
        })
    }
}

/// From mapping from the std::io::Error to our error type
impl From<io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}

/// From mapping from the image::ImageError to our error type
impl From<image::ImageError> for AppError {
    fn from(e: image::ImageError) -> Self {
        AppError::ImageError(e)
    }
}
