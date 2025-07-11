use crate::{
    binary_data::read_string,
    image_util::{
        flip_vertical, flip_vertical_and_unprepremultiply_alpha, prepremultiply_alpha, resize_image,
    },
};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use fast_image_resize::{PixelType, images::Image};
use image::DynamicImage;
use napi_derive::napi;
use num_enum::TryFromPrimitive;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    cmp::max,
    io::{Cursor, Error, ErrorKind, Read, Write},
};

#[derive(TryFromPrimitive, Serialize, Deserialize, Debug)]
#[repr(u32)]
#[napi]
pub enum Platform {
    Default = 0, // unknown
    Pc = 12,
    Ps3 = 10,
    Xbox360 = 11,
}

#[derive(TryFromPrimitive, Serialize, Deserialize, Debug)]
#[repr(u32)]
#[napi]
pub enum PixelFormat {
    Dxt1 = 0, // BC1
    Dxt3 = 1, // BC2
    Dxt5 = 2, // BC3
    Rgba = 4,
    Rgb = 5,
    Unknown = 7,
}

#[derive(TryFromPrimitive, Serialize, Deserialize, Debug)]
#[repr(u32)]
#[napi]
pub enum TextureType {
    OneD = 0,
    TwoD = 1,
    ThreeD = 2,
    CubeMapped = 3,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Specification {
    max_platform: u8,
    max_pixel_format: u8,
    max_texture_type: u8,
    max_mipmap_count: u8,
    max_flag: u8,
    max_fill: u32,
    offset_platform: u8,
    offset_pixel_format: u8,
    offset_texture_type: u8,
    offset_mipmap_count: u8,
    offset_flag: u8,
    offset_fill: u8,
}

pub const PRE_CAVE_SPECIFICATION: Specification = Specification {
    max_platform: 7,      // 2 ** 3 - 1
    max_pixel_format: 7,  // 2 ** 3 - 1
    max_texture_type: 7,  // 2 ** 3 - 1
    max_mipmap_count: 15, // 2 ** 4 - 1
    max_flag: 1,          // 2 ** 1 - 1
    max_fill: 262143,     // 2 ** 18 - 1
    offset_platform: 0,
    offset_pixel_format: 3,
    offset_texture_type: 6,
    offset_mipmap_count: 9,
    offset_flag: 13,
    offset_fill: 14,
};

pub const POST_CAVE_SPECIFICATION: Specification = Specification {
    max_platform: 15,     // 2 ** 4 - 1
    max_pixel_format: 31, // 2 ** 5 - 1
    max_texture_type: 15, // 2 ** 4 - 1
    max_mipmap_count: 31, // 2 ** 5 - 1
    max_flag: 3,          // 2 ** 2 - 1
    max_fill: 4095,       // 2 ** 12 - 1
    offset_platform: 0,
    offset_pixel_format: 4,
    offset_texture_type: 9,
    offset_mipmap_count: 13,
    offset_flag: 18,
    offset_fill: 20,
};

#[derive(Serialize, Deserialize, Debug)]
pub struct DsTexHeader {
    mipmap_count: u8,
    specification: Specification,
    platform: Platform,
    pixel_format: PixelFormat,
    texture_type: TextureType,
    flag: u8,
    fill: u32,
    premultiply_alpha: Option<bool>,
}

impl DsTexHeader {
    pub fn new(
        platform: Platform,
        pixel_format: PixelFormat,
        texture_type: TextureType,
        premultiply_alpha: Option<bool>,
    ) -> DsTexHeader {
        return DsTexHeader {
            platform,
            pixel_format,
            texture_type,
            mipmap_count: 0,
            premultiply_alpha,
            specification: POST_CAVE_SPECIFICATION,
            flag: POST_CAVE_SPECIFICATION.max_flag,
            fill: POST_CAVE_SPECIFICATION.max_fill,
        };
    }

    pub fn has_alpha(pixel_format: PixelFormat) -> bool {
        match pixel_format {
            PixelFormat::Rgba | PixelFormat::Dxt3 | PixelFormat::Dxt5 => true,
            _ => false,
        }
    }

    pub fn default() -> DsTexHeader {
        return DsTexHeader {
            specification: POST_CAVE_SPECIFICATION,
            platform: Platform::Default,
            pixel_format: PixelFormat::Dxt5,
            texture_type: TextureType::TwoD,
            mipmap_count: 0,
            flag: POST_CAVE_SPECIFICATION.max_flag,
            fill: POST_CAVE_SPECIFICATION.max_fill,
            premultiply_alpha: Some(true),
        };
    }

