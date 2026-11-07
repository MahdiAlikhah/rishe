use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const MODELS: &[(&str, &str, &str, &str)] = &[
    (
        "rishe-embed-fa-300-v2",
        "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/embeddings/rishe-embed-fa-300-v2/rishe-embed-fa-300-v2.rvec",
        "b870a2d88dc04cadaf117d7fb8aad07264c640293adb8689e69cac3d123a37f1",
        "rishe-embed-fa-300-v2.rvec",
    ),
    (
        "rishe-embed-fa-300",
        "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/embeddings/rishe-embed-fa-300.vec",
        "afed6aa266e07745ce7b6b1e2f6600f6f0373dc91e197902d7305e19d2d35bf5",
        "rishe-embed-fa-300.vec",
    ),
    (
        "hazm-fasttext-300",
        "https://huggingface.co/roshan-research/hazm-word-embedding/resolve/main/fasttext_skipgram_300.vec",
        "43054876a1a3a62a067ad51304957f5f69fbca1620ca2fe610da6d4c9c8d2d04",
        "hazm-fasttext-300.vec",
    ),
];
const PACKS: &[(&str, &str)] = &[
    ("pos-compact", "pos"),
    ("ner-compact", "ner"),
    ("ezafe-compact", "ezafe"),
    ("depparse-compact", "depparse"),
];
const PACK_BASE: &str = "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main";
const PACK_MANIFEST_SHA256: &str =
    "60885efc0680a47a3f7efbd3bc699c19766dc33c6b072b44c31086811d43f12e";
const PACK_REQUIRED: [&str; 3] = ["model.onnx", "tokenizer.json", "labels.json"];

pub fn run(args: &[String]) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err("usage: rishe hub <pull|list|info|path|remove|verify> ...".to_string());
    };
    match command {
        "list" => list(),
        "info" => info(
            args.get(1)
                .map(String::as_str)
                .ok_or("hub info needs a name")?,
        ),
        "path" => path(
            args.get(1)
                .map(String::as_str)
                .ok_or("hub path needs a name")?,
        ),
        "remove" => remove(
            args.get(1)
                .map(String::as_str)
                .ok_or("hub remove needs a name")?,
        ),
        "verify" => verify(
            args.get(1)
                .map(String::as_str)
                .ok_or("hub verify needs a name")?,
        ),
        "pull" => pull(&args[1..]),
        other => Err(format!("unknown hub command: {other}")),
    }
}

fn pull(args: &[String]) -> Result<(), String> {
    let name = args.first().ok_or("hub pull needs a name")?;
    let mut sha256 = None;
    let mut force = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--sha256" => {
                index += 1;
                sha256 = Some(args.get(index).ok_or("--sha256 needs a value")?.clone());
            }
            "--force" => force = true,
            other => return Err(format!("unknown hub pull option: {other}")),
        }
        index += 1;
    }

    if let Some((_, task)) = PACKS.iter().find(|(pack, _)| pack == name) {
        let path = pull_pack(name, task, force)?;
        println!("{}", path.display());
        return Ok(());
    }

    let (url, expected, filename) = if let Some((_, url, sha, filename)) =
        MODELS.iter().find(|(model, _, _, _)| model == name)
    {
        (
            (*url).to_string(),
            (*sha).to_string(),
            (*filename).to_string(),
        )
    } else {
        let sha = sha256.ok_or_else(|| {
            format!(
                "unknown model {name:?}; known models: {}. Pass --sha256 for a custom URL.",
                model_names()
            )
        })?;
        if sha.len() != 64 {
            return Err("--sha256 must be a 64-character hex digest".to_string());
        }
        let filename = name.rsplit('/').next().unwrap_or(name);
        if filename.is_empty() || filename == "." || filename == ".." {
            return Err("cannot infer a filename from the URL".to_string());
        }
        (name.clone(), sha.to_ascii_lowercase(), filename.to_string())
    };
    let target = cache_dir()?.join(&filename);
    download(&url, &target, &expected, force)?;
    println!("{}", target.display());
    Ok(())
}

