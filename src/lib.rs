pub mod binary_data;
pub mod ds_tex;
pub mod image_util;

use image::{DynamicImage, ImageBuffer};
use napi::{
    bindgen_prelude::{Buffer, Uint8Array},
    Status,
};
use napi_derive::napi;
use texpresso::{Algorithm, COLOUR_WEIGHTS_PERCEPTUAL};

use crate::ds_tex::{DsTex, DsTexHeader, PixelFormat, Platform, TextureType};

fn anyhow_to_napi(err: anyhow::Error) -> napi::Error {
    napi::Error::new(Status::GenericFailure, format!("{}", err))
}

#[napi(object)]
pub struct CompileDstexParams {
    pub platform: Option<Platform>,
    pub pixel_format: Option<PixelFormat>,
    pub texture_type: Option<TextureType>,
    pub premultiply_alpha: Option<bool>,

    pub algorithm: Option<u8>,
    pub weigh_colour_by_alpha: Option<bool>,
}

fn to_texpresso_params(
    params: &Option<CompileDstexParams>,
) -> (DsTexHeader, Option<texpresso::Params>) {
    match params {
        Some(params) => (
            DsTexHeader::new(
                params.platform.unwrap_or(Platform::Default),
                params.pixel_format.unwrap_or(PixelFormat::Dxt5),
                params.texture_type.unwrap_or(TextureType::TwoD),
                params.premultiply_alpha,
            ),
            Some(texpresso::Params {
                algorithm: match params.algorithm.unwrap_or(3) {
                    0 => Algorithm::RangeFit, // 替换为你实际的枚举值
                    1 => Algorithm::ClusterFit,
                    2 => Algorithm::IterativeClusterFit,
                    _ => Algorithm::default(),
                },
                weights: COLOUR_WEIGHTS_PERCEPTUAL,
                weigh_colour_by_alpha: params.weigh_colour_by_alpha.unwrap_or(false),
            }),
        ),
        None => (DsTexHeader::default(), None),
    }
}

#[napi]
pub fn compile_dstex(
    width: u32,
    height: u32,
    rgba_data: Buffer,
    params: Option<CompileDstexParams>,
    generate_mipmaps: Option<bool>,
) -> napi::Result<Uint8Array> {
    let rgba_image = ImageBuffer::from_raw(width, height, rgba_data.to_vec()).unwrap();

    let dyn_image: DynamicImage = DynamicImage::ImageRgba8(rgba_image);

    let (ds_texheader, texpresso_params) = to_texpresso_params(&params);

    let ktex = DsTex::from_image(ds_texheader, &dyn_image, generate_mipmaps, texpresso_params)
        .map_err(anyhow_to_napi)?;

    Ok(Uint8Array::from(ktex.bytes.unwrap_or_default()))
}