    /*
        This test has a false positive (for pre-caves update) if the input TEX is of the post-caves update variety,
        has both flags set to high, and has at least 30 mipmaps. This is considered unlikely enough to be reasonable
        (as it would likely result from an image with an initial size of 73,728 x 73,728) since there is no other way to check
        by oblivioncth：https://github.com/oblivioncth/Stexatlaser
    */
    pub fn from_data(data: u32) -> anyhow::Result<DsTexHeader> {
        let specification = match data >> 14 & 0x3ffff {
            0x3ffff => PRE_CAVE_SPECIFICATION,
            _ => POST_CAVE_SPECIFICATION,
        };
        let max_platform = specification.max_platform as u32;
        let max_pixel_format = specification.max_pixel_format as u32;
        let max_texture_type = specification.max_texture_type as u32;
        let max_mipmap_count = specification.max_mipmap_count as u32;
        let max_flag = specification.max_flag as u32;
        let max_fill = specification.max_fill;

        let platform = Platform::try_from(data >> specification.offset_platform & max_platform)?;
        let pixel_format =
            PixelFormat::try_from(data >> specification.offset_pixel_format & max_pixel_format)?;
        let texture_type =
            TextureType::try_from(data >> specification.offset_texture_type & max_texture_type)?;
        let mipmap_count =
            u8::try_from(data >> specification.offset_mipmap_count & max_mipmap_count)?;
        let flag = u8::try_from(data >> specification.offset_flag & max_flag)?;
        let fill = data >> specification.offset_fill & max_fill;

        return Ok(DsTexHeader {
            specification,
            platform,
            pixel_format,
            texture_type,
            mipmap_count,
            flag,
            fill,
            premultiply_alpha: Some(DsTexHeader::has_alpha(pixel_format)),
        });
    }

