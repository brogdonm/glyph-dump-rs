use auto_args::AutoArgs;
use image::{DynamicImage, Rgba};
use log::debug;
use rustc_serialize::{self, hex::ToHex};
use rusttype::{point, Font, Glyph, Rect, Scale};
use std::{fs, sync, path::Path};
use unicode_categories::UnicodeCategories;
mod error;
use crate::error::AppError;
use rayon::prelude::*;

/// Representation of a color
#[derive(Debug, AutoArgs)]
struct Color {
    /// Red component
    pub red: u8,
    /// Green component
    pub green: u8,
    /// Blue component
    pub blue: u8,
}

/// Structure for the command line arguments.
#[derive(Debug, AutoArgs)]
struct CliArgs {
    /// Path to the font file to render
    pub font_file: String,
    /// Optional output directory for images (defaults to ./out)
    pub output_dir: Option<String>,
    /// Optional color used for output (defaults to [255, 255, 255])
    pub color: Option<Color>,
    /// Size to make the image for each glyph (defaults to 128)
    pub img_size: Option<u32>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            font_file: Default::default(),
            output_dir: Some("out".to_owned()),
            color: Some(Color {
                red: 255,
                green: 255,
                blue: 255,
            }),
            img_size: Some(128),
        }
    }
}

/// Gets the command line arguments
fn get_cli_args() -> Result<CliArgs, AppError> {
    let mut args = CliArgs::from_args();
    // We will need to fill out defaults for what was not supplied
    let default_args: CliArgs = CliArgs::default();
    // Check if we have an output directory
    if args.output_dir.is_none() {
        debug!("Output directory was not specified, using default.");
        args.output_dir = default_args.output_dir;
    }
    // Check if we need to provide a color
    if args.color.is_none() {
        args.color = default_args.color;
    }
    if args.img_size.is_none() {
        args.img_size = default_args.img_size;
    }
    Ok(args)
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

fn create_glyph_img<BD>(font: &sync::Arc<Font>, unicode: char, img_size: u32, output_color: &(u8, u8, u8), base_dir: BD) -> Result<Option<String>, AppError> 
where
    BD: AsRef<Path>,
    {
    let mut image_path = Some("".to_owned());
    // Get the glyph associated with the unicode
    let glyph = font.glyph(unicode);
    // Skip the glyph if we are dealing with .notdef
    if glyph.id().0 == 0 {
        return Ok(image_path);
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
        let hex_name = unicode.to_string().as_bytes().to_hex();
        image_path = Some(format!("{}/{}_image.png", base_dir.as_ref().to_str().unwrap(), &hex_name));
        // And save the image in our output directory
        image.save(image_path.as_ref().unwrap())?;
    }
    // Otherwise what has happened? Why couldn't we get the pixel bounding
    // box?
    else {
        return Err(AppError::FormattedMessage(format!(
            "Failed to get pixel bounding box for unicode: {}",
            &unicode
        )));
    }
    Ok(image_path)
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    env_logger::init();
    let arguments = get_cli_args()?;
    debug!("Command line arguments: {:#?}", &arguments);

    let font_data = std::fs::read(&arguments.font_file)?;
    let font = sync::Arc::new(Font::try_from_vec(font_data).ok_or_else(|| {
        AppError::FormattedMessage(format!(
            "Failed to parse data from file: {}",
            &arguments.font_file
        ))
    })?);
    // Filter down the range to valid codes for printing
    let valid_unicode_ranges: Vec<_> = ('\u{0000}'..'\u{10FFFF}').filter(|c| {
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
    // Use a black color as output
    let color_arg = arguments.color.unwrap();
    let output_color = (color_arg.red, color_arg.green, color_arg.blue);
    // Size of desired image
    let img_size = arguments.img_size.unwrap();
    let base_dir: String;
    // If we have an output directory, which we should since we
    // are setting up the arguments, but still need to check
    if let Some(output_dir) = arguments.output_dir.as_ref() {
        // Grab the base name of the font file
        let base_name = get_base_name(&arguments.font_file)?;
        // And build up the final output directory using the base
        // name.
        base_dir = format!("{}/{}", output_dir, base_name);
        // Create the directory tree
        fs::create_dir_all(&base_dir)?;
    } else {
        return Err(AppError::General("Output directory is not specified"));
    }

    let base_dir_path: &Path = Path::new(&base_dir);
    // The following can be used to control the number of threads globally for
    // benchmarking
    //let _ = rayon::ThreadPoolBuilder::new().num_threads(8).build_global().unwrap();

    let image_paths: Vec<_> = valid_unicode_ranges
        .par_iter()
        .map(|unicode| {
            create_glyph_img(&font, *unicode, img_size, &output_color, base_dir_path)
        }).filter_map(|x| x.err())
        .collect();
    for image_path in image_paths {
        debug!("Created image: {:?}", image_path);
    }
    Ok(())
}
