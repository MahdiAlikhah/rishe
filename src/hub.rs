use serde_json::Value;
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File};
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

/// Packs that live under their own manifest instead of the shared registry.
///
/// Each entry is `(name, task, base URL, manifest sha256)`. These are kept apart
/// from [`PACKS`] because their manifests are pinned separately: the shared
/// `manifest.json` digest is compiled into every released client and a fresh
/// digest mismatch is fatal, so adding a task there would break `hub pull` for
/// *all* packs on installs that already exist. A separate manifest lets a new
/// pack ship without touching the shared index.
const STANDALONE_PACKS: &[(&str, &str, &str, &str)] = &[
    (
        "rishe-vector-e5-small-fa",
        "sentence",
        "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/vector/e5-small-fa",
        "129737d3fa726182313db093a956e5d062440f414e4125de54c2bfe5c8f4089b",
    ),
    (
        "rishe-rerank-mmarco-minilmv2",
        "rerank",
        "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/rerank/mmarco-minilmv2",
        "0997d3f3e97c25de266ee4eaf48189aa3ac1a6be4a728e3095b71810552ccb23",
    ),
    // One pack, three heads (POS, NER, ezafe) over a shared backbone. The task is
    // `merged`, a pack identity rather than a single head: the caller picks the
    // head when loading. It must stay one name, because the cache stores each pack
    // name in its own directory, so a name per head would download the 45 MiB
    // backbone three times and defeat the point of merging them.
    (
        "merged-compact",
        "merged",
        "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/merged/merged-compact",
        "820a5d37fda540eeea672cf7f484b809a4fc696b2440ee0fdf83fc6d98ce60b3",
    ),
];
const PACK_BASE: &str = "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main";
const PACK_MANIFEST_SHA256: &str =
    "60885efc0680a47a3f7efbd3bc699c19766dc33c6b072b44c31086811d43f12e";
const PACK_REQUIRED: [&str; 3] = ["model.onnx", "tokenizer.json", "labels.json"];

/// Major schema version of the Hub registry manifest read by this build.
///
/// The shared `manifest.json` index is versioned independently of the pack
/// files. A missing `"schema"` field means [`MANIFEST_SCHEMA`] — the only
/// version published so far — and [`manifest_schema`] refuses every other
/// version or a non-integer value, so an older client never misreads a newer
/// index. Adding optional fields does not bump the version; a breaking layout
/// change does, and both readers are updated together.
const MANIFEST_SCHEMA: u64 = 1;

/// Run a `rishe hub` invocation, returning the error message on failure.
///
/// This keeps the original public signature. Use [`run_classified`] when the
/// caller needs to tell a malformed invocation from a failed operation; the
/// message text is identical. An empty message means the failure was already
/// reported on stdout by `hub verify` printing `invalid`.
///
/// ```
/// use rishe::hub;
///
/// let error = hub::run(&["bogus".to_string()]).unwrap_err();
/// assert_eq!(error, "unknown hub command: bogus");
/// ```
pub fn run(args: &[String]) -> Result<(), String> {
    match run_classified(args) {
        // `hub verify` reports its failure on stdout; the message is empty.
        Err(HubError::Runtime(message)) if message.is_empty() => Ok(()),
        result => result.map_err(|error| error.to_string()),
    }
}

/// A Hub failure, classified by the exit code the CLI should use.
///
/// Argument and option errors are [`HubError::Usage`]; failures that happen
/// once the arguments are understood — a name that does not resolve, a cache
/// miss, a download, a checksum or an I/O error — are [`HubError::Runtime`].
/// The message text is the same whichever variant carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubError {
    /// The Hub invocation was malformed; the CLI exits with status 2.
    Usage(String),
    /// The invocation was valid but the operation failed; the CLI exits with
    /// status 1.
    Runtime(String),
}

impl std::fmt::Display for HubError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) | Self::Runtime(message) => formatter.write_str(message),
        }
    }
}

/// Run a `rishe hub` invocation, keeping the error classification.
///
/// [`run`] is the compatibility wrapper that flattens the result to a `String`;
/// this function is what the CLI uses so that usage errors and runtime failures
/// can exit with different statuses. See `docs/errors.md`.
pub fn run_classified(args: &[String]) -> Result<(), HubError> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(HubError::Usage(
            "usage: rishe hub <pull|list|info|path|remove|unlock|verify> ...".to_string(),
        ));
    };
    match command {
        "list" => {
            // `hub list` takes no arguments; the Python CLI rejects an extra word
            // with exit 2 and this one used to ignore it.
            if let Some(extra) = args.get(1) {
                return Err(HubError::Usage(format!("unknown hub list option: {extra}")));
            }
            list().map_err(HubError::Runtime)
        }
        "info" => info(require_name(args, "hub info needs a name")?).map_err(HubError::Runtime),
        "path" => path(require_name(args, "hub path needs a name")?).map_err(HubError::Runtime),
        "remove" => {
            remove(require_name(args, "hub remove needs a name")?).map_err(HubError::Runtime)
        }
        "unlock" => {
            unlock(require_name(args, "hub unlock needs a name")?).map_err(HubError::Runtime)
        }
        "verify" => {
            let name = require_name(args, "hub verify needs a name")?;
            let valid = verify(name).map_err(HubError::Runtime)?;
            if valid {
                Ok(())
            } else {
                // `invalid` is already on stdout; exit 1 without a second line.
                Err(HubError::Runtime(String::new()))
            }
        }
        "pull" => pull(&args[1..]),
        other => Err(HubError::Usage(format!("unknown hub command: {other}"))),
    }
}

/// The name argument that every `hub` subcommand except `list` and `pull`
/// takes as its second word.
fn require_name<'a>(args: &'a [String], message: &str) -> Result<&'a str, HubError> {
    args.get(1)
        .map(String::as_str)
        .ok_or_else(|| HubError::Usage(message.to_string()))
}

/// Refuse a digest that is not 64 hex digits before anything is downloaded: a
/// typo would otherwise cost the whole transfer before the checksum rejects it.
fn validate_sha(sha: &str) -> Result<(), HubError> {
    if sha.len() != 64 || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HubError::Runtime(
            "--sha256 must be a 64-character hex digest".to_string(),
        ));
    }
    Ok(())
}

/// Infer a cache file name from a URL, or `None` when it cannot be a plain name.
///
/// The result is joined onto the cache directory, so it must stay one component:
/// a separator, a drive prefix or a `..` in it would escape the cache. A query
/// string or fragment is dropped rather than becoming part of the name.
fn infer_filename(url: &str) -> Option<&str> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    // A name holding a separator, a drive prefix or a NUL names more than one
    // path component on at least one platform, so it is never a plain file name.
    let escapes = name.contains(['/', '\\', ':', '\0']);
    if escapes {
        return None;
    }
    // Windows resolves these names to devices wherever they appear: `NUL` can be
    // downloaded and verified but never renamed into place, which leaves an
    // unremovable `.part` behind.
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&stem.as_str()) {
        return None;
    }
    // A trailing dot or space is stripped by Windows, so `model.vec.` names the
    // same file as `model.vec`: accepting it would write over a model while the
    // collision check saw two different names.
    if name.ends_with('.') || name.ends_with(' ') {
        return None;
    }
    Some(name)
}

