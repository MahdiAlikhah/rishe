//! `rishe` command-line interface.

use std::io::Read;
use std::process::ExitCode;

use rishe::{dates, Pipeline};

const USAGE: &str = "\
rishe - Persian NLP from the command line

Usage:
  rishe <command> [options] [text]

Commands:
  normalize   Normalize Persian text.
  tokenize    Tokenize Persian text.
  lemmatize   Lemmatize Persian text.
  mask        Mask or remove sensitive tokens.
  date        Parse a Jalali date and print the Gregorian equivalent.
  pos         POS tagging with a model pack (needs the `models` feature).
  ner         Named-entity recognition with a model pack (needs `models`).
  ezafe       Ezafe tagging with a model pack (needs `models`).
  depparse    Dependency parsing with a model pack (needs `models`).

Options:
  --json              Emit JSON.
  --stages a,b        Maskers to apply, in order.
  --mask-token TEXT   Override every mask default.
  --pack DIR          Model-pack directory.
  -h, --help          Show this help.
  -V, --version       Show the version.

Text is read from stdin when omitted or when it is `-`.
";

const DEFAULT_MASK_STAGES: [&str; 10] = [
    "url",
    "email",
    "mention",
    "hashtag",
    "emoji",
    "html",
    "non_persian",
    "digits",
    "diacritics",
    "punctuation",
];

struct Options {
    json: bool,
    stages: Vec<String>,
    mask_token: Option<String>,
    pack: Option<String>,
    text: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        print!("{USAGE}");
        return Ok(());
    };
    match command.as_str() {
        "-h" | "--help" => {
            print!("{USAGE}");
            return Ok(());
        }
        "-V" | "--version" => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        _ => {}
    }

    let mut options = Options {
        json: false,
        stages: DEFAULT_MASK_STAGES.iter().map(|s| s.to_string()).collect(),
        mask_token: None,
        pack: None,
        text: Vec::new(),
    };
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => options.json = true,
            "--stages" => {
                let value = args.next().ok_or("--stages needs a value")?;
                options.stages = value
                    .split(',')
                    .filter(|stage| !stage.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            "--mask-token" => {
                options.mask_token = Some(args.next().ok_or("--mask-token needs a value")?);
            }
            "--pack" => options.pack = Some(args.next().ok_or("--pack needs a value")?),
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            flag if flag.starts_with('-') && flag != "-" => {
                return Err(format!("unknown option: {flag}"));
            }
            text => options.text.push(text.to_owned()),
        }
    }

    let text = read_text(&options.text)?;
    let pipe = Pipeline::create(true, true, true);
    match command.as_str() {
        "normalize" => {
            let normalized = pipe.normalize_stages(&text, &[])?;
            emit(&normalized, options.json);
        }
        "tokenize" => emit_list(&pipe.word_tokenize(&text), options.json),
        "lemmatize" => emit_list(&pipe.process(&text).lemmas, options.json),
        "mask" => {
            let masked = pipe.mask_stages(&text, &options.stages, options.mask_token.as_deref())?;
            emit(&masked, options.json);
        }
        "date" => emit_date(&text, options.json)?,
        "pos" | "ner" | "ezafe" | "depparse" => {
            pack_tag(&command, options.pack.as_deref(), &text, options.json)?
        }
        other => return Err(format!("unknown command: {other}\n\n{USAGE}")),
    }
    Ok(())
}

fn read_text(parts: &[String]) -> Result<String, String> {
    if parts.is_empty() || (parts.len() == 1 && parts[0] == "-") {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("cannot read stdin: {error}"))?;
        Ok(text.trim().to_owned())
    } else {
        Ok(parts.join(" "))
    }
}

fn emit(value: &str, json: bool) {
    if json {
        println!("{}", json_string(value));
    } else {
        println!("{value}");
    }
}

fn emit_list(values: &[String], json: bool) {
    if json {
        println!("{}", json_array(values));
    } else {
        println!("{}", values.join(" "));
    }
}

fn emit_date(text: &str, json: bool) -> Result<(), String> {
    let date =
        dates::parse(text).ok_or_else(|| format!("cannot parse a Jalali date from {text:?}"))?;
    let gregorian = date
        .to_gregorian()
        .ok_or_else(|| format!("invalid Jalali date: {date:?}"))?;
    if json {
        println!(
            "{{\"jalali\":{{\"year\":{},\"month\":{},\"day\":{}}},\"gregorian\":{{\"year\":{},\"month\":{},\"day\":{}}},\"iso\":{}}}",
            date.year,
            date.month,
            date.day,
            gregorian.year,
            gregorian.month,
            gregorian.day,
            json_string(&gregorian.to_iso())
        );
    } else {
        println!("{}", gregorian.to_iso());
    }
    Ok(())
}

#[cfg(feature = "models")]
fn pack_tag(task: &str, pack: Option<&str>, text: &str, json: bool) -> Result<(), String> {
    let path = pack.ok_or_else(|| format!("{task} needs --pack <dir>"))?;
    let mut pipe = Pipeline::create(true, true, true);
    pipe.load_pack(task, path)?;
    match task {
        "ner" => {
            let items = pipe.ner(text)?;
            if json {
                let rows: Vec<String> = items
                    .iter()
                    .map(|item| {
                        format!(
                            "{{\"text\":{},\"label\":{},\"start\":{},\"end\":{}}}",
                            json_string(&item.text),
                            json_string(&item.label),
                            item.start,
                            item.end
                        )
                    })
                    .collect();
                println!("[{}]", rows.join(","));
            } else {
                for item in &items {
                    println!("{}\t{}", item.text, item.label);
                }
            }
        }
        "pos" | "ezafe" => {
            let items = if task == "pos" {
                pipe.pos(text)?
            } else {
                pipe.ezafe(text)?
            };
            if json {
                let rows: Vec<String> = items
                    .iter()
                    .map(|item| {
                        format!(
                            "{{\"text\":{},\"tag\":{},\"start\":{},\"end\":{}}}",
                            json_string(&item.text),
                            json_string(&item.tag),
                            item.start,
                            item.end
                        )
                    })
                    .collect();
                println!("[{}]", rows.join(","));
            } else {
                for item in &items {
                    println!("{}\t{}", item.text, item.tag);
                }
            }
        }
        "depparse" => {
            let items = pipe.depparse(text)?;
            if json {
                let rows: Vec<String> = items
                    .iter()
                    .map(|item| {
                        let head = item
                            .head
                            .map(|head| head.to_string())
                            .unwrap_or_else(|| "null".to_string());
                        format!(
                            "{{\"text\":{},\"head\":{},\"relation\":{},\"start\":{},\"end\":{}}}",
                            json_string(&item.text),
                            head,
                            json_string(&item.relation),
                            item.start,
                            item.end
                        )
                    })
                    .collect();
                println!("[{}]", rows.join(","));
            } else {
                for item in &items {
                    let head = item
                        .head
                        .map(|head| head.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    println!("{}\t{}\t{}", item.text, head, item.relation);
                }
            }
        }
        other => return Err(format!("unknown pack task: {other}")),
    }
    Ok(())
}

#[cfg(not(feature = "models"))]
fn pack_tag(_task: &str, _pack: Option<&str>, _text: &str, _json: bool) -> Result<(), String> {
    Err(
        "this rishe binary was built without the `models` feature; install with \
         `cargo install rishe --features models`"
            .to_owned(),
    )
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn json_array(values: &[String]) -> String {
    let items: Vec<String> = values.iter().map(|value| json_string(value)).collect();
    format!("[{}]", items.join(","))
}