    pub fn to_data(&self) -> anyhow::Result<u32> {
        let platform = self.platform as u64;
        let pixel_format = self.pixel_format as u64;
        let texture_type = self.texture_type as u64;
        let mipmap_count = self.mipmap_count as u64;
        let flag = self.flag as u64;
        let fill = self.fill as u64;
        let offset_platform = self.specification.offset_platform;
        let offset_pixel_format = self.specification.offset_pixel_format;
        let offset_texture_type = self.specification.offset_texture_type;
        let offset_mipmap_count = self.specification.offset_mipmap_count;
        let offset_flag = self.specification.offset_flag;
        let offset_fill = self.specification.offset_fill;

        let data: u32 = (platform << offset_platform
            | pixel_format << offset_pixel_format
            | texture_type << offset_texture_type
            | mipmap_count << offset_mipmap_count
            | flag << offset_flag
            | fill << offset_fill)
            .try_into()?;

        Ok(data)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mipmap {
    width: u16,
    height: u16,
    pitch: u16,
    data_size: u32,
    data: Vec<u8>,
}

impl Mipmap {
    pub fn decompress(
        &self,
        pixel_format: PixelFormat,
        premultiply_alpha: bool,
    ) -> anyhow::Result<Vec<u8>> {
        let data = &self.data;
        let width = self.width;
        let height = self.height;

        let rgba_data = match pixel_format {
            PixelFormat::Dxt1 | PixelFormat::Dxt3 | PixelFormat::Dxt5 => {
                let format = match pixel_format {
                    PixelFormat::Dxt1 => texpresso::Format::Bc1,
                    PixelFormat::Dxt3 => texpresso::Format::Bc2,
                    PixelFormat::Dxt5 => texpresso::Format::Bc3,
                    _ => unreachable!(),
                };

                let mut output: Vec<u8> = vec![0; (width as usize) * (height as usize) * 4];
                format.decompress(data, width as usize, height as usize, &mut output);

                match premultiply_alpha {
                    true => flip_vertical_and_unprepremultiply_alpha(
                        &output,
                        width as usize,
                        height as usize,
                    ),
                    false => flip_vertical(&output, width as usize, height as usize),
                }
            }
            PixelFormat::Rgba => data.clone(),
            PixelFormat::Rgb => {
                assert!(data.len() % 3 == 0, "RGB data must be divisible by 3");
                let mut rgba_data = Vec::with_capacity(data.len() * 4 / 3);
                for chunk in data.chunks_exact(3) {
                    let (r, g, b) = (chunk[0], chunk[1], chunk[2]);
                    rgba_data.extend_from_slice(&[r, g, b, 255]);
                }
                rgba_data
            }
            _ => {
                return Err(
                    Error::new(ErrorKind::InvalidData, "not supported format ktex file").into(),
                );
            }
        };
        Ok(rgba_data)
    }

    pub fn compress(
        pixel_format: PixelFormat,
        width: u16,
        height: u16,
        rgba_data: &[u8],
        premultiply_alpha: bool,
        parmas: texpresso::Params,
    ) -> anyhow::Result<Mipmap> {
        let (pitch, data) = match pixel_format {
            PixelFormat::Dxt1 | PixelFormat::Dxt3 | PixelFormat::Dxt5 => {
                let (pitch, format) = match pixel_format {
                    PixelFormat::Dxt1 => ((width + 3 / 4) * 8, texpresso::Format::Bc1),
                    PixelFormat::Dxt3 => ((width + 3) / 4 * 16, texpresso::Format::Bc2),
                    PixelFormat::Dxt5 => ((width + 3) / 4 * 16, texpresso::Format::Bc3),
                    _ => unreachable!(),
                };
                let mut data = vec![0u8; format.compressed_size(width as usize, height as usize)];
                let premultiplied_data = match premultiply_alpha {
                    true => &prepremultiply_alpha(rgba_data),
                    false => rgba_data,
                };
                format.compress(
                    premultiplied_data,
                    width as usize,
                    height as usize,
                    parmas,
                    &mut data,
                );
                (pitch, data)
            }
            PixelFormat::Rgba => (width * 4, rgba_data.to_vec()),
            PixelFormat::Rgb => (width * 3, rgba_data.to_vec()),
            _ => {
                return Err(
                    Error::new(ErrorKind::InvalidData, "not supported format ktex file").into(),
                );
            }
        };

        return Ok(Mipmap {
            width,
            height,
            pitch,
            data_size: data.len().try_into()?,
            data,
        });
    }

    pub fn general_mipmaps(
        max_count: u8,
        image: &Image,
        pixel_format: PixelFormat,
        premultiply_alpha: bool,
        parmas: texpresso::Params,
    ) -> anyhow::Result<Vec<Mipmap>> {
        let mut mipmap_width: u16 = image.width().try_into()?;
        let mut mipmap_height: u16 = image.height().try_into()?;
        let mut mipmap_params = Vec::new();
        for _ in 2..=max_count {
            mipmap_width = max(1, mipmap_width / 2);
            mipmap_height = max(1, mipmap_height / 2);
            mipmap_params.push((mipmap_width, mipmap_height));

            if mipmap_width <= 1 && mipmap_height <= 1 {
                break;
            }
        }
        let mipmaps: Vec<Mipmap> = mipmap_params
            .into_par_iter()
            .map(|(width, height)| {
                let resized = resize_image(image, width as u32, height as u32)?;
                let compressed = Mipmap::compress(
                    pixel_format,
                    width,
                    height,
                    &resized.buffer(),
                    premultiply_alpha,
                    parmas,
                )?;
                Ok(compressed)
            })
            .collect::<Result<Vec<Mipmap>, anyhow::Error>>()?;

        Ok(mipmaps)
    }

    // pub fn general_mipmaps(
    //     max_count: u8,
    //     image: &Image,
    //     pixel_format: PixelFormat,
    //     premultiply_alpha: bool,
    //     parmas: texpresso::Params,
    // ) -> anyhow::Result<Vec<Mipmap>> {
    //     let (width, height) = (image.width(), image.height());

    //     let mut mipmap_width: u16 = width.try_into()?;
    //     let mut mipmap_height: u16 = height.try_into()?;
    //     let mut mipmaps = Vec::new();
    //     for _ in 2..=max_count {
    //         mipmap_width = max(1, mipmap_width / 2);
    //         mipmap_height = max(1, mipmap_height / 2);

    //         let resized = resize_image(image, mipmap_width as u32, mipmap_height as u32)?;
    //         let mipmap = Mipmap::compress(
    //             pixel_format,
    //             mipmap_width,
    //             mipmap_height,
    //             &resized.buffer(),
    //             premultiply_alpha,
    //             parmas,
    //         )?;
    //         mipmaps.push(mipmap);
    //         if mipmap_width <= 1 && mipmap_height <= 1 {
    //             break;
    //         }
    //     }

    //     Ok(mipmaps)
    // }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DsTex {
    pub header: DsTexHeader,
    pub mipmaps: Vec<Mipmap>,
    pub bytes: Option<Vec<u8>>,
}

impl DsTex {
    const MAGIC: &str = "KTEX";

    pub fn default() -> DsTex {
        return DsTex {
            header: DsTexHeader::default(),
            mipmaps: Vec::new(),
            bytes: None,
        };
    }

    pub fn read(bytes: Vec<u8>) -> anyhow::Result<DsTex> {
        let mut reader = Cursor::new(bytes.clone());
        let magic = read_string(&mut reader, 4)?;
        if magic != DsTex::MAGIC {
            return Err(Error::new(ErrorKind::InvalidData, "File is not a DsTex file.").into());
        }
        let mut header = DsTexHeader::from_data(reader.read_u32::<LittleEndian>()?)?;
        let mut mipmaps: Vec<Mipmap> = Vec::new();

        for _ in 0..header.mipmap_count {
            let width = reader.read_u16::<LittleEndian>()?;
            let height = reader.read_u16::<LittleEndian>()?;
            let pitch = reader.read_u16::<LittleEndian>()?;
            let data_size = reader.read_u32::<LittleEndian>()?;

            mipmaps.push(Mipmap {
                width,
                height,
                pitch,
                data_size,
                data: Vec::new(),
            });
        }

        for mipmap in &mut mipmaps {
            let mut data = vec![0; mipmap.data_size as usize];
            reader.read_exact(&mut data)?;
            mipmap.data = data;
        }
        let remaining_bytes = reader.get_ref().len() - reader.position() as usize;
        if remaining_bytes == 1 {
            header.premultiply_alpha = Some(reader.read_u8()? == 1)
        };

        Ok(DsTex {
            header,
            mipmaps,
            bytes: Some(bytes),
        })
    }

    pub fn to_image(&self) -> anyhow::Result<Image<'_>> {
        let mipmap = &self.mipmaps[0];
        let rgba_data = mipmap.decompress(
            self.header.pixel_format,
            self.header.premultiply_alpha.unwrap_or(true),
        )?;

        Ok(Image::from_vec_u8(
            mipmap.width as u32,
            mipmap.height as u32,
            rgba_data,
            PixelType::U8x4,
        )?)
    }

    pub fn from_image(
        ds_header: DsTexHeader,
        image: &DynamicImage,
        generate_mipmaps: Option<bool>,
        parmas: Option<texpresso::Params>,
    ) -> anyhow::Result<DsTex> {
        let mut ds_tex = DsTex {
            header: ds_header,
            mipmaps: Vec::new(),
            bytes: None,
        };

        let parmas = parmas.unwrap_or_default();
        let premultiply_alpha = ds_tex.header.premultiply_alpha.unwrap_or(true)
            && match ds_tex.header.pixel_format {
                PixelFormat::Rgba | PixelFormat::Dxt3 | PixelFormat::Dxt5 => true,
                _ => false,
            };

        let (width, height) = (image.width(), image.height());
        let fliped = Image::from_vec_u8(
            width,
            height,
            flip_vertical(&image.as_bytes(), width as usize, height as usize),
            PixelType::U8x4,
        )?;

        ds_tex.mipmaps.push(Mipmap::compress(
            ds_tex.header.pixel_format,
            width.try_into()?,
            height.try_into()?,
            &fliped.buffer(),
            premultiply_alpha,
            parmas,
        )?);

        let generate_mipmaps = generate_mipmaps.unwrap_or(true);
        if generate_mipmaps {
            let mipmaps = Mipmap::general_mipmaps(
                ds_tex.header.specification.max_mipmap_count,
                &fliped,
                ds_tex.header.pixel_format,
                premultiply_alpha,
                parmas,
            )?;
            ds_tex.mipmaps.extend(mipmaps);
        }
        ds_tex.header.mipmap_count = ds_tex.mipmaps.len().try_into()?;

        let mut bytes = Vec::<u8>::new();
        let mut writer = Cursor::new(&mut bytes);
        writer.write_all(DsTex::MAGIC.as_bytes())?;
        writer.write_u32::<LittleEndian>(ds_tex.header.to_data()?)?;

        // write mipmap metaData
        for mipmap in &ds_tex.mipmaps {
            writer.write_u16::<LittleEndian>(mipmap.width)?;
            writer.write_u16::<LittleEndian>(mipmap.height)?;
            writer.write_u16::<LittleEndian>(mipmap.pitch)?;
            writer.write_u32::<LittleEndian>(mipmap.data_size)?;
        }

        // write mipmap blockData
        for mipmap in &ds_tex.mipmaps {
            writer.write_all(&mipmap.data)?;
        }

        // write preMultiplyAlpha info
        writer.write_u8(premultiply_alpha as u8)?;

        ds_tex.bytes = Some(bytes);

        return Ok(ds_tex);
    }
}
