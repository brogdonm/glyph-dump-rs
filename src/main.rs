use clap::{arg, Parser};
use image::{DynamicImage, Rgba};
use log::debug;
use rusttype::{point, Font, Glyph, Rect, Scale};
use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync,
};
use unicode_categories::UnicodeCategories;
mod error;
use crate::error::AppError;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Representation of a color
#[derive(Debug, Clone)]
struct Color {
    /// Red component
    pub red: u8,
    /// Green component
    pub green: u8,
    /// Blue component
    pub blue: u8,
}

impl FromStr for Color {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Verify the length of the hex color is in the form of #rRgGbB
        if s.len() != 7 {
            return Err(AppError::ColorParseError(s.to_string()));
        }
        // Parse the red color
        let red = u8::from_str_radix(&s[1..3], 16)?;
        // Parse the green color
        let green = u8::from_str_radix(&s[3..5], 16)?;
        // Parse the blue color
        let blue = u8::from_str_radix(&s[5..7], 16)?;
        Ok(Self { red, green, blue })
    }
}

/// Specifies a range of unicode values to dump
#[derive(Debug, Clone)]
struct UnicodeRange {
    /// Starting unicode in a range (inclusive)
    pub start: UnicodeValue,
    /// Ending unicode in a range (inclusive)
    pub end: UnicodeValue,
}

impl FromStr for UnicodeRange {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pieces = s.split("..").collect::<Vec<_>>();
        if pieces.len() != 2 {
            return Err(AppError::General(
                "Failed to parse unicode range, invalid format",
            ));
        }
        let start = UnicodeValue::from_str(pieces[0])?;
        let end = UnicodeValue::from_str(pieces[1])?;
        Ok(Self { start, end })
    }
}

/// A wrapper structure around a character for parsing command line argument
/// using AutoArgs.
#[derive(Debug, Clone)]
struct UnicodeValue {
    /// The character
    character: char,
}

impl FromStr for UnicodeValue {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse the argument as a string first
        let the_arg = s
            // Removing the 0x
            .replace("0x", "")
            // or the U+
            .replace("U+", "");

        // Convert our input string to a hex string and then into a vector of u8 values
        let the_arg_bytes = hex_string::HexString::from_string(&the_arg)
            .map_err(|x| {
                AppError::FormattedMessage(format!(
                    "Failed to convert base name to str; {:?} (expected to be multiples of 2)",
                    x
                ))
            })?
            .as_bytes();
        // Setup a 32-bit array
        let mut arg_u32: [u8; 4] = [0u8; 4];
        // And copy in our slice from the conversion
        arg_u32[(4 - the_arg_bytes.len())..].copy_from_slice(&the_arg_bytes[..]);
        // Read in the value as u32 from big endian bytes
        let val: u32 = u32::from_be_bytes(arg_u32);
        unsafe {
            Ok(Self {
                character: char::from_u32_unchecked(val),
            })
        }
    }
}

/// Conversion from our unicode value to the primitive char type.
impl From<UnicodeValue> for char {
    fn from(item: UnicodeValue) -> char {
        item.character
    }
}

/// Dumps glyphs from a specified font.
#[derive(Parser, Debug)]
struct CliArgs {
    /// Path to the font file to render
    #[arg(short, long)]
    pub font_file: String,
    /// Optional output directory for images
    #[arg(short, long, default_value = "./out")]
    pub output_dir: String,
    /// Optional hex color used for output
    #[arg(short, long, default_value = "#ffffff")]
    pub color: Color,
    /// Size to make the image for each glyph
    #[arg(short, long, default_value_t = 128)]
    pub img_size: u32,
    #[cfg(feature = "parallel")]
    #[arg(short, long)]
    /// Optional number of threads to configure the thread pool for.
    pub number_of_threads: Option<usize>,
    /// Optional range (inclusively) of unicode values to dump, for example 0x0030..0x00ff
    #[arg(short, long, verbatim_doc_comment)]
    pub unicode_range: Option<UnicodeRange>,
}

/// Calculates the height and width of a glyph.
trait GlyphDimensions {
    /// Calculates the height of the glyph
    fn get_glyph_height(&self) -> u32;

    /// Calculates the width of the glyph
    fn get_glyph_width(&self) -> u32;
}

