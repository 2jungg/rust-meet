use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
use std::error::Error;
use image::{DynamicImage, ImageBuffer, Rgb};
use imageproc::drawing::draw_text_mut;
use ab_glyph::{FontArc, PxScale};
use fast_image_resize as fr;
use std::num::NonZeroU32;

pub const ASCII_CHARS: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
pub const OUTPUT_WIDTH: u32 = 80;
pub const OUTPUT_HEIGHT: u32 = 40;

pub fn initialize_camera() -> Result<Camera, Box<dyn Error>> {
    let index = CameraIndex::Index(0);
    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let camera = Camera::new(index, requested)?;
    Ok(camera)
}

pub fn capture_and_process_frame(camera: &mut Camera) -> Result<String, Box<dyn Error>> {
    let frame = camera.frame()?;
    let decoded = frame.decode_image::<RgbFormat>()?;

    let original_image = DynamicImage::ImageRgb8(decoded);
    let src_image = fr::Image::from_vec_u8(
        NonZeroU32::new(original_image.width()).unwrap(),
        NonZeroU32::new(original_image.height()).unwrap(),
        original_image.to_rgb8().into_raw(),
        fr::PixelType::U8x3,
    )?;

    let mut dst_image = fr::Image::new(
        NonZeroU32::new(OUTPUT_WIDTH).unwrap(),
        NonZeroU32::new(OUTPUT_HEIGHT).unwrap(),
        fr::PixelType::U8x3,
    );

    let mut resizer = fr::Resizer::new(fr::ResizeAlg::Nearest);
    resizer.resize(&src_image.view(), &mut dst_image.view_mut())?;

    let image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_vec(
        OUTPUT_WIDTH,
        OUTPUT_HEIGHT,
        dst_image.buffer().to_vec(),
    )
    .ok_or("Failed to create image buffer")?;

    Ok(to_ascii(&DynamicImage::ImageRgb8(image_buffer)))
}

pub fn create_no_camera_frame() -> Result<String, Box<dyn Error>> {
    let mut image = ImageBuffer::from_pixel(OUTPUT_WIDTH, OUTPUT_HEIGHT, Rgb([0, 0, 0]));
    let font = FontArc::try_from_slice(include_bytes!("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"))?;

    let height = 20.0;
    let scale = PxScale {
        x: height,
        y: height,
    };

    let text = "No camera";
    let x_offset = (OUTPUT_WIDTH / 2) - 40;
    let y_offset = (OUTPUT_HEIGHT / 2) - 10;

    draw_text_mut(
        &mut image,
        Rgb([255, 255, 255]),
        x_offset as i32,
        y_offset as i32,
        scale,
        &font,
        text,
    );

    Ok(to_ascii(&DynamicImage::ImageRgb8(image)))
}

fn to_ascii(image: &DynamicImage) -> String {
    let gray_image = image.to_luma8();
    let mut ascii_art = String::new();

    for y in 0..gray_image.height() {
        for x in 0..gray_image.width() {
            let pixel = gray_image.get_pixel(x, y);
            let intensity = pixel[0] as usize;
            let char_index = (intensity * (ASCII_CHARS.len() - 1)) / 255;
            ascii_art.push(ASCII_CHARS[char_index]);
        }
        ascii_art.push('\n');
    }

    ascii_art
}
