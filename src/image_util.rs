use fast_image_resize::{Resizer, images::Image};
use rayon::prelude::*;

// pub fn flip_vertical1(rgba_data: &[u8], width: usize, height: usize) -> Vec<u8> {
//     let row_bytes = width * 4;
//     assert_eq!(rgba_data.len(), row_bytes * height);

//     let mut result = vec![0u8; rgba_data.len()];

//     result
//         .par_chunks_mut(row_bytes)
//         .enumerate()
//         .for_each(|(y, row)| {
//             let src_y = height - 1 - y;
//             let src_row = &rgba_data[src_y * row_bytes..(src_y + 1) * row_bytes];
//             row.copy_from_slice(src_row);
//         });
//     result
// }

pub fn flip_vertical(rgba_data: &[u8], width: usize, height: usize) -> Vec<u8> {
    let row_bytes = width * 4;
    assert_eq!(rgba_data.len(), row_bytes * height);

    let mut result = vec![0u8; rgba_data.len()];

    for y in 0..height {
        let src_y = height - 1 - y;
        let src_row = &rgba_data[src_y * row_bytes..(src_y + 1) * row_bytes];
        let dst_row = &mut result[y * row_bytes..(y + 1) * row_bytes];
        dst_row.copy_from_slice(src_row);
    }
    result
}

pub fn flip_vertical_and_prepremultiply_alpha(
    rgba_data: &[u8],
    width: usize,
    height: usize,
) -> Vec<u8> {
    let row_bytes = width * 4;
    assert_eq!(rgba_data.len(), row_bytes * height);

    let mut result = vec![0u8; rgba_data.len()];

    result
        .par_chunks_mut(row_bytes)
        .enumerate()
        .for_each(|(y, row)| {
            let src_y = height - 1 - y;
            let src_row = &rgba_data[src_y * row_bytes..(src_y + 1) * row_bytes];

            for (i, chunk) in src_row.chunks(4).enumerate() {
                let r = chunk[0] as f32;
                let g = chunk[1] as f32;
                let b = chunk[2] as f32;
                let a = chunk[3];
                let alpha = a as f32 / 255.0;

                row[i * 4] = (r * alpha) as u8;
                row[i * 4 + 1] = (g * alpha) as u8;
                row[i * 4 + 2] = (b * alpha) as u8;
                row[i * 4 + 3] = a;
            }
        });

    result
}

pub fn flip_vertical_and_unprepremultiply_alpha(
    rgba_data: &[u8],
    width: usize,
    height: usize,
) -> Vec<u8> {
    let row_bytes = width * 4;
    assert_eq!(rgba_data.len(), row_bytes * height);

    let mut result = vec![0u8; rgba_data.len()];

    result
        .par_chunks_mut(row_bytes)
        .enumerate()
        .for_each(|(y, row)| {
            let src_y = height - 1 - y;
            let src_row = &rgba_data[src_y * row_bytes..(src_y + 1) * row_bytes];

            for (i, chunk) in src_row.chunks(4).enumerate() {
                let r = chunk[0] as f32;
                let g = chunk[1] as f32;
                let b = chunk[2] as f32;
                let a = chunk[3];
                let alpha = a as f32 / 255.0;

                if a == 0 {
                    row[i * 4] = 0;
                    row[i * 4 + 1] = 0;
                    row[i * 4 + 2] = 0;
                    row[i * 4 + 3] = 0;
                } else {
                    row[i * 4] = (r / alpha) as u8;
                    row[i * 4 + 1] = (g / alpha) as u8;
                    row[i * 4 + 2] = (b / alpha) as u8;
                    row[i * 4 + 3] = a;
                }
            }
        });

    result
}

pub fn prepremultiply_alpha(rgba_data: &[u8]) -> Vec<u8> {
    assert!(rgba_data.len() % 4 == 0, "Input data is not valid RGBA");

    let mut premultiplied_data = vec![0u8; rgba_data.len()];
    premultiplied_data
        .par_chunks_mut(4)
        .zip(rgba_data.par_chunks(4))
        .for_each(|(dst_pixel, src_pixel)| {
            let a = src_pixel[3];
            if a == 0 {
                dst_pixel.copy_from_slice(&[0, 0, 0, 0]);
            } else {
                let alpha = a as f32 / 255.0;

                dst_pixel[0] = (src_pixel[0] as f32 * alpha) as u8;
                dst_pixel[1] = (src_pixel[1] as f32 * alpha) as u8;
                dst_pixel[2] = (src_pixel[2] as f32 * alpha) as u8;
                dst_pixel[3] = a;
            }
        });
    premultiplied_data
}

pub fn resize_image<'a>(
    image: &'a Image<'a>,
    resize_width: u32,
    resize_height: u32,
) -> anyhow::Result<Image<'a>> {
    let mut resizer = Resizer::new();
    let mut resized = Image::new(resize_width, resize_height, image.pixel_type());

    resizer.resize(image, &mut resized, None)?;
    Ok(resized)
}