/// Whether two cache file names resolve to the same file.
///
/// Windows and default macOS resolve names case-insensitively, so
/// `RISHE-EMBED-FA-300.VEC` overwrote the pinned model while an exact `==` said
/// it did not. Comparing lowercase is right on Linux too: the two names cannot
/// both be a registered model, and refusing is the safe direction.
fn same_cache_file(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn pull(args: &[String]) -> Result<(), HubError> {
    let name = args
        .first()
        .ok_or_else(|| HubError::Usage("hub pull needs a name".to_string()))?;
    let mut sha256 = None;
    let mut force = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--sha256" => {
                index += 1;
                sha256 = Some(
                    args.get(index)
                        .ok_or_else(|| HubError::Usage("--sha256 needs a value".to_string()))?
                        .clone(),
                );
            }
            "--force" => force = true,
            other => {
                return Err(HubError::Usage(format!("unknown hub pull option: {other}")));
            }
        }
        index += 1;
    }

    if let Some(pack) = known_pack(name) {
        // A pack is verified against the digests in its manifest, so a
        // caller-supplied digest has nothing to apply to. Refuse it rather than
        // ignore it: ignoring reads as a pin that never happened. This is a value
        // the parser accepted, so it is a runtime failure (exit 1), matching
        // `docs/errors.md` and the Python CLI.
        if sha256.is_some() {
            return Err(HubError::Runtime(
                "task packs do not accept --sha256".to_string(),
            ));
        }
        // `pack.task` and `pack.name` borrow from a row of a `const` table, so
        // they already live for the program; copy them out to drop the borrow
        // before calling a function that takes `&str`.
        let (pack_name, task) = (pack.name, pack.task);
        let path = pull_pack(pack_name, task, force).map_err(HubError::Runtime)?;
        println!("{}", path.display());
        return Ok(());
    }

    let (url, expected, filename) = if let Some((_, url, sha, filename)) =
        MODELS.iter().find(|(model, _, _, _)| model == name)
    {
        // The registry pins this name's digest, so a caller-supplied one cannot be
        // honoured; ignoring it reads as a pin that never happened.
        if sha256.is_some() {
            return Err(HubError::Runtime(
                "registered models are pinned to their published digest; omit --sha256".to_string(),
            ));
        }
        (
            (*url).to_string(),
            (*sha).to_string(),
            (*filename).to_string(),
        )
    } else {
        let sha = sha256.ok_or_else(|| {
            HubError::Runtime(format!(
                "unknown model {name:?}; known models: {}. Pass --sha256 for a custom URL.",
                model_names()
            ))
        })?;
        validate_sha(&sha)?;
        // Only a network URL is a download. `file:`/`ftp:`/`data:` would copy or
        // fetch bytes by another protocol and drop them in the model cache. The
        // HTTP client happens to reject them too, but with a parse error that says
        // nothing about the rule, so the check is explicit here like the Python one.
        let authority = name.split(['/', '?', '#']).next().unwrap_or(name);
        let scheme = match name.find(':') {
            Some(at) if at < authority.len() => name[..at].to_ascii_lowercase(),
            _ => String::new(),
        };
        if scheme != "http" && scheme != "https" {
            return Err(HubError::Runtime(format!(
                "unsupported URL scheme {scheme:?}; use http or https"
            )));
        }
        let filename = infer_filename(name).ok_or_else(|| {
            HubError::Runtime(
                "cannot infer a filename from the URL; it must end in a plain file name"
                    .to_string(),
            )
        })?;
        // A custom URL must not land on a registered model's cache file: the
        // bytes are only as trustworthy as the digest the caller passed, so
        // `hub pull http://mirror/rishe-embed-fa-300.vec` would replace a
        // pinned model and `hub path`/`embed` would then hand it out.
        if let Some((owner, _, _, _)) = MODELS
            .iter()
            .find(|(model, _, _, known)| *model != name && same_cache_file(known, filename))
        {
            return Err(HubError::Runtime(format!(
                "{filename} belongs to the registered model {owner:?}; pick a different file name for a custom URL"
            )));
        }
        (name.clone(), sha.to_ascii_lowercase(), filename.to_string())
    };
    let target = cache_dir().map_err(HubError::Runtime)?.join(&filename);
    download(&url, &target, &expected, force).map_err(HubError::Runtime)?;
    println!("{}", target.display());
    Ok(())
}

fn list() -> Result<(), String> {
    for (name, _, sha, filename) in MODELS {
        let path = cache_dir()?.join(filename);
        // `cached` is a claim about the file, so it is checked against the
        // pinned digest; a corrupt or foreign file used to read as `cached`.
        let status = if path.is_file() {
            if sha256_file(&path)? == *sha {
                "cached"
            } else {
                "corrupt"
            }
        } else if part_path(&path).is_file() {
            // An interrupted pull still takes up disk; say so instead of "missing".
            "partial"
        } else {
            "missing"
        };
        println!("{name}\t{status}\t{filename}");
    }
    for (name, _) in PACKS {
        let target = cache_dir()?.join(name);
        let status = if cached_pack(name)?.is_some() {
            "cached"
        } else if target.is_dir() {
            // An interrupted pull leaves a half-filled pack directory behind.
            "partial"
        } else {
            "missing"
        };
        println!("{name}\t{status}\tpack");
    }
    for (name, _, _, _) in STANDALONE_PACKS {
        let target = cache_dir()?.join(name);
        let status = if cached_pack(name)?.is_some() {
            "cached"
        } else if target.is_dir() {
            "partial"
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
            // A part left by an interrupted pull still takes up disk.
            let part = part_path(&path);
            if !part.is_file() {
                println!("{name}\tmissing");
                return Ok(());
            }
            let bytes = part.metadata().map_err(|error| error.to_string())?.len();
            println!("{name}\t{}\t{bytes} bytes\tpartial", part.display());
            return Ok(());
        }
        let bytes = path.metadata().map_err(|error| error.to_string())?.len();
        (path, bytes)
    } else if is_pack(name) {
        let target = cache_dir()?.join(name);
        match cached_pack(name)? {
            Some(path) => (path.clone(), directory_size(&path)?),
            None if target.is_dir() => {
                // An interrupted pull leaves a half-filled pack directory behind.
                println!(
                    "{name}\t{}\t{} bytes\tpartial",
                    target.display(),
                    directory_size(&target)?
                );
                return Ok(());
            }
            None => {
                println!("{name}\tmissing");
                return Ok(());
            }
        }
    } else {
        return Err(format!(
            "unknown model or pack {name:?}; known packs: {}",
            pack_name_list()
        ));
    };
    println!("{name}\t{}\t{bytes} bytes", path.display());
    Ok(())
}

fn path(name: &str) -> Result<(), String> {
    // Unlike `cached_model`, this also resolves packs: `docs/api.md` documents
    // `hub.path("pos-compact")` and the Python implementation already does.
    let found = if MODELS.iter().any(|(model, _, _, _)| *model == name) {
        cached_model(name)?
    } else if is_pack(name) {
        cached_pack(name)?
    } else {
        return Err(format!(
            "unknown model or pack {name:?}; known models: {}",
            model_names()
        ));
    };
    match found {
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
    } else if is_pack(name) {
        cache_dir()?.join(name)
    } else {
        return Err(format!(
            "unknown model or pack {name:?}; known packs: {}",
            pack_name_list()
        ));
    };
    let mut removed = false;
    if path.is_dir() {
        fs::remove_dir_all(&path).map_err(|error| error.to_string())?;
        removed = true;
    }
    // A model may be only partly downloaded; the part file holds that disk space.
    let mut targets = Vec::new();
    if path.is_file() {
        targets.push(path.clone());
    }
    let part = part_path(&path);
    if part.is_file() {
        targets.push(part);
    }
    // A stale lock from a killed process is invisible to this command, so a
    // removal clears it too; otherwise the next pull is refused for an hour.
    let lock = lock_path(&path);
    if lock.is_file() {
        targets.push(lock);
    }
    // An interrupted manifest fetch leaves a part file in the cache root that no
    // model or pack owns, so a pack removal also clears the manifest it reads.
    if let Some(pack) = known_pack(name) {
        let manifest = pack_manifest_path(&pack)?;
        for stale in [part_path(&manifest), lock_path(&manifest)] {
            if stale.is_file() {
                targets.push(stale);
            }
        }
    }
    for file in &targets {
        fs::remove_file(file).map_err(|error| error.to_string())?;
        removed = true;
    }
    println!("{}", if removed { "removed" } else { "not cached" });
    Ok(())
}

