use std::path::Path;
use std::fs::File;
use std::io::{prelude::*, stdout, BufReader, BufWriter, Write, Error};
use std::str::FromStr;

use clap::{CommandFactory, ErrorKind, Parser};
use lazy_regex::{regex, Captures};


fn ts2str(ts: i32, ms_marker: char) -> String {
    let ms = ts%1000;
    let s = ts/1000%60;
    let m = ts/1000/60%60;
    let h = ts/1000/60/60;
    format!("{:02}:{:02}:{:02}{}{:03}", h, m, s, ms_marker, ms)
}

fn str2ts(t: &str) -> i32 {
    let v: Vec<&str> = t.trim().split(&[':', ',', '.']).collect();
    let h = i32::from_str(v[0]).unwrap();
    let m = i32::from_str(v[1]).unwrap();
    let s = i32::from_str(v[2]).unwrap();
    let mut ms = 0;
    if v.len() > 3 {
        ms = i32::from_str(v[3]).unwrap();
    }

    (h*3600+m*60+s)*1000+ms
}

fn process_srt_file(input: &File, offset: i32, output: &mut BufWriter< Box<dyn Write> >, use_newts: bool) -> Result<(), Error> {
    let reader = BufReader::new(input);
    let mut is_line1 = true;
    let mut offset_val = offset;

    for line in reader.lines() {
        let l = line?;
        let new_l = regex!(r"(\d+:\d+:\d+[.,]\d+)\s+-->\s+(\d+:\d+:\d+[.,]\d+)(.*)")
            .replace(&l, |caps: &Captures| {
                let start_ts = str2ts(&caps[1]);
                let end_ts   = str2ts(&caps[2]);
                if use_newts && is_line1 {
                    offset_val = offset - start_ts;
                    is_line1 = false;
                }
                format!("{} --> {}{}",
                    ts2str(start_ts+offset_val, ','),
                    ts2str(end_ts+offset_val, ','),
                    &caps[3]
                )
            });
        writeln!(output, "{}", new_l)?;
    }

    Ok(())
}

fn process_ass_file(input: &File, offset: i32, output: &mut BufWriter< Box<dyn Write> >, use_newts: bool) -> Result<(), Error> {
    let reader = BufReader::new(input);
    let mut is_line1 = true;
    let mut offset_val = offset;

    for line in reader.lines() {
        let l = line?;
        let new_l = regex!(r"(Dialogue\s*:.*),(\d+:\d+:\d+.\d+),(\d+:\d+:\d+.\d+),(.*)")
            .replace(&l, |caps: &Captures| {
                let start_ts = str2ts(&caps[2]);
                let end_ts   = str2ts(&caps[3]);
                if use_newts && is_line1 {
                    offset_val = offset - start_ts;
                    is_line1 = false;
                }
                format!("{},{},{},{}",
                    &caps[1],
                    ts2str(start_ts+offset_val, '.'),
                    ts2str(end_ts+offset_val, '.'),
                    &caps[4]
                )
            });
        writeln!(output, "{}", new_l)?;
    }

    Ok(())
}

#[derive(Parser)]
#[clap(name = "YASS")]
#[clap(bin_name = "yass")]
#[clap(author = "Shao Hao. <shaohao@outlook.com>")]
#[clap(version = "1.0")]
#[clap(about = "Yet Another Subtitle Sync", long_about = None)]
struct Cli {
    /// Use +hh:mm:ss,ms/-hh:mm:ss,ms to adjust the offset.
    /// Use hh:mm:ss,ms to fix the first dialogue and adjust the remains.
    #[clap(allow_hyphen_values(true))]
    offset: String,

    #[clap(parse(from_os_str))]
    /// Subtitle file. Only support .srt or .ass format.
    input_file: std::path::PathBuf,

    #[clap(short, parse(from_os_str))]
    /// Output file (Optional, leave blank means to stdout)
    output_file: Option<std::path::PathBuf>,
}


fn main() -> Result<(), Error> {

    let cli = Cli::parse();

    // Check input args
    let offset_expr = cli.offset;
    if ! regex!(r"[+\-]?\d+:\d+:\d+(,\d+)?").is_match(&offset_expr) {
        let mut cmd = Cli::command();
        cmd.error(
            ErrorKind::InvalidValue,
            "The valid offset format: +01:02:03, -01:02:03,04 or 01:02:03,04",
        ).exit();
    }
    let input_path: &Path = cli.input_file.as_path();
    if ! input_path.is_file() {
        let mut cmd = Cli::command();
        cmd.error(
            ErrorKind::Io,
            format!("'{}' is NOT a vaild filename", input_path.to_str().unwrap()),
        ).exit();
    }

    let mut output = BufWriter::new(match cli.output_file {
        Some(x) => Box::new(File::create(x).unwrap()) as Box<dyn Write>,
        None    => Box::new(stdout()) as Box<dyn Write>,
    });

    // Calculate the offset
    let offset: i32;
    let mut use_newts = false;
    match &offset_expr[0..1] {
        "+" => { offset = str2ts(&offset_expr[1..]) *  1; },
        "-" => { offset = str2ts(&offset_expr[1..]) * -1; },
         _  => { offset = str2ts(&offset_expr); use_newts = true; },
    }

    // Prepare/Open the input file
    let input_fn = input_path.to_str().unwrap();
    let input = File::open(input_fn)?;

    // Process the sync based on file format
    let ext = input_path.extension().unwrap().to_str().unwrap();
    match ext {
        "srt" => { process_srt_file(&input, offset, &mut output, use_newts).expect("Error processing SRT file."); },
        "ass" => { process_ass_file(&input, offset, &mut output, use_newts).expect("Error processing ASS file."); },
          _   => { println!("Unsupported file format: .{}", ext); },
    }

    Ok(())
}