/// Implementation for the GlyphDimensions using the `Rect<i32>` from
/// `rusttype` mod.
impl GlyphDimensions for Rect<i32> {
    fn get_glyph_height(&self) -> u32 {
        {
            let min_y = self.min.y;
            let max_y = self.max.y;
            (min_y - max_y).unsigned_abs() as u32
        }
    }

    fn get_glyph_width(&self) -> u32 {
        {
            let min_x = self.min.x;
            let max_x = self.max.x;
            (max_x - min_x) as u32
        }
    }
}

/// Gets the base name of the specified file path.
fn get_base_name(file_path: &str) -> Result<String, AppError> {
    // Grab the base name of the font file
    let base_name = std::path::Path::new(file_path)
        .file_name()
        .ok_or_else(|| {
            AppError::FormattedMessage(format!(
                "Failed to get file name for file path: {}",
                file_path
            ))
        })?
        .to_str()
        .ok_or(AppError::General("Failed to convert base name to str"))?;
    Ok(base_name.to_owned())
}

fn get_scale(glyph: Glyph, img_size: &u32) -> Result<Scale, AppError> {
    // We can get at the x/y min/max of the glyph directly, so we use a little
    // trick of scaling by 1.0 to get the exact bounding box to get the
    // dimensions.
    let one_to_one_scaling = glyph
        .scaled(Scale::uniform(1.0))
        .exact_bounding_box()
        .ok_or(AppError::General(
            "Failed to get exact bounding box for glyph",
        ))?;
    // Calculate the height and width of the glyph
    let height: f32 = one_to_one_scaling.max.y - one_to_one_scaling.min.y;
    let width: f32 = one_to_one_scaling.max.x - one_to_one_scaling.min.x;
    // Create a vector so we can find the max
    let dimensions: Vec<f32> = vec![height, width];
    // Iterate through the values and find the largest one
    let max_dimension = dimensions
        .iter()
        .max_by(|x, y| x.abs().partial_cmp(&y.abs()).unwrap())
        .unwrap()
        .abs();
    // Find the scaling factor
    let scale_factor = ((*img_size as f32) / max_dimension).floor();
    // And create a uniform scale from it
    Ok(Scale::uniform(scale_factor))
}

/// Converts a unicode character to a big endian hex string as 8 hex digits.
fn convert_to_be_hex_string(unicode: char) -> Result<String, AppError> {
    // Create a prefix for the unicode in a hex string
    let mut encoded_utf16: [u16; 2] = [0u16; 2];
    unicode.encode_utf16(&mut encoded_utf16);
    let mut be_bytes: Vec<u8> = Vec::with_capacity(4);
    // Convert to big endian bytes
    for n in (0..2).rev() {
        let val = encoded_utf16[n];
        be_bytes.push((val >> 8) as u8);
        be_bytes.push((val & 0xFF) as u8);
    }
    // And convert to hex
    Ok(be_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
    )
}

/// Creates an image for a glyph mapped to the specified unicode value
fn create_glyph_img<BD>(
    font: &Font,
    unicode: char,
    img_size: u32,
    output_color: &(u8, u8, u8),
    base_dir: BD,
) -> Result<Option<String>, AppError>
where
    BD: AsRef<Path>,
{
    // Get the glyph associated with the unicode
    let glyph = font.glyph(unicode);
    // Skip the glyph if we are dealing with .notdef
    if glyph.id().0 == 0 {
        return Err(AppError::GlyphNotDefined(unicode));
    }
    let scale = get_scale(glyph.clone(), &img_size)?;
    // Scale it and position at {0, 0}
    let positioned_glyph = glyph.scaled(scale).positioned(point(0.0, 0.0));
    debug!("Dealing with unicode: {:?}", unicode);

    // If we have a pixel bounding box for the glyph, we can draw it into
    // an image
    if let Some(bounding_box) = positioned_glyph.pixel_bounding_box() {
        debug!("\tBounding box: {:#?}", &bounding_box);
        // Grab the height and width of the glyph
        let glyph_height = bounding_box.get_glyph_height();
        let glyph_width = bounding_box.get_glyph_width();
        // Find the greatest size
        let max_sz = std::cmp::max(glyph_height, glyph_width);
        debug!("Glyph WxH: {}x{}", &glyph_width, &glyph_height);

        // Create a new 8-bit RGBA square image
        let mut image = DynamicImage::new_rgba8(max_sz, max_sz).to_rgba8();
        // Calculate x/y offsets before calling the draw command for a slight
        // optimization
        let x_offset = (max_sz - glyph_width) / 2;
        let y_offset = (max_sz - glyph_height) / 2;
        // Draw the single pixel into the image
        positioned_glyph.draw(|x, y, v| {
            image.put_pixel(
                x + x_offset as u32,
                y + y_offset as u32,
                Rgba([
                    output_color.0,
                    output_color.1,
                    output_color.2,
                    (v * 255.0) as u8,
                ]),
            );
        });

        // Create a prefix for the unicode in a hex string
        let hex_name = convert_to_be_hex_string(unicode)?;
        // Build up the image path from the base directory
        let mut image_path_buf = PathBuf::from(base_dir.as_ref());
        image_path_buf.push(format!("{}_image.png", &hex_name[2..8]));
        let image_path = Some(
            image_path_buf
                .into_os_string()
                .to_str()
                .ok_or(AppError::General("Failed to convert path to string"))?
                .to_string(),
        );
        // And save the image in our output directory
        image.save(image_path.as_ref().unwrap())?;
        Ok(image_path)
    }
    // Otherwise what has happened? Why couldn't we get the pixel bounding
    // box?
    else {
        Err(AppError::FormattedMessage(format!(
            "Failed to get pixel bounding box for unicode: {}",
            &unicode
        )))
    }
}

