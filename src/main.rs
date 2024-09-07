use std::io::{prelude::*, BufReader, BufWriter, Write, Error, ErrorKind};
use std::str::FromStr;
use std::io::Cursor;

use clap::{Parser};
use lazy_regex::{regex, Captures};
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Serialize};
use actix_multipart::Multipart;
use futures::{StreamExt, TryStreamExt};

fn ms2str(ts: i64, ms_marker: char) -> String {
    let ms = ts%1000;
    let s = ts/1000%60;
    let m = ts/1000/60%60;
    let h = ts/1000/60/60;
    // Format milliseconds appropriately based on marker
    if ms_marker == ',' {
        format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
    } else if ms_marker == '.' {
        // Convert milliseconds to centiseconds if the marker is '.'
        let cs = ms / 10;
        format!("{:02}:{:02}:{:02}.{:02}", h, m, s, cs)
    } else {
        // Default case, just in case ms_marker is neither ',' nor '.'
        format!("{:02}:{:02}:{:02}{}{:03}", h, m, s, ms_marker, ms)
    }
}

fn str2ms(t: &str) -> Result<i64, Error> {
    let v: Vec<&str> = t.trim().split(&[':', ',', '.']).collect();
    if v.len() < 3 {
        return Err(Error::new(ErrorKind::InvalidInput, "Invalid time format"));
    }
    let h = i64::from_str(v[0]).map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
    let m = i64::from_str(v[1]).map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
    let s = i64::from_str(v[2]).map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
    let mut ms = 0;
    if v.len() > 3 {
        let fraction = v[3];
        if fraction.len() == 3 {
            ms = i64::from_str(fraction).map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;
        } else if fraction.len() == 2 {
            ms = i64::from_str(fraction).map_err(|e| Error::new(ErrorKind::InvalidInput, e))? * 10;
        }
    }

    Ok((h*3600+m*60+s)*1000+ms)
}

fn process_srt_file<R: Read, W: Write>(
    input: &mut R,
    offset: i64,
    output: &mut BufWriter<W>,
    use_newts: bool
) -> Result<(), Error> {
    let reader = BufReader::new(input);
    let mut is_line1 = true;
    let mut offset_val = offset;

    for line in reader.lines() {
        let l = line?;
        let new_l = regex!(r"(\d+:\d+:\d+[.,]\d+)\s+-->\s+(\d+:\d+:\d+[.,]\d+)(.*)")
            .replace(&l, |caps: &Captures| {
                let start_ms:i64 = str2ms(&caps[1]).unwrap_or(0);
                let end_ms:i64   = str2ms(&caps[2]).unwrap_or(0);
                if use_newts && is_line1 {
                    offset_val = offset - start_ms;
                    is_line1 = false;
                }
                format!("{} --> {}{}",
                    ms2str(start_ms+offset_val, ','),
                    ms2str(end_ms+offset_val, ','),
                    &caps[3]
                )
            });
        writeln!(output, "{}", new_l)?;
    }

    Ok(())
}

fn process_ass_file<R: Read, W: Write>(
    input: &mut R,
    offset: i64,
    output: &mut BufWriter<W>,
    use_newts: bool
) -> Result<(), Error> {
    let reader = BufReader::new(input);
    let mut is_line1 = true;
    let mut offset_val: i64 = offset;

    for line in reader.lines() {
        let l = line?;
        let new_l = regex!(r"(Dialogue\s*:.*),(\d+:\d+:\d+.\d+),(\d+:\d+:\d+.\d+),(.*)")
            .replace(&l, |caps: &Captures| {
                let start_ms: i64 = str2ms(&caps[2]).unwrap_or(0);
                let end_ms: i64  = str2ms(&caps[3]).unwrap_or(0);
                if use_newts && is_line1 {
                    offset_val = offset - start_ms as i64;
                    is_line1 = false;
                }
                format!("{},{},{},{}",
                    &caps[1],
                    ms2str(start_ms + offset_val, '.'),
                    ms2str(end_ms   + offset_val, '.'),
                    &caps[4]
                )
            });
        writeln!(output, "{}", new_l)?;
    }

    Ok(())
}

#[derive(Serialize)]
struct SubtitleResponse {
    content: String,
}

async fn sync_subtitle(mut payload: Multipart) -> impl Responder {
    let mut offset = String::new();
    let mut file_type = String::new();
    let mut subtitle_content = Vec::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition();

        let _ = match content_disposition.get_name() {
            Some("offset") => {
                while let Some(chunk) = field.next().await {
                    let data = chunk.unwrap();
                    offset = String::from_utf8(data.to_vec()).unwrap();
                }
            }
            Some("file_type") => {
                while let Some(chunk) = field.next().await {
                    let data = chunk.unwrap();
                    file_type = String::from_utf8(data.to_vec()).unwrap();
                }
            }
            Some("subtitle_file") => {
                while let Some(chunk) = field.next().await {
                    subtitle_content.extend_from_slice(&chunk.unwrap());
                }
            }
            _ => {}
        };
    }

    if !regex!(r"[+\-]?\d+:\d+:\d+(,\d+)?").is_match(&offset) {
        return HttpResponse::BadRequest().body("Invalid offset format");
    }

    let offset_ms: i64;
    let mut use_newts = false;
    let _ = match &offset[0..1] {
        "+" => { offset_ms = str2ms(&offset[1..]).unwrap() *  1; },
        "-" => { offset_ms = str2ms(&offset[1..]).unwrap() * -1; },
         _  => { offset_ms = str2ms(&offset).unwrap(); use_newts = true; },
    };

    let mut input = Cursor::new(subtitle_content);
    let mut output = Vec::new();
    let mut writer = std::io::BufWriter::new(&mut output);

    let _ = match file_type.as_str() {
        "srt" => process_srt_file(&mut input, offset_ms, &mut writer, use_newts),
        "ass" => process_ass_file(&mut input, offset_ms, &mut writer, use_newts),
        _ => return HttpResponse::BadRequest().body("Unsupported file format"),
    };

    drop(writer);
    let result = String::from_utf8(output).unwrap();

    HttpResponse::Ok().json(SubtitleResponse { content: result })
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "127.0.0.1")]
    address: String,

    #[arg(short, long, default_value_t = 8080)]
    port: u16,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let bind_address = format!("{}:{}", args.address, args.port);

    println!("Server is starting, listening on: {}", bind_address);

    HttpServer::new(|| {
        App::new()
            .service(web::resource("/sync").route(web::post().to(sync_subtitle)))
            .service(actix_files::Files::new("/", "./static").index_file("index.html"))
    })
    .bind(bind_address)?
    .run()
    .await
}
