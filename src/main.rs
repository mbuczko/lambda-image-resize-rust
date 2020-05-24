extern crate aws_lambda_events;
extern crate image;
#[macro_use]
extern crate lambda_runtime as lambda;
extern crate log;
extern crate s3;
extern crate serde_json;

use aws_lambda_events::event::s3::{S3Event, S3EventRecord};
use image::imageops::FilterType;
use image::{GenericImageView, ImageError, ImageOutputFormat};
use lambda::error::HandlerError;
use log::info;
use s3::bucket::Bucket;
use s3::credentials::Credentials;
use s3::region::Region;
use serde_json::Value;
use std::env;
use std::error::Error;

#[derive(Debug)]
struct Thumb {
    width: f32,
    height: f32, // ignored for now
}

impl Thumb {
    fn from_env() -> Self {
        let config = env::var("THUMB").expect("No THUMB size found in env.");
        let size: Vec<f32> = config.split('x').map(|v| v.parse().unwrap()).collect();
        Thumb {
            width: *size.get(0).unwrap(),
            height: *size.get(1).unwrap(),
        }
    }
}

fn truncate(input: &str, ch: char) -> &str {
    let mut it = input.chars();
    let mut byte_end = 0;
    loop {
        if let Some(c) = it.next() {
            if c == ch {
                break;
            }
            byte_end += c.len_utf8();
        } else {
            break;
        }
    }
    &input[..byte_end]
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init_with_level(log::Level::Info)?;

    lambda!(handle_event);
    Ok(())
}

// fn main() -> Result<(), Box<dyn Error>> {
//     simple_logger::init_with_level(log::Level::Info)?;
//
//     let args: Vec<String> = env::args().collect();
//     let img = image::open(&args[1])?;
//     resize_image(&img, &Thumb::from_env()).expect("Could not resize image");
//     Ok(())
// }

fn handle_event(event: Value, ctx: lambda::Context) -> Result<(), HandlerError> {
    let s3_event: S3Event =
        serde_json::from_value(event).map_err(|e| ctx.new_error(e.to_string().as_str()))?;

    for record in s3_event.records {
        handle_record(Thumb::from_env(), record);
    }
    Ok(())
}

fn handle_record(config: Thumb, record: S3EventRecord) {
    let credentials = Credentials::default();
    let region: Region = record
        .aws_region
        .expect("Could not get region from record")
        .parse()
        .expect("Could not parse region from record");
    let bname = &record
        .s3
        .bucket
        .name
        .expect("Could not get bucket name from record");
    let bucket = Bucket::new(bname, region, credentials);
    let source = record
        .s3
        .object
        .key
        .expect("Could not get key from object record");

    info!("Fetching: {}", &source);
    if source.contains('-') {
        return;
    }
    let (data, _) = bucket
        .get(&source)
        .expect(&format!("Could not get object: {}", &source));

    let img = image::load_from_memory(&data)
        .ok()
        .expect("Opening image failed");

    let buffer = resize_image(&img, &config).expect("Could not resize image");
    let target = format!("{}-thumb.png", truncate(&source, '.'));
    let (_, code) = bucket
        .put(&target, &buffer, "image/jpeg")
        .expect(&format!("Could not upload object to :{}", &target));

    info!("Uploaded: {} with: {}", &target, &code);
}

fn resize_image(img: &image::DynamicImage, config: &Thumb) -> Result<Vec<u8>, ImageError> {
    let mut result: Vec<u8> = Vec::new();

    let old_w = img.width() as f32;
    let old_h = img.height() as f32;
    let ratio = config.width / old_w;
    let new_h = (old_h * ratio).floor();

    let scaled = img.resize(config.width as u32, new_h as u32, FilterType::Lanczos3);
    scaled.write_to(&mut result, ImageOutputFormat::Png)?;
    //scaled.save("scaled.png");
    Ok(result)
}