/// A lock quiet for this long belongs to a dead process.
///
/// A live download refreshes its lock on every chunk and the HTTP read timeout is
/// 60 s, so a running transfer cannot stay quiet this long: it either writes a
/// chunk or fails and drops the lock. `unlock` used to delete whatever lock it
/// found, so clearing one during a live pull let a second process append to the
/// same `.part` and both then failed the checksum.
///
/// `unlock` has no `--force`: the wait is two minutes, not the hour the automatic
/// takeover makes a user wait, so the escape hatch is not worth its flags.
/// shortcut: a pull stalled in a single 1 MiB read for over 120 s can be cut off; the 60 s timeout ends it first.
const LIVE_LOCK: Duration = Duration::from_secs(120);

/// Clear a download lock left behind by a killed process.
///
/// The lock is only taken over automatically after an hour, so a user whose pull
/// was interrupted had no way to retry: `--force` re-downloads but does not
/// bypass the lock, and `remove` also drops the cached files. For a pack the
/// leaked lock belongs to the shared manifest, which blocks every pack.
fn unlock(name: &str) -> Result<(), String> {
    let path = if let Some((_, _, _, filename)) = MODELS.iter().find(|item| item.0 == name) {
        cache_dir()?.join(filename)
    } else if is_pack(name) {
        cache_dir()?.join(name)
    } else {
        return Err(format!(
            "unknown model or pack {name:?}; known packs: {}",
            pack_name_list()
        ));
    };
    let mut cleared = false;
    let mut targets = vec![lock_path(&path)];
    if let Some(pack) = known_pack(name) {
        // A pack pull locks the manifest it reads, not the pack directory.
        let manifest = pack_manifest_path(&pack)?;
        targets.push(lock_path(&manifest));
    }
    for lock in targets {
        if !lock.is_file() {
            continue;
        }
        let age = fs::metadata(&lock)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok());
        if age.is_some_and(|age| age < LIVE_LOCK) {
            return Err(format!(
                "a download of {name} is running; retry, or wait {} minutes for it to count as dead",
                LIVE_LOCK.as_secs() / 60
            ));
        }
        fs::remove_file(&lock).map_err(|error| error.to_string())?;
        cleared = true;
    }
    println!("{}", if cleared { "unlocked" } else { "no lock" });
    Ok(())
}

/// A pack the registry knows, from either table.
struct KnownPack<'a> {
    name: &'a str,
    task: &'a str,
    /// Where the pack's manifest lives, and its pinned digest.
    manifest: PackManifest<'a>,
}

/// How to reach a pack's manifest.
enum PackManifest<'a> {
    /// One manifest shared by every task pack in [`PACKS`].
    Shared,
    /// A manifest belonging to this pack alone.
    Standalone(&'a str, &'a str),
}

/// Look a pack up by name in both tables.
fn known_pack(name: &str) -> Option<KnownPack<'_>> {
    if let Some((found, task)) = PACKS.iter().find(|(pack, _)| *pack == name) {
        return Some(KnownPack {
            name: found,
            task,
            manifest: PackManifest::Shared,
        });
    }
    STANDALONE_PACKS
        .iter()
        .find(|(pack, _, _, _)| *pack == name)
        .map(|(found, task, base, sha)| KnownPack {
            name: found,
            task,
            manifest: PackManifest::Standalone(base, sha),
        })
}

/// Every pack name the registry knows, in both tables.
fn pack_names() -> impl Iterator<Item = &'static str> {
    PACKS
        .iter()
        .map(|(pack, _)| *pack)
        .chain(STANDALONE_PACKS.iter().map(|(pack, _, _, _)| *pack))
}

/// Whether `name` is a pack of either kind.
fn is_pack(name: &str) -> bool {
    known_pack(name).is_some()
}

/// Name of the file a pack's manifest is cached under.
///
/// Task packs share one file; a standalone pack gets its own so a new pack can
/// be published without changing the shared manifest or its pinned digest.
fn pack_manifest_path(pack: &KnownPack<'_>) -> Result<PathBuf, String> {
    let name = match pack.manifest {
        PackManifest::Shared => "rishe-models-manifest.json".to_string(),
        PackManifest::Standalone(_, _) => format!("{}-manifest.json", pack.name),
    };
    Ok(cache_dir()?.join(name))
}
/// Verify a cached model or pack, printing `verified` or `invalid`.
///
/// Returns whether the cached files matched. The caller turns `false` into a
/// runtime exit status: a known name whose files are missing or corrupt is a
/// failed operation, not a malformed invocation.
fn verify(name: &str) -> Result<bool, String> {
    let valid = if let Some((_, _, sha, filename)) = MODELS.iter().find(|item| item.0 == name) {
        let path = cache_dir()?.join(filename);
        path.is_file() && sha256_file(&path)? == *sha
    } else if let Some(pack) = known_pack(name) {
        verify_pack(&pack)?
    } else {
        return Err(format!(
            "unknown model or pack {name:?}; known packs: {}",
            pack_name_list()
        ));
    };
    println!("{}", if valid { "verified" } else { "invalid" });
    Ok(valid)
}