fn list() -> Result<(), String> {
    for (name, _, _, filename) in MODELS {
        let status = if cache_dir()?.join(filename).is_file() {
            "cached"
        } else {
            "missing"
        };
        println!("{name}\t{status}\t{filename}");
    }
    for (name, _) in PACKS {
        let status = if cached_pack(name)?.is_some() {
            "cached"
        } else {
            "missing"
        };
        println!("{name}\t{status}\tpack");
    }
    Ok(())
}

fn info(name: &str) -> Result<(), String> {
    let (path, bytes) = if let Some((_, _, _, filename)) = MODELS.iter().find(|item| item.0 == name)
    {
        let path = cache_dir()?.join(filename);
        if !path.is_file() {
            println!("{name}\tmissing");
            return Ok(());
        }
        let bytes = path.metadata().map_err(|error| error.to_string())?.len();
        (path, bytes)
    } else if PACKS.iter().any(|(pack, _)| *pack == name) {
        let Some(path) = cached_pack(name)? else {
            println!("{name}\tmissing");
            return Ok(());
        };
        (path.clone(), directory_size(&path)?)
    } else {
        return Err(format!("unknown model or pack {name:?}"));
    };
    println!("{name}\t{}\t{bytes} bytes", path.display());
    Ok(())
}

fn path(name: &str) -> Result<(), String> {
    match cached_model(name)? {
        Some(path) => {
            println!("{}", path.display());
            Ok(())
        }
        None => Err(format!(
            "{name:?} is not cached; run `rishe hub pull {name}`"
        )),
    }
}

fn remove(name: &str) -> Result<(), String> {
    let path = if let Some((_, _, _, filename)) = MODELS.iter().find(|item| item.0 == name) {
        cache_dir()?.join(filename)
    } else if PACKS.iter().any(|(pack, _)| *pack == name) {
        cache_dir()?.join(name)
    } else {
        return Err(format!("unknown model or pack {name:?}"));
    };
    if !path.exists() {
        println!("not cached");
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(&path).map_err(|error| error.to_string())?;
    } else {
        fs::remove_file(&path).map_err(|error| error.to_string())?;
    }
    println!("removed");
    Ok(())
}

fn verify(name: &str) -> Result<(), String> {
    let valid = if let Some((_, _, sha, filename)) = MODELS.iter().find(|item| item.0 == name) {
        let path = cache_dir()?.join(filename);
        path.is_file() && sha256_file(&path)? == *sha
    } else if let Some((_, task)) = PACKS.iter().find(|(pack, _)| *pack == name) {
        verify_pack(name, task)?
    } else {
        return Err(format!("unknown model or pack {name:?}"));
    };
    println!("{}", if valid { "verified" } else { "invalid" });
    Ok(())
}

fn pull_pack(name: &str, task: &str, force: bool) -> Result<PathBuf, String> {
    let target = cache_dir()?.join(name);
    if !force && PACK_REQUIRED.iter().all(|file| target.join(file).is_file()) {
        return Ok(target);
    }
    fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    let manifest = load_manifest()?;
    let files = manifest
        .get("tasks")
        .and_then(|tasks| tasks.get(task))
        .and_then(|task| task.get("files"))
        .and_then(Value::as_object)
        .ok_or_else(|| format!("manifest has no files for task {task:?}"))?;
    for (filename, metadata) in files {
        let sha = metadata
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("manifest has no sha256 for {filename}"))?;
        let local_name = Path::new(filename)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("invalid pack filename {filename:?}"))?;
        download(
            &format!("{}/{task}/{local_name}", pack_base()),
            &target.join(local_name),
            sha,
            force,
        )?;
    }
    for file in PACK_REQUIRED {
        if !target.join(file).is_file() {
            return Err(format!("pack {name:?} is incomplete; missing {file}"));
        }
    }
    Ok(target)
}

fn load_manifest() -> Result<Value, String> {
    let path = cache_dir()?.join("rishe-models-manifest.json");
    download(
        &format!("{}/manifest.json", pack_base()),
        &path,
        PACK_MANIFEST_SHA256,
        true,
    )?;
    let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| format!("invalid manifest: {error}"))
}

