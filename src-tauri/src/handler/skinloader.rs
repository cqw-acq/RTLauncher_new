// src/lib.rs
use anyhow::{Context, Result};
use image::{imageops, DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage};
use std::path::Path;

pub struct AvatarGenerator;

impl AvatarGenerator {
    pub fn generate_avatar(input_path: &str, output_path: &str) -> Result<()> {
        let skin_image = image::open(input_path)
            .with_context(|| format!("无法打开皮肤文件: {}", input_path))?;

        let (operations, base_size) = match skin_image.dimensions() {
            (64, 32) => (get_classic_operations(), (128, 64)),
            (64, 64) => (get_slim_operations(), (128, 128)),
            _ => anyhow::bail!("不支持的皮肤尺寸，仅支持64x32或64x64"),
        };

        let mut canvas = ImageBuffer::from_pixel(1000, 1000, Rgba([255, 255, 255, 0]));
        let resized_skin = DynamicImage::ImageRgba8(imageops::resize(
            &skin_image,
            base_size.0,
            base_size.1,
            imageops::FilterType::Nearest,
        ));

        for operation in operations {
            process_operation(&resized_skin, &mut canvas, operation)?;
        }

        canvas.save(output_path)
            .with_context(|| format!("无法保存生成的头像到: {}", output_path))?;

        Ok(())
    }
}

fn process_operation(
    skin: &DynamicImage,
    canvas: &mut RgbaImage,
    (crop, scale, pos, mirror): ((u32, u32, u32, u32), f32, (i64, i64), bool),
) -> Result<()> {
    // 裁剪皮肤部分
    let (x, y, w, h) = (crop.0, crop.1, crop.2 - crop.0, crop.3 - crop.1);
    let mut part = skin.crop_imm(x, y, w, h);
    
    // 镜像处理
    if mirror {
        part = DynamicImage::ImageRgba8(imageops::flip_horizontal(&part.to_rgba8()));
    }

    // 缩放处理
    let new_w = (w as f32 * scale) as u32;
    let new_h = (h as f32 * scale) as u32;
    let scaled = imageops::resize(&part.to_rgba8(), new_w, new_h, imageops::FilterType::Nearest);

    // 创建带边框的图像
    let mut bordered = ImageBuffer::from_pixel(new_w + 30, new_h + 30, Rgba([0, 0, 0, 0]));
    imageops::overlay(&mut bordered, &scaled, 15, 15);

    // 生成阴影效果
    let shadow = create_shadow(&bordered, 7.0, 0.6);
    imageops::overlay(canvas, &shadow, pos.0 - 15, pos.1 - 10);

    // 粘贴实际图像
    imageops::overlay(canvas, &bordered, pos.0 - 15, pos.1 - 15);

    Ok(())
}


fn create_shadow(image: &RgbaImage, radius: f32, opacity: f32) -> RgbaImage {
    let mut shadow = image.clone();
    let color = Rgba([75, 85, 142, (255.0 * opacity) as u8]);
    
    for pixel in shadow.pixels_mut() {
        if pixel[3] > 0 {
            *pixel = color;
        }
    }
    
    imageops::blur(&shadow, radius)
}

// 皮肤贴图坐标
fn get_classic_operations() -> Vec<((u32, u32, u32, u32), f32, (i64, i64), bool)> {
    vec![
        ((8, 40, 16, 64), 8.375, (434, 751), false),
        ((8, 40, 16, 64), 8.375, (505, 751), true),
        ((86, 40, 92, 64), 8.167, (388, 561), false),
        ((86, 40, 92, 64), 8.167, (566, 561), true),
        ((40, 40, 56, 64), 8.0625, (437, 561), false),
        ((16, 16, 32, 32), 26.875, (287, 131), false),
        ((80, 16, 96, 32), 30.8125, (254, 107), false),
    ]
}

fn get_slim_operations() -> Vec<((u32, u32, u32, u32), f32, (i64, i64), bool)> {
    vec![
        ((8, 40, 16, 64), 8.375, (434, 751), false),
        ((8, 72, 16, 96), 9.375, (428, 737), false),
        ((40, 104, 48, 128), 8.375, (505, 751), false),
        ((8, 104, 16, 128), 9.375, (503, 737), false),
        ((86, 40, 92, 64), 8.167, (388, 561), false),
        ((88, 72, 94, 96), 9.5, (382, 538), false),
        ((74, 104, 80, 128), 8.167, (566, 561), false),
        ((104, 104, 110, 128), 9.5, (564, 538), false),
        ((40, 40, 56, 64), 8.0625, (437, 561), false),
        ((40, 72, 56, 96), 8.6575, (432, 555), false),
        ((16, 16, 32, 32), 26.875, (287, 131), false),
        ((80, 16, 96, 32), 30.8125, (254, 107), false),
    ]
}