fn pull_pack(name: &str, task: &str, force: bool) -> Result<PathBuf, String> {
    let pack = known_pack(name).ok_or_else(|| format!("unknown pack {name:?}"))?;
    let target = cache_dir()?.join(name);
    // A full-looking directory is not a valid pack: an older manifest or an
    // interrupted download leaves the same three files behind. Verify the hashes
    // before returning.
    if !force && verify_pack(&pack)? {
        return Ok(target);
    }
    fs::create_dir_all(&target).map_err(|error| error.to_string())?;
    let manifest = load_pack_manifest(&pack)?;
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
            &pack_file_url(&pack, local_name),
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

/// Base URL a pack's files are served from.
///
/// Task packs share the registry base and nest files under the task name; a
/// standalone pack's base already points at its own directory.
fn pack_base_of(pack: &KnownPack<'_>) -> String {
    match pack.manifest {
        PackManifest::Shared => pack_base().to_string(),
        PackManifest::Standalone(base, _) => base.trim_end_matches('/').to_string(),
    }
}

/// URL of one file inside a pack.
///
/// Task packs live at `<base>/<task>/<file>`. A standalone pack's base is the
/// pack directory itself, so its files sit directly under `<base>/<file>`.
fn pack_file_url(pack: &KnownPack<'_>, filename: &str) -> String {
    let base = pack_base_of(pack);
    match pack.manifest {
        PackManifest::Shared => format!("{base}/{}/{filename}", pack.task),
        PackManifest::Standalone(_, _) => format!("{base}/{filename}"),
    }
}

/// Fetch a pack's manifest and parse it.
///
/// A standalone pack pins its own digest, so unlike the shared manifest it can
/// be republished without invalidating the pin baked into existing clients.
fn load_pack_manifest(pack: &KnownPack<'_>) -> Result<Value, String> {
    let (base, sha) = match pack.manifest {
        PackManifest::Shared => return load_manifest(),
        PackManifest::Standalone(base, sha) => (base.trim_end_matches('/'), sha),
    };
    let path = pack_manifest_path(pack)?;
    let attempt = download(&format!("{base}/manifest.json"), &path, sha, false);
    if let Err(error) = attempt {
        if !error.contains("another rishe process is downloading") {
            return Err(error);
        }
        let deadline = Instant::now() + MANIFEST_WAIT;
        loop {
            if manifest_matches_with(&path, sha)? {
                break;
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "{error} (waited {}s for the pack manifest)",
                    MANIFEST_WAIT.as_secs()
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    parse_manifest(&text)
}

/// Validate the `schema` field of a parsed manifest and return the version.
///
/// Absent means [`MANIFEST_SCHEMA`]. Only [`MANIFEST_SCHEMA`] is accepted; any
/// other integer or a non-integer value is refused with a message naming the
/// found value and the supported version.
fn manifest_schema(manifest: &Value) -> Result<u64, String> {
    let Some(field) = manifest.get("schema") else {
        return Ok(MANIFEST_SCHEMA);
    };
    if field.as_u64() == Some(MANIFEST_SCHEMA) {
        return Ok(MANIFEST_SCHEMA);
    }
    Err(format!(
        "unsupported manifest schema {field}, this build reads schema {MANIFEST_SCHEMA}"
    ))
}

/// Parse manifest text and validate the schema version it declares.
fn parse_manifest(text: &str) -> Result<Value, String> {
    let manifest: Value =
        serde_json::from_str(text).map_err(|error| format!("invalid manifest: {error}"))?;
    manifest_schema(&manifest)?;
    Ok(manifest)
}

fn load_manifest() -> Result<Value, String> {
    let path = cache_dir()?.join("rishe-models-manifest.json");
    // The manifest is sha-pinned, so a cached copy with the right digest is the
    // same file; re-fetching it costs a round trip and fails offline for nothing.
    let attempt = download(
        &format!("{}/manifest.json", pack_base()),
        &path,
        PACK_MANIFEST_SHA256,
        false,
    );
    if let Err(error) = attempt {
        // One manifest is shared by every pack, so a second `hub pull` for a
        // different pack used to fail fast here while the first one fetched it.
        // Wait for that fetch instead: the file either appears valid or the
        // original error stands.
        if !error.contains("another rishe process is downloading") {
            return Err(error);
        }
        let deadline = Instant::now() + MANIFEST_WAIT;
        loop {
            if manifest_matches(&path)? {
                break;
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "{error} (waited {}s for the shared manifest)",
                    MANIFEST_WAIT.as_secs()
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    parse_manifest(&text)
}

/// Whether the cached manifest is present and carries the pinned digest.
fn manifest_matches(path: &Path) -> Result<bool, String> {
    manifest_matches_with(path, PACK_MANIFEST_SHA256)
}

/// Whether `path` is present and carries `digest`.
///
/// `manifest_matches_with` with an explicit digest, so a standalone pack can pin
/// its own manifest and a test needs no network.
fn manifest_matches_with(path: &Path, digest: &str) -> Result<bool, String> {
    Ok(path.is_file() && sha256_file(path)? == digest)
}

fn verify_pack(pack: &KnownPack<'_>) -> Result<bool, String> {
    let target = cache_dir()?.join(pack.name);
    let manifest_path = pack_manifest_path(pack)?;
    let expected = match pack.manifest {
        PackManifest::Shared => PACK_MANIFEST_SHA256,
        PackManifest::Standalone(_, sha) => sha,
    };
    if !target.is_dir() || !manifest_matches_with(&manifest_path, expected)? {
        return Ok(false);
    }
    let text = fs::read_to_string(&manifest_path).map_err(|error| error.to_string())?;
    let manifest = parse_manifest(&text)?;
    let Some(files) = manifest
        .get("tasks")
        .and_then(|tasks| tasks.get(pack.task))
        .and_then(|task| task.get("files"))
        .and_then(Value::as_object)
    else {
        return Ok(false);
    };
    // An empty `files` object used to run the loop below zero times and bless an
    // empty directory as a verified pack. The Python implementation already
    // refused this, so the two binaries disagreed on the same cache.
    files_match(&target, files)
}

/// Whether every file in a manifest `files` object matches `target`.
///
/// An empty object is not a match: it would bless any directory that has the
/// required file names.
fn files_match(target: &Path, files: &serde_json::Map<String, Value>) -> Result<bool, String> {
    if files.is_empty() {
        return Ok(false);
    }
    for (filename, metadata) in files {
        let Some(sha) = metadata.get("sha256").and_then(Value::as_str) else {
            return Ok(false);
        };
        let local_name = Path::new(filename)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(filename);
        let path = target.join(local_name);
        if !path.is_file() || sha256_file(&path)? != sha.to_ascii_lowercase() {
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

/// Sibling `<name>.part` file that holds an in-progress download.
fn part_path(target: &Path) -> PathBuf {
    let mut part = target.as_os_str().to_os_string();
    part.push(".part");
    PathBuf::from(part)
}

/// Sibling `<name>.lock` file that marks a download in progress.
fn lock_path(target: &Path) -> PathBuf {
    let mut lock = target.as_os_str().to_os_string();
    lock.push(".lock");
    PathBuf::from(lock)
}

/// A lock this old was left by a process that died mid-download.
const STALE_LOCK: Duration = Duration::from_secs(3600);

/// How long a second pack pull waits for another process's manifest fetch.
///
/// The manifest is small and shared by every pack, so waiting a minute is far
/// better than making `hub pull ner-compact` fail because `hub pull pos-compact`
/// happened to be inside its first second.
const MANIFEST_WAIT: Duration = Duration::from_secs(60);

/// Ceiling for a body with no `Content-Length`: 8 GiB is far past every published
/// artifact, so hitting it means the mirror is not serving a model file.
const MAX_UNKNOWN_BODY: u64 = 8 * 1024 * 1024 * 1024;

/// Held for the whole download and removed on drop.
struct DownloadLock {
    path: PathBuf,
    token: String,
}

impl DownloadLock {
    /// Refresh the mtime used for stale-lock takeover: a slow download must not
    /// look like a dead process after the first hour.
    fn touch(&self) {
        if let Ok(file) = fs::OpenOptions::new().write(true).open(&self.path) {
            let _ = file.set_modified(SystemTime::now());
        }
    }
}

impl Drop for DownloadLock {
    fn drop(&mut self) {
        // Remove the lock only while it is still ours: a download that ran past
        // the stale window has already had its lock taken over by another process.
        if fs::read_to_string(&self.path).is_ok_and(|token| token == self.token) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Claim `target` so two processes never append to one `.part` file and rename
/// it out from under each other.
///
/// A second pull fails with a clear message instead of corrupting the first. A
/// lock older than [`STALE_LOCK`] is taken over, so a crash does not block the
/// download forever.
fn acquire_download_lock(target: &Path) -> Result<DownloadLock, String> {
    let path = lock_path(target);
    let token = format!(
        "{} {}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0)
    );
    for _ in 0..2 {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                // The token lets `drop` tell its own lock from one it lost.
                let _ = file.write_all(token.as_bytes());
                return Ok(DownloadLock { path, token });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                // `force` is not a bypass: it means "re-download", not "ignore the
                // other process". Clearing the lock here would let two downloads
                // append to one `.part` file, which is the corruption the lock
                // exists to prevent.
                let stale = fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .ok()
                    .and_then(|time| time.elapsed().ok())
                    .is_some_and(|age| age > STALE_LOCK);
                if stale {
                    let _ = fs::remove_file(&path);
                    continue;
                }
                return Err(format!(
                    "another rishe process is downloading {}; retry when it finishes",
                    target.display()
                ));
            }
            Err(error) => {
                return Err(format!("cannot create {}: {error}", path.display()));
            }
        }
    }
    Err(format!("cannot lock {}", target.display()))
}

/// Why one download attempt did not finish.
enum Attempt {
    /// The remote bytes are wrong; another attempt fetches the same bytes again.
    Fatal(String),
    /// Another attempt can help: an I/O error, or a stale prefix already dropped.
    Retry(String),
}

fn download(url: &str, target: &Path, sha: &str, force: bool) -> Result<(), String> {
    if !force && target.is_file() && sha256_file(target)? == sha.to_ascii_lowercase() {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let lock = acquire_download_lock(target)?;
    download_locked(url, target, sha, &lock)
}

fn download_locked(url: &str, target: &Path, sha: &str, lock: &DownloadLock) -> Result<(), String> {
    let part = part_path(target);
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(120))
        .build();

    let mut last_error = String::new();
    for _ in 0..3 {
        let result = (|| -> Result<(), Attempt> {
            let io = |error: String| Attempt::Retry(error);
            let offset = fs::metadata(&part)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            let mut request = agent.get(url);
            if offset > 0 {
                request = request.set("Range", &format!("bytes={offset}-"));
            }
            let response = match request.call() {
                Ok(response) => response,
                Err(ureq::Error::Status(416, _)) if offset > 0 => {
                    // A complete-but-unrenamed part answers a resume with 416;
                    // promote it instead of downloading the whole body again.
                    let complete = fs::metadata(&part)
                        .map(|metadata| metadata.len() == offset)
                        .unwrap_or(false);
                    if complete
                        && sha256_file(&part).is_ok_and(|actual| actual == sha.to_ascii_lowercase())
                    {
                        fs::rename(&part, target).map_err(|error| {
                            Attempt::Fatal(format!(
                                "cannot rename {} to {}: {error}",
                                part.display(),
                                target.display()
                            ))
                        })?;
                        return Ok(());
                    }
                    // The prefix does not belong to this remote object; start over.
                    let _ = fs::remove_file(&part);
                    return Err(Attempt::Retry(format!(
                        "download failed for {url}: range not satisfiable"
                    )));
                }
                Err(error) => {
                    return Err(Attempt::Retry(format!(
                        "download failed for {url}: {error}"
                    )))
                }
            };
            let resumed = offset > 0 && response.status() == 206;
            let start = if resumed { offset } else { 0 };
            let expected = response
                .header("Content-Length")
                .and_then(|value| value.parse::<u64>().ok())
                .map(|length| length + start);
            let mut file = if resumed {
                fs::OpenOptions::new()
                    .append(true)
                    .open(&part)
                    .map_err(|error| io(error.to_string()))?
            } else {
                File::create(&part).map_err(|error| io(error.to_string()))?
            };
            let mut reader = response.into_reader();
            copy_with_progress(&mut reader, &mut file, start, expected, lock).map_err(io)?;
            // Flush to disk before the rename promotes the part file to the
            // artifact: without the fsync a crash here leaves the target short or
            // empty, which destroys a file that was good before.
            file.sync_all().map_err(|error| io(error.to_string()))?;
            drop(file);
            let actual = sha256_file(&part).map_err(io)?;
            if actual != sha.to_ascii_lowercase() {
                // The part is wrong either way, so drop it and decide whether a
                // retry is worth making.
                let _ = fs::remove_file(&part);
                return Err(if start == 0 {
                    // A fresh download's bytes came from the remote as-is, so
                    // fetching them again produces the same mismatch.
                    Attempt::Fatal(format!(
                        "checksum mismatch for {url}: expected {sha}, got {actual}"
                    ))
                } else {
                    // A resume's mismatch may be entirely in the stale prefix the
                    // retry has just deleted, so the next attempt starts from zero
                    // and can succeed.
                    Attempt::Retry(format!(
                        "checksum mismatch for {url}: the partial file did not match; restarting the download"
                    ))
                });
            }
            // A rename failure is local (the target is a directory or held by
            // another program), so retrying re-downloads the body for nothing.
            // The part stays: it is complete and the next attempt can promote it.
            fs::rename(&part, target).map_err(|error| {
                Attempt::Fatal(format!(
                    "cannot rename {} to {}: {error}",
                    part.display(),
                    target.display()
                ))
            })?;
            Ok(())
        })();
        match result {
            Ok(()) => return Ok(()),
            // A mismatch on fresh bytes is not worth repeating: the same request
            // returns the same bytes. Every other failure keeps its partial file
            // and is retried, so a slow or flaky link still makes progress.
            Err(Attempt::Fatal(error)) => return Err(error),
            Err(Attempt::Retry(error)) => last_error = error,
        }
    }
    Err(last_error)
}

/// Copy a response body into `file`, printing progress on a terminal.
fn copy_with_progress(
    reader: &mut impl Read,
    file: &mut File,
    mut written: u64,
    expected: Option<u64>,
    lock: &DownloadLock,
) -> Result<(), String> {
    let terminal = std::io::stderr().is_terminal();
    let mut report = (written / (64 << 20) + 1) * (64 << 20);
    let mut printed = false;
    let mut buffer = vec![0_u8; 1 << 20];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|error| error.to_string())?;
        // Keep the lock fresh: `acquire_download_lock` only trusts the mtime.
        lock.touch();
        written += read as u64;
        if let Some(total) = expected {
            if written > total {
                return Err(format!("server sent more than {total} bytes"));
            }
        } else if written > MAX_UNKNOWN_BODY {
            // Without a `Content-Length` the body has no declared end, so a
            // mistaken or hostile mirror could fill the disk. The largest known
            // artifact is a few gigabytes; anything past this is not a model.
            return Err(format!(
                "server sent more than {} bytes without a Content-Length",
                MAX_UNKNOWN_BODY
            ));
        }
        if terminal && written >= report {
            match expected {
                Some(total) if total > 0 => eprint!(
                    "\r  {:.0}/{:.0} MB ({:.0}%)",
                    written as f64 / 1e6,
                    total as f64 / 1e6,
                    100.0 * written as f64 / total as f64
                ),
                _ => eprint!("\r  {:.0} MB", written as f64 / 1e6),
            }
            let _ = std::io::stderr().flush();
            report += 64 << 20;
            printed = true;
        }
    }
    if printed {
        eprintln!();
    }
    Ok(())
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
    Ok(format!("{:x}", digest.finalize()))
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

/// Every registered pack name, for the "unknown pack" message.
///
/// Without this a user who mistypes the vector pack is told only about the four
/// task packs, so the pack they wanted looks like it does not exist.
fn pack_name_list() -> String {
    pack_names().collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(text: &str) -> Value {
        serde_json::from_str(text).expect("test manifest is valid JSON")
    }

    /// A loopback HTTP server that replays queued responses for tests.
    struct Serve {
        url: String,
        address: std::net::SocketAddr,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        handle: std::thread::JoinHandle<Vec<String>>,
    }

    impl Serve {
        /// Serve the queued responses in order, repeating the last one.
        fn start(responses: Vec<(String, Vec<u8>)>) -> Self {
            use std::io::{Read, Write};
            use std::net::TcpListener;
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;

            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let address = listener.local_addr().expect("address");
            let stop = Arc::new(AtomicBool::new(false));
            let finish = Arc::clone(&stop);
            let handle = std::thread::spawn(move || {
                let mut seen = Vec::new();
                while let Ok((mut stream, _)) = listener.accept() {
                    if finish.load(Ordering::SeqCst) {
                        break;
                    }
                    let mut buffer = [0_u8; 2048];
                    let read = stream.read(&mut buffer).expect("request");
                    seen.push(String::from_utf8_lossy(&buffer[..read]).to_ascii_lowercase());
                    let (headers, body) = &responses[(seen.len() - 1).min(responses.len() - 1)];
                    stream.write_all(headers.as_bytes()).expect("headers");
                    stream.write_all(body).expect("body");
                }
                seen
            });
            Self {
                url: format!("http://{address}/model"),
                address,
                stop,
                handle,
            }
        }

        /// Stop accepting and return every request the client sent.
        fn finish(self) -> Vec<String> {
            use std::sync::atomic::Ordering;

            self.stop.store(true, Ordering::SeqCst);
            std::net::TcpStream::connect(self.address).expect("wake server");
            self.handle.join().expect("server")
        }
    }

    fn ok_response(body: &[u8]) -> (String, Vec<u8>) {
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        (head, body.to_vec())
    }

    fn partial_response(body: &[u8], start: usize) -> (String, Vec<u8>) {
        // `Connection: close` matters as much here as in `ok_response`: without it
        // ureq pools the socket, while this server has already gone back to
        // `accept()` and will never read that connection again. A test that made a
        // second request would then block until the read timeout instead of failing.
        let head = format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-{}/{}\r\nConnection: close\r\n\r\n",
            body.len() - start,
            start,
            body.len() - 1,
            body.len()
        );
        (head, body[start..].to_vec())
    }

    fn empty_response(status: &str) -> (String, Vec<u8>) {
        (
            format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
            Vec::new(),
        )
    }

    fn digest(body: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(body);
        format!("{:x}", hasher.finalize())
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        // Under `target/`, not `temp_dir()`: on Windows the temp directory's ACL
        // refuses `create_dir_all` from this process, which failed twelve of the
        // tests below with "scratch dir: PermissionDenied" and left the whole hub
        // module unverified there. `tests/cli.rs` already bases its scratch dirs
        // here for the same reason, and `target/` is gitignored.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("hub-scratch")
            .join(format!("{name}-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("scratch dir");
        root
    }

    #[test]
    fn a_held_manifest_lock_waits_instead_of_failing_a_second_pack() {
        // One manifest is shared by every pack, so a second `hub pull` for a
        // different pack used to fail with "another rishe process is
        // downloading" while the first one was still fetching the manifest.
        let root = scratch("manifest-wait");
        let path = root.join("rishe-models-manifest.json");
        let body = br#"{"tasks":{}}"#;
        let digest = {
            use sha2::Digest;
            let mut hasher = Sha256::new();
            hasher.update(body);
            format!("{:x}", hasher.finalize())
        };
        std::fs::write(lock_path(&path), b"other").expect("lock");
        // Stand-in for the other process finishing: the valid manifest appears.
        let writer = {
            let path = path.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                std::fs::write(&path, body).expect("manifest");
            })
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        let matched = loop {
            if manifest_matches_with(&path, &digest).expect("digest") {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        writer.join().expect("writer thread");
        assert!(
            matched,
            "the wait must observe the manifest the other pull wrote"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// `manifest_matches` with an explicit digest, so the test needs no network.
    fn manifest_matches_with(path: &Path, digest: &str) -> Result<bool, String> {
        Ok(path.is_file() && sha256_file(path)? == digest)
    }

    #[test]
    fn an_uppercase_manifest_digest_still_verifies() {
        // `download` lowercases the digest in all three of its comparisons, but
        // `files_match` compared verbatim, so a manifest written with uppercase
        // hex (what Windows Get-FileHash prints) made `hub verify` call a
        // byte-correct cache invalid forever while `pull` and `list` said it was
        // fine. This does not need the temp-dir ACL the other hub tests trip on.
        use sha2::Digest;
        let root = scratch("verify-uppercase-digest");
        let pack = root.join("pos-compact");
        std::fs::create_dir_all(&pack).expect("pack dir");
        let mut files = serde_json::Map::new();
        for name in PACK_REQUIRED {
            let body = b"rishe";
            std::fs::write(pack.join(name), body).expect("required file");
            let mut hasher = Sha256::new();
            hasher.update(body);
            files.insert(
                name.to_string(),
                serde_json::json!({"sha256": format!("{:X}", hasher.finalize())}),
            );
        }
        let valid = files_match(&pack, &files).expect("compare");
        std::fs::remove_dir_all(&root).ok();
        assert!(valid, "an uppercase digest names the same bytes");
    }

    #[test]
    fn an_empty_file_list_does_not_verify_a_pack() {
        // `files` was iterated without a length check, so a manifest entry with
        // an empty object blessed any directory that had the required names.
        let root = scratch("verify-empty-files");
        let pack = root.join("pos-compact");
        std::fs::create_dir_all(&pack).expect("pack dir");
        for name in PACK_REQUIRED {
            std::fs::write(pack.join(name), b"x").expect("required file");
        }
        let parsed = parse_manifest(r#"{"tasks":{"pos":{"files":{}}}}"#).expect("manifest");
        let files = parsed
            .get("tasks")
            .and_then(|tasks| tasks.get("pos"))
            .and_then(|task| task.get("files"))
            .and_then(Value::as_object)
            .expect("files object");
        let valid = files_match(&pack, files).expect("compare");
        std::fs::remove_dir_all(&root).ok();
        assert!(!valid, "an empty file list must not verify a directory");
    }

    #[test]
    fn absent_schema_means_version_one() {
        assert_eq!(manifest_schema(&manifest(r#"{"tasks":{}}"#)), Ok(1));
    }

    #[test]
    fn a_standalone_pack_is_known_and_task_packs_are_not_disturbed() {
        // The vector pack lives in its own table so publishing it never touches
        // the shared manifest, whose digest every released client pins.
        let pack = known_pack("rishe-vector-e5-small-fa").expect("vector pack is known");
        assert_eq!(pack.task, "sentence");
        assert!(matches!(pack.manifest, PackManifest::Standalone(_, _)));
        assert!(is_pack("rishe-vector-e5-small-fa"));

        // The four task packs keep using the shared manifest.
        for name in [
            "pos-compact",
            "ner-compact",
            "ezafe-compact",
            "depparse-compact",
        ] {
            let found = known_pack(name).expect(name);
            assert!(
                matches!(found.manifest, PackManifest::Shared),
                "{name} must use the shared manifest"
            );
        }
        assert!(known_pack("rishe-embed-fa-300-v2").is_none());
    }

    #[test]
    fn a_standalone_pack_reads_its_own_manifest_file() {
        // Sharing the task-pack manifest file would mean two packs of different
        // versions overwriting each other's pinned digest in one cache slot.
        let standalone = known_pack("rishe-vector-e5-small-fa").expect("pack");
        let shared = known_pack("pos-compact").expect("pack");
        assert_ne!(
            pack_manifest_path(&standalone).unwrap(),
            pack_manifest_path(&shared).unwrap()
        );
        assert!(pack_manifest_path(&standalone)
            .unwrap()
            .ends_with("rishe-vector-e5-small-fa-manifest.json"));
    }

    #[test]
    fn a_standalone_pack_nests_files_under_its_own_base() {
        // Task packs serve `<base>/<task>/<file>`; a standalone pack's base is
        // already its directory, so it must serve `<base>/<file>`. Getting this
        // wrong 404s every file: `.../e5-small-fa/sentence/model.onnx`.
        let standalone = known_pack("rishe-vector-e5-small-fa").expect("pack");
        assert_eq!(
            pack_file_url(&standalone, "model.onnx"),
            "https://huggingface.co/MahdiAlikhah/rishe-models/resolve/main/vector/e5-small-fa/model.onnx"
        );
        let shared = known_pack("pos-compact").expect("pack");
        assert!(pack_file_url(&shared, "model.onnx").ends_with("/pos/model.onnx"));
    }

    #[test]
    fn every_pack_name_is_listed_once() {
        let names: Vec<_> = pack_names().collect();
        assert_eq!(names.len(), PACKS.len() + STANDALONE_PACKS.len());
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "duplicate pack name");
        assert!(names.contains(&"rishe-vector-e5-small-fa"));
    }

    #[test]
    fn the_standalone_pin_is_well_formed() {
        // A typo in the pinned digest would fail every pull with a checksum
        // mismatch, which reads as a corrupted upload rather than a bad constant.
        for (name, _, base, sha) in STANDALONE_PACKS {
            assert!(
                sha.len() == 64 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{name} has a malformed manifest digest"
            );
            assert!(base.starts_with("https://"), "{name} base is not https");
            assert!(!base.ends_with('/'), "{name} base has a trailing slash");
        }
    }

    #[test]
    fn schema_one_is_accepted() {
        assert_eq!(manifest_schema(&manifest(r#"{"schema":1}"#)), Ok(1));
    }

    #[test]
    fn newer_schema_is_refused() {
        let error = manifest_schema(&manifest(r#"{"schema":2}"#)).unwrap_err();
        assert!(error.contains("schema 2"), "{error}");
        assert!(error.contains("reads schema 1"), "{error}");
    }

    #[test]
    fn non_integer_schema_is_refused() {
        for text in [
            r#"{"schema":"1"}"#,
            r#"{"schema":1.5}"#,
            r#"{"schema":true}"#,
            r#"{"schema":null}"#,
        ] {
            let error = manifest_schema(&manifest(text)).unwrap_err();
            assert!(error.contains("reads schema 1"), "{text}: {error}");
        }
    }

    #[test]
    fn a_held_lock_refuses_a_second_download() {
        let root = scratch("download-lock");
        let target = root.join("model.bin");
        std::fs::write(root.join("model.bin.lock"), b"").expect("lock");
        let error = download("http://127.0.0.1:1/x", &target, &"0".repeat(64), false)
            .expect_err("a held lock must refuse the download");
        assert!(
            error.contains("another rishe process is downloading"),
            "{error}"
        );
        assert!(
            !root.join("model.bin.part").exists(),
            "no download may start while another holds the lock"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_sha_that_is_not_hex_is_rejected_before_the_download() {
        // Length alone used to pass, so a typo paid for the whole transfer.
        let error = pull(&[
            "https://example.invalid/model.bin".to_string(),
            "--sha256".to_string(),
            "z".repeat(64),
        ])
        .expect_err("a non-hex digest must be refused");
        assert!(error.to_string().contains("hex digest"), "{error}");
        // A real hex digest gets past validation and fails looking for the host.
        let valid = "0".repeat(64);
        let error = pull(&[
            "https://127.0.0.1:1/model.bin".to_string(),
            "--sha256".to_string(),
            valid,
        ])
        .expect_err("the download cannot succeed");
        assert!(!error.to_string().contains("hex digest"), "{error}");
    }

    #[test]
    fn a_taken_over_lock_is_not_deleted_by_its_previous_owner() {
        // The old download kept running past the stale window; on finish it used
        // to delete the new owner's lock and let a third process in.
        let root = scratch("download-lock-owner");
        let target = root.join("model.bin");
        let first = acquire_download_lock(&target).expect("first lock");
        let lock = lock_path(&target);
        let file = fs::File::options().write(true).open(&lock).expect("open");
        file.set_modified(std::time::SystemTime::now() - Duration::from_secs(7200))
            .expect("backdate the lock");
        drop(file);
        let second = acquire_download_lock(&target).expect("take over the stale lock");
        drop(first);
        assert!(lock.exists(), "the previous owner deleted the new lock");
        drop(second);
        assert!(!lock.exists(), "the owner left its lock behind");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_body_longer_than_its_declared_length_is_refused() {
        // The length was parsed only to print a percentage, so an endless mirror
        // wrote until the disk was full.
        let root = scratch("download-length");
        let target = root.join("body.bin");
        let lock = acquire_download_lock(&target).expect("lock");
        let mut sink = File::create(&target).expect("body");
        let mut body = std::io::Cursor::new(vec![0_u8; 4096]);
        let error = copy_with_progress(&mut body, &mut sink, 0, Some(1024), &lock)
            .expect_err("a body past the declared length must not be written");
        assert!(error.contains("more than 1024 bytes"), "{error}");
        drop(lock);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_stale_lock_is_taken_over() {
        let root = scratch("download-stale-lock");
        let target = root.join("model.bin");
        let lock = root.join("model.bin.lock");
        std::fs::write(&lock, b"").expect("lock");
        let file = fs::File::options()
            .write(true)
            .open(&lock)
            .expect("open the lock");
        file.set_modified(std::time::SystemTime::now() - Duration::from_secs(7200))
            .expect("backdate the lock");
        drop(file);

        let body = b"fresh payload".to_vec();
        let server = Serve::start(vec![ok_response(&body)]);
        download(&server.url, &target, &digest(&body), false).expect("download");
        assert_eq!(std::fs::read(&target).expect("target"), body);
        assert!(!lock.exists(), "lock left behind");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn download_resumes_a_partial_file() {
        let body = b"resumed payload".to_vec();
        let kept = 5;
        let root = scratch("download");
        let target = root.join("model.bin");
        std::fs::write(root.join("model.bin.part"), &body[..kept]).expect("partial file");

        let server = Serve::start(vec![partial_response(&body, kept)]);
        download(&server.url, &target, &digest(&body), false).expect("download");
        let requests = server.finish();
        assert!(requests[0].contains("range: bytes=5-"), "{}", requests[0]);
        assert_eq!(std::fs::read(&target).expect("target"), body);
        assert!(!root.join("model.bin.part").exists(), "partial left behind");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_failed_attempt_keeps_the_partial_file() {
        let root = scratch("download-keep");
        let target = root.join("model.bin");
        let part = root.join("model.bin.part");
        std::fs::write(&part, b"partial").expect("partial file");

        let server = Serve::start(vec![empty_response("500 Internal Server Error")]);
        assert!(download(&server.url, &target, &"0".repeat(64), false).is_err());
        let requests = server.finish();
        assert_eq!(requests.len(), 3, "a server error is worth retrying");
        assert_eq!(std::fs::read(&part).expect("partial file kept"), b"partial");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_unsatisfiable_range_starts_over() {
        let body = b"fresh payload".to_vec();
        let root = scratch("download-416");
        let target = root.join("model.bin");
        std::fs::write(root.join("model.bin.part"), vec![0_u8; 64]).expect("stale partial");

        let server = Serve::start(vec![
            empty_response("416 Range Not Satisfiable"),
            ok_response(&body),
        ]);
        download(&server.url, &target, &digest(&body), false).expect("download");
        let requests = server.finish();
        assert!(requests[0].contains("range: bytes=64-"), "{}", requests[0]);
        assert!(!requests[1].contains("range:"), "{}", requests[1]);
        assert_eq!(std::fs::read(&target).expect("target"), body);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_checksum_mismatch_fails_once_and_drops_the_partial_file() {
        let body = b"bytes that do not match the digest".to_vec();
        let root = scratch("download-sum");
        let target = root.join("model.bin");
        let part = root.join("model.bin.part");

        // The server would answer a retry, so a slow client shows up as a count.
        let server = Serve::start(vec![ok_response(&body)]);
        let error = download(&server.url, &target, &"0".repeat(64), false).expect_err("mismatch");
        let requests = server.finish();
        assert!(
            error.contains("checksum mismatch"),
            "a wrong checksum must fail on the first attempt: {error}"
        );
        assert_eq!(
            requests.len(),
            1,
            "a wrong checksum downloaded the same bytes again"
        );
        assert!(!part.exists(), "partial file survived a checksum mismatch");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_stale_prefix_on_a_resume_restarts_instead_of_failing() {
        let body = b"correct remote payload".to_vec();
        let kept = 8;
        let root = scratch("download-stale-prefix");
        let target = root.join("model.bin");
        // Same length as the real prefix, different bytes: the 416 branch cannot
        // catch this, only the digest step can.
        std::fs::write(root.join("model.bin.part"), b"STALE!!!").expect("stale partial");

        let server = Serve::start(vec![partial_response(&body, kept), ok_response(&body)]);
        download(&server.url, &target, &digest(&body), false)
            .expect("a stale prefix must not block the pull");
        let requests = server.finish();
        assert!(
            requests[0].contains("range: bytes=8-"),
            "the first attempt should resume: {}",
            requests[0]
        );
        assert!(
            !requests[1].contains("range:"),
            "the retry should start from zero: {}",
            requests[1]
        );
        assert_eq!(requests.len(), 2, "one mismatch, one restart");
        assert_eq!(std::fs::read(&target).expect("target"), body);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn infer_filename_rejects_anything_that_could_escape_the_cache() {
        // Plain URLs keep their last segment.
        assert_eq!(infer_filename("http://host/model.vec"), Some("model.vec"));
        assert_eq!(infer_filename("https://host/a/b/fa.rvec"), Some("fa.rvec"));
        // A query string or fragment is dropped, not turned into a name.
        assert_eq!(
            infer_filename("http://host/model.vec?a=1"),
            Some("model.vec")
        );
        assert_eq!(
            infer_filename("http://host/model.vec#frag"),
            Some("model.vec")
        );
        // A traversal in the URL collapses to its last segment, which is safe:
        // `http://host/../../etc/passwd` lands at `<cache>/passwd`, inside the cache.
        assert_eq!(
            infer_filename("http://host/../../etc/passwd"),
            Some("passwd")
        );
        // Traversal, separators, drive prefixes and empty names are refused. The
        // backslash forms are the real attack on Windows, where it separates, and a
        // colon in the last segment is an alternate data stream or a drive prefix.
        for url in [
            "http://host/..",
            "http://host/.",
            "http://host/",
            "http://host/..\\..\\important.txt",
            "http://host/a\\b.vec",
            "http://host/x:y.dll",
            // Windows device names resolve to the device, so `NUL` downloads and
            // verifies but can never be renamed into place.
            "http://host/NUL",
            "http://host/nul.vec",
            "http://host/com1",
            "http://host/LPT9.bin",
        ] {
            assert_eq!(infer_filename(url), None, "{url} must be refused");
        }
        // A device name only bites as the whole stem, not as part of one.
        assert_eq!(infer_filename("http://host/nulls.vec"), Some("nulls.vec"));
    }

    #[test]
    fn a_custom_url_cannot_overwrite_a_registered_model() {
        // `hub pull http://mirror/rishe-embed-fa-300.vec --sha256 <own digest>`
        // used to replace the pinned file with bytes nobody verified, and `hub
        // path`/`embed` then handed them out.
        let registered = MODELS[0];
        let filename = registered.3;
        let error = pull(&[
            format!("http://mirror/{filename}"),
            "--sha256".to_string(),
            "0".repeat(64),
        ])
        .expect_err("a registered file name must be refused");
        assert!(
            error
                .to_string()
                .contains("belongs to the registered model"),
            "{error}"
        );
    }

    #[test]
    fn a_registered_model_name_cannot_be_matched_by_case_or_a_trailing_dot() {
        // The guard compared exact strings, so on Windows (and default macOS)
        // `RISHE-EMBED-FA-300.VEC` resolved to the pinned file and replaced it.
        let registered = MODELS[0];
        let filename = registered.3;
        for variant in [
            filename.to_ascii_uppercase(),
            filename.to_ascii_lowercase(),
            format!("{} ", filename),
        ] {
            if variant.trim() == filename {
                continue;
            }
            let error = pull(&[
                format!("http://mirror/{variant}"),
                "--sha256".to_string(),
                "0".repeat(64),
            ])
            .expect_err("a case or trailing-space variant must be refused");
            assert!(
                error
                    .to_string()
                    .contains("belongs to the registered model"),
                "{variant}: {error}"
            );
        }
        // A trailing dot is refused before the download starts: Windows strips it,
        // so it names the pinned file while comparing as a different string.
        assert_eq!(infer_filename(&format!("http://host/{filename}.")), None);
    }

    #[test]
    fn same_cache_file_ignores_case() {
        assert!(same_cache_file("model.vec", "MODEL.VEC"));
        assert!(same_cache_file("Model.Vec", "model.vec"));
        assert!(!same_cache_file("model.vec", "model.rvec"));
    }

    #[test]
    fn a_custom_url_must_be_http() {
        // `file:`/`ftp:`/`data:` copied or fetched bytes by another protocol and
        // dropped them in the model cache. The HTTP client rejected them by
        // accident with a parse error, so the rule was never stated or tested.
        for url in [
            "file:///etc/passwd",
            "ftp://example.invalid/model.bin",
            "data:text/plain,hi",
        ] {
            let error = pull(&[url.to_string(), "--sha256".to_string(), "0".repeat(64)])
                .expect_err("a non-http URL must be refused");
            assert!(
                error.to_string().contains("unsupported URL scheme"),
                "{error}"
            );
        }
    }

    #[test]
    fn unlocked_clears_a_leaked_lock_without_dropping_the_cache() {
        // A killed pull left a lock that blocked retries for an hour, and neither
        // `--force` nor `remove` cleared it (`remove` also deletes the payload).
        let root = scratch("hub-unlock");
        let target = root.join("model.bin");
        std::fs::write(&target, b"payload").expect("payload");
        let lock = lock_path(&target);
        std::fs::write(&lock, b"1234 0").expect("lock");

        // The lock refuses a second pull for as long as it is fresh.
        let error = match acquire_download_lock(&target) {
            Ok(_) => panic!("a held lock must refuse a second download"),
            Err(error) => error,
        };
        assert!(error.contains("another rishe process"), "{error}");

        std::fs::remove_file(&lock).expect("unlock");
        assert!(target.is_file(), "unlocking must keep the payload");
        acquire_download_lock(&target).expect("the freed lock is claimable");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn parse_manifest_applies_the_schema_check() {
        assert!(parse_manifest(r#"{"tasks":{}}"#).is_ok());
        assert!(parse_manifest(r#"{"schema":1}"#).is_ok());
        assert!(parse_manifest(r#"{"schema":2}"#).is_err());
        assert!(parse_manifest("not json").is_err());
    }
}
