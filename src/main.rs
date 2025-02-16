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

/// Discover and build flake stuff for CI
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Discover flake outputs
    Discover {
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
    },

    Path {
        #[clap(long)]
        output: String,
    },

    Hash {
        #[clap(long)]
        output: String,
    },

    Check {
        #[clap(long)]
        output: String,

        #[clap(long)]
        cache: String,
    }
}

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

fn parse<'a, T: Deserialize<'a>>(s: &'a str) -> Result<T, String> {
    serde_json::from_str(s)
        .map_err(|e| format!("Unable to parse json ({})", e))
}

fn nix_discover_func(prefix: String, system: Option<String>, blocklist: Option<&str>) -> String {
    let prefix_str = match system {
        Some(sys) => format!("\"{}.{}\"", prefix, sys),
        None => format!("\"{}\"", prefix),
    };
    let skip_str = format!("\"{}\"", SKIP_TOKEN);
    let blocklist_str = match blocklist {
        Some(list) => format!("\"{}\"", list.replace("\"", "\\\"")),
        None => "\"[]\"".to_owned(),
    };

    DISCOVER_NIX_FUNC.replace("PREFIX", &prefix_str)
        .replace("SKIP_TOKEN", &skip_str)
        .replace("BLOCKLIST", &blocklist_str)
}

fn discover(prefix: String, systems: Option<String>, filter: Option<String>, check: Option<String>, auth: Option<String>) -> Result<(), String> {
    let mut unchecked_attrs = HashMap::new();

    if let Some(systems) = systems {
        let systems: Vec<String> = parse(&systems)?;

        for system in systems {
            let search_path = format!(".#{}.{}", prefix, system);
            let func = nix_discover_func(format!("{}.{}", prefix, system), Some(system), filter.as_deref());
            let output = nix(&[
                "eval",
                &search_path,
                "--apply",
                &func,
                "--json",
                "--quiet"
            ]).unwrap_or("[]".to_owned());
            let parsed = parse::<HashMap<String, String>>(&output)
                .unwrap_or(HashMap::new());
            unchecked_attrs.extend(parsed);
        }
    } else {
        let search_path = format!(".#{}", prefix);
        let func = format!("builtins.attrNames");
        let output = nix(&[
            "eval",
            &search_path,
            "--apply",
            &func,
            "--json",
            "--quiet"
        ]).unwrap_or("[]".to_owned());
        let parsed = parse::<HashMap<String, String>>(&output)
            .unwrap_or(HashMap::new());
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

    let mut attrs = Vec::new();

    if let Some(cache) = &check {
        let cached_channel = check_cache_for_all(unchecked_attrs, cache, auth);
        for attr_result in cached_channel {
            let (attr, is_cached) = attr_result?;
            if is_cached {
                eprintln!("[CACHED] \t{}", attr);
            } else {
                eprintln!("[FOUND]  \t{}", attr);
                attrs.push(attr)
            }
        }
    } else {
        for (attr, _) in unchecked_attrs {
            eprintln!("[FOUND]  \t{}", attr);
            attrs.push(attr)
        }
    }

    let s = serde_json::to_string(&attrs)
        .map_err(|e| format!("Unable to encode result ({})", e))?;
    println!("{}", s);
    Ok(())
}

fn check_cache_for_all(outputs: HashMap<String, String>, cache: &str, auth: Option<String>)
        -> mpsc::Receiver<Result<(String, bool), String>> {
    let (tx, rx) = mpsc::channel();
    let request_pool = ThreadPool::new(CACHE_CHECK_THREADS);
    let cache = cache.to_owned();

    for (output, hash) in outputs {
        eprintln!("   (Checking {} for {} at {})", cache, output, hash);
        let tx = tx.clone();
        let cache = cache.clone();
        let auth = auth.clone();
        request_pool.execute(move || {
            let is_cached = check_cache(&hash, &cache, auth);
            tx.send(is_cached.map(|c| (output, c))).unwrap();
        });
    }

    rx
}

fn calc_path(output: &str) -> Result<String, String> {
    let flake_ref = format!(".#{}", output);
    let output = nix(&[
        "eval",
        &flake_ref,
        "--json",
        "--quiet"
    ])?;
    parse(&output)
}

fn calc_hash(output: &str) -> Result<String, String> {
    let path = calc_path(output)?;
     path.replace("/nix/store/", "")
        .split('-')
        .next()
        .ok_or("Cannot derive path from malformed store path".to_owned())
        .map(String::from)
}

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

fn check(output: String, cache: String) -> Result<(), String> {
    let hash = calc_hash(&output)?;
    let path = check_cache(&hash, &cache, None)?;
    println!("{:?}", path);
    Ok(())
}

fn path(output: String) -> Result<(), String> {
    let path = calc_path(&output)?;
    println!("{}", path);
    Ok(())
}

fn hash(output: String) -> Result<(), String> {
    let path = calc_hash(&output)?;
    println!("{}", path);
    Ok(())
}

fn resolve<T, E: Display>(result: Result<T, E>) -> T {
    match result {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1)
        },
    }
}

fn main() {
    let args = Args::parse();

    use Command::*;
    let result = match args.command {
        Discover { prefix, systems, filter, check, auth } => discover(prefix, systems, filter, check, auth),
        Check { output, cache } => check(output, cache),
        Path { output } => path(output),
        Hash { output } => hash(output),
    };
    resolve(result);
}
