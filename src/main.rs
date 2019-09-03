extern crate aws_lambda_events;
extern crate image;
#[macro_use]
extern crate lambda_runtime as lambda;
#[macro_use]
extern crate log;
extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate simple_logger;
extern crate reqwest;
extern crate exif;

use image::{ImageOutputFormat, GenericImageView, ImageError};

mod config;

use config::Config;
use lambda::error::HandlerError;
use serde_json::Value;
use std::error::Error;
use aws_lambda_events::event::apigw::ApiGatewayProxyRequest;
use aws_lambda_events::event::apigw::ApiGatewayProxyResponse;
use std::collections::HashMap;
use std::io::Read;
use std::borrow::{Borrow, BorrowMut};


const SIZE_KEY: &'static str = "size";

const SOURCE_HEADER: &'static str = "source-url";
const DEST_HEADER: &'static str = "destination-url";
const MIME_HEADER: &'static str = "mime-type";

const MIME_JPEG: &'static str = "image/jpeg";
const MIME_PNG: &'static str = "image/png";


fn main() -> Result<(), Box<Error>> {
    simple_logger::init_with_level(log::Level::Info)?;

    let response = lambda!(handle_event);
    Ok(response)
}

fn handle_event(event: Value, ctx: lambda::Context) -> Result<ApiGatewayProxyResponse, HandlerError> {
    let config = Config::new();

    let api_event: ApiGatewayProxyRequest = serde_json::from_value(event).map_err(|e| ctx.new_error(e.to_string().as_str()))?;

    let source_url = api_event.headers.get(SOURCE_HEADER).unwrap_or_else(|| panic!("Missing source url"));
    let dest_url = api_event.headers.get(DEST_HEADER).unwrap_or_else(|| panic!("Missing destination url"));
    let size = api_event.query_string_parameters.get(SIZE_KEY).unwrap_or_else(|| panic!("Missing size"));

    let fallback_mime_type = MIME_JPEG.to_string();
    let mime_type = api_event.headers.get(MIME_HEADER).unwrap_or(&fallback_mime_type);

    info!("source_url: {}, dest_url: {}, size: {}", &source_url, &dest_url, &size);
    let result = handle_request(
        &config,
        source_url.to_string(),
        dest_url.to_string(),
        size.to_string(),
        mime_type.to_string(),
    );

    let response = ApiGatewayProxyResponse {
        status_code: 200,
        headers: HashMap::new(),
        multi_value_headers: HashMap::new(),
        is_base64_encoded: Option::from(false),
        body: Option::from(result),
    };

    Ok(response)
}

fn handle_request(config: &Config, source_url: String, dest_url: String, size_as_string: String, mime_type: String) -> String {
    let size = size_as_string.parse::<f32>().unwrap();

    let mut source_response = reqwest::get(source_url.as_str()).expect("Failed to download source image");
    let mut source_image_buffer = Vec::new();
    let source_size = source_response.read_to_end(&mut source_image_buffer).unwrap();
    let img = image::load_from_memory(&source_image_buffer)
        .ok()
        .expect("Opening image failed");


    // RESIZE
    info!("Will resize image");
    let mut processed_image = resize_image(&img, &size, mime_type.clone()).expect("Could not resize image");

    // READ EXIF
    if mime_type.eq(MIME_JPEG) {
        info!("Will apply EXIF rotation");
        let exif_reader = exif::Reader::new(&mut std::io::BufReader::new(source_image_buffer.as_slice()));
        if exif_reader.is_ok() {
            if let Some(field) = exif_reader.unwrap().get_field(exif::Tag::Orientation, false).and_then(|f| f.value.get_uint(0)) {
                processed_image = rotate_image(&processed_image, field).expect("Could not rotate image");
            }
        } else {
            error!("{}", format!("Could not rotate image {}", exif_reader.err().unwrap().to_string()));
        }
    }

    let response = write_file_to_dest_url(dest_url, mime_type.clone(), &mut processed_image);
    return "OK".to_string();
}

fn write_file_to_dest_url(dest_url: String, mime_type: String, processed_image: &mut image::DynamicImage) -> reqwest::Response {
    let mut result: Vec<u8> = Vec::new();
    processed_image.write_to(&mut result, get_image_format(mime_type));
    let client = reqwest::Client::new();
    let response = client.put(dest_url.as_str()).body(result).send().unwrap_or_else(|_| panic!("Failed to upload to destination"));
    response
}

fn resize_image(img: &image::DynamicImage, new_w: &f32, mime_type: String) -> Result<image::DynamicImage, ImageError> {
    let old_w = img.width() as f32;
    let old_h = img.height() as f32;
    let ratio = new_w / old_w;
    let new_h = (old_h * ratio).floor();

    let scaled_image = img.resize(*new_w as u32, new_h as u32, image::FilterType::Lanczos3);
    Ok(scaled_image)
}

fn rotate_image(img: &image::DynamicImage, orientation: u32) -> Result<image::DynamicImage, ImageError> {
    match orientation {
        2 => Ok(img.fliph()),
        3 => Ok(img.rotate180()),
        4 => Ok(img.flipv()),
        5 => {
            let rotated = img.fliph();
            Ok(rotated.rotate270())
        }
        6 => Ok(img.rotate90()),
        7 => {
            let rotated = img.rotate270();
            Ok(rotated.fliph())
        }
        8 => Ok(img.rotate270()),
        _ => Ok(img.clone())
    }
}

fn get_image_format(mime_type: String) -> ImageOutputFormat {
    match &mime_type[..] {
        MIME_JPEG => ImageOutputFormat::JPEG(90),
        MIME_PNG => ImageOutputFormat::PNG,
        _ => ImageOutputFormat::JPEG(90)
    }
}
