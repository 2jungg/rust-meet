use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
use std::error::Error;
use image::{DynamicImage, ImageBuffer, Rgb, GenericImageView};
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
    let width = NonZeroU32::new(OUTPUT_WIDTH).unwrap();
    let height = NonZeroU32::new(OUTPUT_HEIGHT).unwrap();
    
    let mut resize_alg = fr::Resizer::new(fr::ResizeAlg::Nearest);
    let mut resized_image = fr::Image::new(width, height, fr::PixelType::U8x3);
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
        resized_image.buffer().to_vec(),
    )
    .ok_or("Failed to create image buffer")?;

    Ok(to_ascii(&DynamicImage::ImageRgb8(image_buffer)))
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
