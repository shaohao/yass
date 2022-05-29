extern crate regex;

use std::fs::File;
use std::io::{prelude::*, BufReader, Error};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;

//use regex::Regex;

#[derive(Debug)]
struct SRTSubtitle {
    num: i32,
    start_ms: i32,
    end_ms: i32,
    text: String,
}

enum SRTState {
    Number,
    Timestamp,
    Content,
}

fn ts2str(ts: i32) -> String {
    let ms = ts%1000;
    let s = ts/1000%60;
    let m = ts/1000/60%60;
    let h = ts/1000/60/60;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

fn str2ts(t: &str) -> i32 {
    let v: Vec<&str> = t.trim().split(&[':', ',']).collect();
    let h = i32::from_str(v[0]).unwrap();
    let m = i32::from_str(v[1]).unwrap();
    let s = i32::from_str(v[2]).unwrap();
    let mut ms = 0;
    if v.len() > 3 {
        ms = i32::from_str(v[3]).unwrap();
    }

    (h*3600+m*60+s)*1000+ms
}

fn main() -> Result<(), Error> {

    let args: Vec<String> = env::args().collect();

    let offset_expr = &args[1];
    let input_fn = &args[2];

    let mut content = HashMap::new();

    let input = File::open(input_fn)?;
    let reader = BufReader::new(input);

    let mut num = 0;
    let mut start_ms = 0;
    let mut end_ms = 0;
    let mut text = String::new();

    let mut state = SRTState::Number;
    for line in reader.lines() {
        match state {
            SRTState::Number => {
                let l = line?;
                if l.trim().is_empty() {
                    state = SRTState::Number;
                }
                else {
                    assert!(l.starts_with(char::is_numeric));
                    num = i32::from_str(l.trim()).unwrap();

                    state = SRTState::Timestamp;
                }
            },

            SRTState::Timestamp => {
                let l = line?;
                if l.trim().chars().nth(2).unwrap() == ':' {
                    //TODO:
                    let str = l.trim();
                    let v: Vec<&str> = str.split("-->").collect();
                    start_ms = str2ts(v[0]);
                    end_ms = str2ts(v[1]);

                    state = SRTState::Content;
                }
                else {
                    println!("Find invalid timestamp syntax!");
                }
            },

            SRTState::Content => {
                let l = line?;
                if l.len() > 0 {
                    if text.len() > 0 {
                        text.push_str("\n");
                    }
                    text.push_str(&l);
                }
                else {
                    if content.contains_key(&num) {
                        println!("Found dumplicated number: {}", num);
//                        Err(error"));
                    }

                    content.insert(num, SRTSubtitle {
                        num: num,
                        start_ms: start_ms,
                        end_ms: end_ms,
                        text: String::from(&text),
                    });

                    text.clear();

                    state = SRTState::Number;
                }
            },
        }
    }

    if text.len() > 0 {
        content.insert(num, SRTSubtitle {
            num: num,
            start_ms: start_ms,
            end_ms: end_ms,
            text: text.clone(),
        });
    }

    let mut sorted_keys = content.keys().copied().collect::<Vec<_>>();
    sorted_keys.sort();

    let old_1st_ts = content[&sorted_keys[0]].start_ms;
    let offset: i32;
    match &offset_expr[0..1] {
        "+" => { offset = str2ts(&offset_expr[1..]) *  1; }
        "-" => { offset = str2ts(&offset_expr[1..]) * -1; }
         _  => { offset = str2ts(&offset_expr) - old_1st_ts; }
    }

    for key in sorted_keys {
        let v= &content[&key];
        println!(
            "{}\n{} --> {}\n{}\n",
            v.num, ts2str(v.start_ms+offset), ts2str(v.end_ms+offset), v.text
        );
    }

    Ok(())
}

