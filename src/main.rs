use auto_args::AutoArgs;
use image::{DynamicImage, Rgba};
use log::{debug, warn};
use rustc_serialize::{self, hex::ToHex};
use rusttype::{point, Font, Rect, Scale};
use std::error::Error;
use std::fs;
use unicode_categories::UnicodeCategories;

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

impl Default for Color {
    fn default() -> Self {
        Self {
            red: 255,
            green: 255,
            blue: 255,
        }
    }
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
    pub img_size: Option<i32>
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            font_file: Default::default(),
            output_dir: Some("out".to_owned()),
            color: Some(Color::default()),
            img_size: Some(128)
        }
    }
}

/// Gets the command line arguments
fn get_cli_args() -> Result<CliArgs, Box<dyn Error>> {
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
impl GlyphDimensions for Rect<f32> {
    fn get_glyph_height(&self) -> u32 {
        {
            let min_y = self.min.y;
            let max_y = self.max.y;
            (min_y - max_y).ceil().abs() as u32
        }
    }

    fn get_glyph_width(&self) -> u32 {
        {
            let min_x = self.min.x;
            let max_x = self.max.x;
            (max_x - min_x).ceil() as u32
        }
    }
}

/// Gets the base name of the specified file path.
fn get_base_name(file_path: &str) -> Result<String, Box<dyn Error>> {
    // Grab the base name of the font file
    let base_name = std::path::Path::new(file_path)
        .file_name()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to get base name of file_path: {}", file_path),
            )
        })?
        .to_str()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to convert base name of file_path to &str",
            )
        })?;
    Ok(base_name.to_owned())
}

fn get_scale(font: &Font, unicode_id: char, img_size: &i32) -> Result<Scale, Box<dyn Error>> {
    let one_to_one_scaling = font.glyph(unicode_id).scaled(Scale::uniform(1.0)).exact_bounding_box().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, ""))?;
    let new_sca_h: f32 = one_to_one_scaling.max.y - one_to_one_scaling.min.y;
    let new_sca_w: f32 = one_to_one_scaling.max.x - one_to_one_scaling.min.x;
    let set_vals: Vec<f32> = vec![new_sca_w, new_sca_h];
    let max_val = set_vals.iter().max_by(|x,y| x.abs().partial_cmp(&y.abs()).unwrap()).unwrap().abs();
    let f_img_size = *img_size as f32;
    let s_factor = (f_img_size / max_val).floor();
    Ok(Scale::uniform(s_factor))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let arguments = get_cli_args()?;
    debug!("Command line arguments: {:#?}", &arguments);

    let font_data = std::fs::read(&arguments.font_file)?;
    let font = Font::try_from_vec(font_data).unwrap_or_else(|| {
        panic!(
            "Error constructing font from file: {:?}",
            &arguments.font_file
        );
    });
    // Filter down the range to valid codes for printing
    let valid_unicode_ranges = ('\u{0000}'..'\u{10FFFF}')
        .filter(|c| c.is_alphabetic() || c.is_alphanumeric() || c.is_letter_other() || c.is_symbol_other() || c.is_punctuation() || c.is_letter_modifier() || c.is_symbol_modifier() || c.is_symbol());
    // Use a black color as output
    let color_arg = arguments.color.unwrap();
    let output_color = (color_arg.red, color_arg.green, color_arg.blue);
    // Size of desired image
    let img_size = arguments.img_size.unwrap();

    for unicode in valid_unicode_ranges {
        // Get the glyph associated with the unicode
        let glyph = font.glyph(unicode);
        // Check to see if we have something other than .notdef
        if glyph.id().0 == 0 {
            continue;
        }
        let scale = get_scale(&font, unicode, &img_size)?;
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
            debug!("Glyph WxH: {}x{}", &glyph_width, &glyph_height);

            // Create a new 8-bit RGBA image
            let mut image = DynamicImage::new_rgba8(glyph_width, glyph_height).to_rgba8();
            // Draw the single pixel into the image
            positioned_glyph.draw(|x, y, v| {
                image.put_pixel(
                    x as u32,
                    y as u32,
                    Rgba([
                        output_color.0,
                        output_color.1,
                        output_color.2,
                        (v * 255.0) as u8,
                    ]),
                );
            });

            // If we have an output directory, which we should since we
            // are setting up the arguments, but still need to check
            if let Some(output_dir) = arguments.output_dir.as_ref() {
                // Grab the base name of the font file
                let base_name = get_base_name(&arguments.font_file)?;
                // And build up the final output directory using the base
                // name.
                let output_dir = &format!("{}/{}", output_dir, base_name);
                // Create the directory tree
                fs::create_dir_all(output_dir)?;
                // Create a prefix for the unicode in a hex string
                let hex_name = unicode.to_string().as_bytes().to_hex();
                // And save the image in our output directory
                image.save(format!("{}/{}_image.png", &output_dir, &hex_name))?;
            } else {
                panic!("Output directory is not specified!");
            }
        }
        // Otherwise what has happened? Why couldn't we get the pixel bounding
        // box?
        else {
            warn!("Failed to get pixel bounding box for unicode: {}", &unicode);
        }
    }
    Ok(())
}
