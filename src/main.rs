use auto_args::AutoArgs;
use image::{DynamicImage, Rgba};
use log::debug;
use owned_ttf_parser::{AsFaceRef, OwnedFace};
use rustc_serialize::{self, hex::ToHex};
use rusttype::{point, Font, Scale};
use unicode_categories::UnicodeCategories;
use std::fs;

/// Representation of a color
#[derive(Debug, AutoArgs)]
struct Color {
    /// Red component
    pub red: u8,
    /// Green component
    pub green: u8,
    /// Blue component
    pub blue: u8
}

impl Default for Color {
    fn default() -> Self {
        Self { red: 255, green: 255, blue: 255 }
    }
}

/// Structure for the command line arguments.
#[derive(Debug, AutoArgs)]
struct CliArgs {
    /// Path to the font file to render
    pub font_file: String,
    /// Optional output directory for images, defaults to current working
    /// directory.
    pub output_dir: Option<String>,
    /// Optional color used for output.
    pub color: Option<Color>
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            font_file: Default::default(),
            output_dir: Some("out".to_owned()),
            color: Some(Color::default())
        }
    }
}

fn get_cli_args() -> Result<CliArgs, Box<dyn std::error::Error>> {
    let mut args = CliArgs::from_args();
    let default_args: CliArgs = CliArgs::default();
    if args.output_dir.is_none() {
        debug!("Output directory was not specified, using default.");
        args.output_dir = default_args.output_dir;
    }
    if args.color.is_none() {
        args.color = default_args.color;
    }
    Ok(args)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let arguments = get_cli_args()?;
    debug!("Command line arguments: {:#?}", &arguments);

    let font_data = std::fs::read(&arguments.font_file)?;
    let font_face = OwnedFace::from_vec(font_data, 0).unwrap();
    // TODO: Use the font_face to get number of glyphs to draw
    font_face.as_face_ref().tables().maxp.number_of_glyphs.get();
    //font_face.as_face_ref()
    let font_data = std::fs::read(&arguments.font_file)?;
    let font = Font::try_from_vec(font_data).unwrap_or_else(|| {
        panic!(
            "Error constructing font from file: {:?}",
            &arguments.font_file
        );
    });
    let valid_unicode_ranges =
        ('\u{0000}'..'\u{10FFFF}').filter(|c| c.is_alphabetic() || c.is_alphanumeric() || c.is_letter_other());
    // Use a black color as output
    let color_arg = arguments.color.unwrap();
    let output_color = (color_arg.red, color_arg.green, color_arg.blue);
    // Scale by a uniform factor
    let scale = Scale::uniform(512.0);
    // Grab the vertical metrics for the font at the specified scale
    let v_metrics = font.v_metrics(scale);

    for unicode in valid_unicode_ranges {
        let glyph = font.glyph(unicode);
        // Check to see if we have something other than .notdef
        if glyph.id().0 == 0 {
            continue;
        }
        let positioned_glyph = glyph
            .scaled(scale)
            .positioned(point(0.0, 0.0));
        debug!("Dealing with unicode: {:?}", unicode);
        debug!("\tv_metrics: {:#?}", &v_metrics);

        if let Some(bounding_box) = positioned_glyph.pixel_bounding_box() {
            debug!("\tBounding box: {:#?}", &bounding_box);
            let glyph_height = {
                let min_y = bounding_box.min.y;
                let max_y = bounding_box.max.y;
                (min_y - max_y).unsigned_abs() as u32
            };
            let glyph_width = {
                let min_x = bounding_box.min.x;
                let max_x = bounding_box.max.x;
                (max_x - min_x) as u32
            };
            debug!("Glyph WxH: {}x{}", &glyph_width, &glyph_height);
            let mut image = DynamicImage::new_rgba8(glyph_width, glyph_height).to_rgba8();
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
            if let Some(output_dir) = arguments.output_dir.as_ref() {
                let base_name = std::path::Path::new(&arguments.font_file)
                    .file_name()
                    .unwrap_or_else(|| {
                        panic!("Error getting file name");
                    })
                    .to_str()
                    .unwrap_or_else(|| {
                        panic!("Error converting to string.");
                    });
                let output_dir = &format!("{}/{}", output_dir, base_name);
                fs::create_dir_all(output_dir).or_else(|e| {
                    debug!(
                        "Could not create directory, most likely already exists: {:?}",
                        e
                    );
                    Ok::<(), Box<dyn std::error::Error>>(())
                })?;
                let hex_name = unicode.to_string().as_bytes().to_hex();
                image.save(format!("{}/{}_image.png", &output_dir, &hex_name))?;
            } else {
                panic!("Output directory is not specified!");
            }
        }
    }
    Ok(())
}