fn main() -> Result<(), AppError> {
    env_logger::init();
    let arguments = CliArgs::parse();
    debug!("Command line arguments: {:#?}", &arguments);

    let font_data = std::fs::read(&arguments.font_file)?;
    let font = sync::Arc::new(Font::try_from_vec(font_data).ok_or_else(|| {
        AppError::FormattedMessage(format!(
            "Failed to parse data from file: {}",
            &arguments.font_file
        ))
    })?);
    let valid_unicode_ranges: Vec<_>;
    // If user specified a range, use it
    if let Some(unicode_range) = arguments.unicode_range {
        valid_unicode_ranges =
            (unicode_range.start.character..=unicode_range.end.character).collect();
    } else {
        // Otherwise, we will use our own range
        valid_unicode_ranges = ('\u{0000}'..='\u{10FFFF}')
            .filter(|c| {
                c.is_alphabetic()
                    || c.is_alphanumeric()
                    || c.is_letter_other()
                    || c.is_symbol_other()
                    || c.is_punctuation()
                    || c.is_letter_modifier()
                    || c.is_symbol_modifier()
                    || c.is_symbol()
                /* Should others be included?? */
            })
            .collect();
    }
    // Use a black color as output
    let color_arg = &arguments.color;
    let output_color = (color_arg.red, color_arg.green, color_arg.blue);
    // Size of desired image
    let img_size = arguments.img_size;
    let mut base_dir = PathBuf::new();
    // Grab the base name of the font file
    let base_name = get_base_name(&arguments.font_file)?;
    // And build up the final output directory using the base
    // name.
    base_dir.push(&arguments.output_dir);
    base_dir.push(&base_name);
    // Create the directory tree
    fs::create_dir_all(base_dir.as_path())?;

    // If user provided a specific number of threads to use in the thread pool,
    // configure the global rayon thread pool appropriately.
    #[cfg(feature = "parallel")]
    if let Some(count) = arguments.number_of_threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(count)
            .build_global()
            .unwrap();
    }

    // If parallel processing is enabled, then use the parallel iterator
    #[cfg(feature = "parallel")]
    let image_paths: Vec<_> = valid_unicode_ranges
        .par_iter()
        .map(|unicode| {
            let safe = sync::Arc::clone(&font);
            create_glyph_img(&safe, *unicode, img_size, &output_color, base_dir.as_path())
        })
        .filter_map(|x| x.ok())
        .collect();
    // Otherwise, we will will just iterate through them one at a time
    #[cfg(not(feature = "parallel"))]
    let image_paths: Vec<_> = valid_unicode_ranges
        .iter()
        .map(|unicode| {
            let safe = sync::Arc::clone(&font);
            create_glyph_img(&safe, *unicode, img_size, &output_color, base_dir.as_path())
        })
        .filter_map(|x| x.ok())
        .collect();
    for image_path in image_paths {
        debug!("Created image: {:?}", image_path);
    }
    Ok(())
}
