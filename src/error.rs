use std::{error, fmt, io};

/// Application errors
#[derive(Debug)]
pub enum AppError {
    /// Parsing error for a hex color.
    ColorParseError(String),
    /// Hex string error.
    HexStringError(hex_string::HexStringError),
    /// The std::io error
    Io(io::Error),
    /// Image error from the image library
    ImageError(image::ImageError),
    /// Invalid range error.
    InvalidRange(),
    /// Integer parsing error.
    ParseIntError(std::num::ParseIntError),
    /// General error
    General(&'static str),
    /// Error when a glyph is not defined
    GlyphNotDefined(char),
    /// Formatted message error
    FormattedMessage(String),
    /// Out of range unicode error.
    OutOfRangeUnicode(String),
}

/// Display format implementation for our custom error
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (module, e) = match self {
            AppError::ColorParseError(e) => (
                "app",
                format!(
                    "Error parsing color, expected a hex color string got: {}",
                    e
                ),
            ),
            AppError::Io(e) => ("IO", e.to_string()),
            AppError::ImageError(e) => ("ImageError", e.to_string()),
            AppError::InvalidRange() => ("app", "Invalid range specified".to_string()),
            AppError::HexStringError(e) => ("hex_string", format!("Error: {:?}", e)),
            AppError::ParseIntError(e) => ("ParseIntError", e.to_string()),
            AppError::General(e) => ("app", format!("Error: {}", e)),
            AppError::GlyphNotDefined(e) => ("app", format!("Glyph not defined for: {}", e)),
            AppError::FormattedMessage(e) => ("app", e.to_string()),
            AppError::OutOfRangeUnicode(e) => {
                ("app", format!("Unicode value is out of range: {}", e))
            }
        };
        write!(f, "error in {}: {}", module, e)
    }
}

impl error::Error for AppError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(match self {
            AppError::ColorParseError(_e) => return None,
            AppError::HexStringError(_e) => return None,
            AppError::Io(e) => e,
            AppError::ImageError(e) => e,
            AppError::InvalidRange() => return None,
            AppError::ParseIntError(e) => e,
            AppError::General(_e) => return None,
            AppError::GlyphNotDefined(_e) => return None,
            AppError::FormattedMessage(_e) => return None,
            AppError::OutOfRangeUnicode(_e) => return None,
        })
    }
}

/// Mapping from hex_string::HexStringError to our local error type.
impl From<hex_string::HexStringError> for AppError {
    fn from(e: hex_string::HexStringError) -> Self {
        AppError::HexStringError(e)
    }
}

/// From mapping from the std::io::Error to our error type
impl From<io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}

/// Mapping from std::num::ParseIntError to our local error type.
impl From<std::num::ParseIntError> for AppError {
    fn from(e: std::num::ParseIntError) -> Self {
        AppError::ParseIntError(e)
    }
}

/// From mapping from the image::ImageError to our error type
impl From<image::ImageError> for AppError {
    fn from(e: image::ImageError) -> Self {
        AppError::ImageError(e)
    }
}