fn verify_pack(name: &str, task: &str) -> Result<bool, String> {
    let target = cache_dir()?.join(name);
    let manifest_path = cache_dir()?.join("rishe-models-manifest.json");
    if !target.is_dir()
        || !manifest_path.is_file()
        || sha256_file(&manifest_path)? != PACK_MANIFEST_SHA256
    {
        return Ok(false);
    }
    let text = fs::read_to_string(&manifest_path).map_err(|error| error.to_string())?;
    let manifest: Value =
        serde_json::from_str(&text).map_err(|error| format!("invalid manifest: {error}"))?;
    let Some(files) = manifest
        .get("tasks")
        .and_then(|tasks| tasks.get(task))
        .and_then(|task| task.get("files"))
        .and_then(Value::as_object)
    else {
        return Ok(false);
    };
    for (filename, metadata) in files {
        let Some(sha) = metadata.get("sha256").and_then(Value::as_str) else {
            return Ok(false);
        };
        let local_name = Path::new(filename)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(filename);
        let path = target.join(local_name);
        if !path.is_file() || sha256_file(&path)? != sha {
            return Ok(false);
        }
    }
    Ok(true)
}

fn cached_pack(name: &str) -> Result<Option<PathBuf>, String> {
    let target = cache_dir()?.join(name);
    Ok(
        if PACK_REQUIRED.iter().all(|file| target.join(file).is_file()) {
            Some(target)
        } else {
            None
        },
    )
}

/// Local path of a pulled single-file model, or `None` when it is not cached.
///
/// The name must be one of the registered models (see `rishe hub list`). This
/// never touches the network; use `rishe hub pull` to download first.
pub fn cached_model(name: &str) -> Result<Option<PathBuf>, String> {
    let (_, _, _, filename) = MODELS
        .iter()
        .find(|(model, _, _, _)| *model == name)
        .ok_or_else(|| format!("unknown model {name:?}; known models: {}", model_names()))?;
    let path = cache_dir()?.join(filename);
    Ok(path.is_file().then_some(path))
}

fn cache_dir() -> Result<PathBuf, String> {
    if let Some(root) = env::var_os("RISHE_HOME") {
        return Ok(PathBuf::from(root).join("models"));
    }
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(PathBuf::from))
        .ok_or("cannot locate the home directory; set RISHE_HOME")?;
    Ok(home.join(".cache").join("rishe").join("models"))
}

fn pack_base() -> String {
    env::var("RISHE_MODEL_BASE")
        .unwrap_or_else(|_| PACK_BASE.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn download(url: &str, target: &Path, sha: &str, force: bool) -> Result<(), String> {
    if !force && target.is_file() && sha256_file(target)? == sha.to_ascii_lowercase() {
        return Ok(());
    }
    let mut part = target.as_os_str().to_os_string();
    part.push(".part");
    let part = PathBuf::from(part);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(120))
        .build();
    let mut last_error = String::new();
    for _ in 0..3 {
        let attempt = (|| -> Result<(), String> {
            let response = agent
                .get(url)
                .call()
                .map_err(|error| format!("download failed for {url}: {error}"))?;
            let mut reader = response.into_reader();
            let mut file = File::create(&part).map_err(|error| error.to_string())?;
            std::io::copy(&mut reader, &mut file).map_err(|error| error.to_string())?;
            file.flush().map_err(|error| error.to_string())?;
            drop(file);
            let actual = sha256_file(&part)?;
            if actual != sha.to_ascii_lowercase() {
                return Err(format!(
                    "checksum mismatch for {url}: expected {sha}, got {actual}"
                ));
            }
            fs::rename(&part, target).map_err(|error| error.to_string())?;
            Ok(())
        })();
        match attempt {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = error;
                let _ = fs::remove_file(&part);
            }
        }
    }
    Err(last_error)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn directory_size(path: &Path) -> Result<u64, String> {
    let mut bytes = 0;
    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        bytes += if metadata.is_dir() {
            directory_size(&entry.path())?
        } else {
            metadata.len()
        };
    }
    Ok(bytes)
}

fn model_names() -> String {
    MODELS
        .iter()
        .map(|(name, _, _, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}
