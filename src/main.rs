use clap::{Parser, Subcommand};
use serde::Deserialize;
use threadpool::ThreadPool;
use std::collections::HashMap;
use std::fmt::Display;
use std::process;
use std::sync::mpsc;


const CACHE_CHECK_THREADS: usize = 16;
const DISCOVER_NIX_FUNC: &str = include_str!("discover.nix");
const SKIP_TOKEN: &str = "SKIPPED";

/// Channel type cache queries
type CacheCheckChannel = mpsc::Receiver<Result<(String, (String, bool)), String>>;

/// Discover and build flake stuff for CI
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Discover flake outputs
    Discover {
        #[clap(flatten)]
        args: DiscoverArgs,
    },
}

#[derive(Parser)]
struct DiscoverArgs {
    /// Search the flake for this output type
    #[clap(long)]
    prefix: String,

    /// Also descend into the specified system sub-attributes
    #[clap(long)]
    systems: Option<String>,

    /// Filter out these outputs
    #[clap(long)]
    filter: Option<String>,

    /// Check binary cache first
    #[clap(long)]
    check: Option<String>,

    /// Authorization for attic binary cache
    #[clap(long)]
    auth: Option<String>,

    /// Return attribute set with names and hashes
    #[clap(long)]
    with_hashes: bool,
}


/// Run a nix command and return its output
fn nix(args: &[&str]) -> Result<String, String> {
    // eprintln!("$ nix {:?}", args);
    let cmd_out = process::Command::new("nix")
        .args(args)
        .stdin(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .map_err(|e| format!("Nix failed ({})", e))?;

    String::from_utf8(cmd_out.stdout)
        .map_err(|e| format!("Unable to decode Nix output ({})", e))
}

/// Parse a serialized json object
fn parse<'a, T: Deserialize<'a>>(s: &'a str) -> Result<T, String> {
    serde_json::from_str(s)
        .map_err(|e| format!("Unable to parse json ({})", e))
}

/// Fill the `DISCOVER_NIX_FUNC` template for flake discover
fn nix_discover_func(label: &str, blocklist: Option<&str>) -> String {
    let prefix_str = format!("\"{}\"", label);
    let skip_str = format!("\"{}\"", SKIP_TOKEN);
    let blocklist_str = match blocklist {
        Some(list) => format!("\"{}\"", list.replace("\"", "\\\"")),
        None => "\"[]\"".to_owned(),
    };

    DISCOVER_NIX_FUNC.replace("PREFIX", &prefix_str)
        .replace("SKIP_TOKEN", &skip_str)
        .replace("BLOCKLIST", &blocklist_str)
}

/// Check `cache` for derivations.
///
/// `outputs` is a map from output labels to hashes
///
/// returns a channel over which the responses are communicated
fn check_cache_for_all(outputs: HashMap<String, String>, cache: &str, auth: Option<String>) -> CacheCheckChannel {
    let (tx, rx) = mpsc::channel();
    let request_pool = ThreadPool::new(CACHE_CHECK_THREADS);
    let cache = cache.to_owned();

    for (output, hash) in outputs {
        let tx = tx.clone();
        let cache = cache.clone();
        let auth = auth.clone();
        request_pool.execute(move || {
            let is_cached = check_cache(&hash, &cache, auth);
            tx.send(is_cached.map(|c| (output, (hash, c)))).unwrap();
        });
    }

    rx
}

/// Check binary cache (`cache`) for hash (`hash`) while optionally authenticating with `auth`
fn check_cache(hash: &str, cache: &str, auth: Option<String>) -> Result<bool, String> {
    let request = format!("{}/{}.narinfo", cache, hash);

    let response = if let Some(token) = auth {
        ureq::get(request)
            .header("authorization", format!("bearer {}", token))
            .call()
    } else {
        ureq::get(request)
            .call()
    };

    match response {
        Ok(_) => Ok(true),
        Err(e) => match e {
            ureq::Error::StatusCode(404) => Ok(false),
            e => Err(format!("Unable to query binary cache ({})", e)),
        },
    }
}

/// Resolve any errors by displaying an error message and exiting
fn resolve<T, E: Display>(result: Result<T, E>) -> T {
    match result {
        Ok(t) => t,
        Err(e) => { eprintln!("Error: {}", e); process::exit(1) },
    }
}

fn discover(args: DiscoverArgs) -> Result<(), String> {
    let mut unchecked_attrs = HashMap::new();

    let output_labels: Vec<_> = match args.systems {
        Some(systems) => parse::<Vec<String>>(&systems)?
            .into_iter()
            .map(|s| format!("{}.{}", args.prefix, s))
            .collect(),
        None => vec!(args.prefix),
    };

    for output_label in output_labels {
        let flake_ref = format!(".#{}", output_label);
        let func = nix_discover_func(&output_label, args.filter.as_deref());
        let output = nix(&[
            "eval",
            &flake_ref,
            "--apply",
            &func,
            "--json",
            "--quiet"
        ]).unwrap_or("[]".to_owned());
        let parsed = parse::<HashMap<String, String>>(&output)
            .unwrap_or_default();
        unchecked_attrs.extend(parsed);
    }

    unchecked_attrs.retain(|k, v| {
        if v == SKIP_TOKEN {
            eprintln!("[SKIPPED]\t{}", k);
            false
        } else {
            true
        }
    });

    let mut attrs = HashMap::new();

    if let Some(cache) = &args.check {
        let cached_channel = check_cache_for_all(unchecked_attrs, cache, args.auth);
        for attr_result in cached_channel {
            let (attr, (hash, is_cached)) = attr_result?;
            if is_cached {
                eprintln!("[CACHED] \t{}", attr);
            } else {
                eprintln!("[BUILD]  \t{}", attr);
                attrs.insert(attr, hash);
            }
        }
    } else {
        attrs = unchecked_attrs;
        for attr in attrs.keys() {
            eprintln!("[BUILD]  \t{}", attr);
        }
    }

    let s = if args.with_hashes {
        serde_json::to_string(&attrs)
            .map_err(|e| format!("Unable to encode result ({})", e))?
    } else {
        serde_json::to_string(&attrs.keys().collect::<Vec<_>>())
            .map_err(|e| format!("Unable to encode result ({})", e))?
    };
    println!("{}", s);
    Ok(())
}

fn main() {
    let args = Args::parse();

    use Command::*;
    let result = match args.command {
        Discover { args } => discover(args),
    };
    resolve(result);
}
