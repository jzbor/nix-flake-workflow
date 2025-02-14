use clap::{Parser, Subcommand};
use serde::Deserialize;
use threadpool::ThreadPool;
use std::fmt::Display;
use std::process;
use std::sync::mpsc;


const CACHE_CHECK_THREADS: usize = 16;

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

fn discover(prefix: String, systems: Option<String>, filter: Option<String>, check: Option<String>, auth: Option<String>) -> Result<(), String> {
    let mut unchecked_attrs = Vec::new();

    let filter: Vec<String> = match &filter {
        Some(s) => parse(s)?,
        None => Vec::new(),
    };

    if let Some(systems) = systems {
        let systems: Vec<String> = parse(&systems)?;

        for system in systems {
            let search_path = format!(".#{}.{}", prefix, system);
            let func = format!("x: map (x: \"{}.{}.\" + x) (builtins.attrNames x)", prefix, system);
            let output = nix(&[
                "eval",
                &search_path,
                "--apply",
                &func,
                "--json",
                "--quiet"
            ])?;
            unchecked_attrs.extend(parse::<Vec<String>>(&output)?);
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
        ])?;
        unchecked_attrs.extend(parse::<Vec<String>>(&output)?);
    }

    unchecked_attrs.retain(|a| {
        if filter.contains(&a) {
            eprintln!("  [SKIPPED]\t{}", a);
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
                eprintln!("  [CACHED] \t{}", attr);
            } else {
                eprintln!("  [FOUND]  \t{}", attr);
                attrs.push(attr)
            }
        }
    } else {
        for attr in unchecked_attrs {
            eprintln!("  [FOUND]  \t{}", attr);
            attrs.push(attr)
        }
    }

    let s = serde_json::to_string(&attrs)
        .map_err(|e| format!("Unable to encode result ({})", e))?;
    println!("{}", s);
    Ok(())
}

fn check_cache_for_all(outputs: Vec<String>, cache: &str, auth: Option<String>) -> mpsc::Receiver<Result<(String, bool), String>> {
    let (tx, rx) = mpsc::channel();
    let pool = ThreadPool::new(CACHE_CHECK_THREADS);

    for output in outputs.into_iter() {
        let tx = tx.clone();
        let cache = cache.to_owned();
        let auth = auth.clone();
        pool.execute(move || {
            let is_cached = check_cache(&output, &cache, auth);
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

fn check_cache(output: &str, cache: &str, auth: Option<String>) -> Result<bool, String> {
    let hash = calc_hash(output)?;
    let request = format!("{}/{}.narinfo", cache, hash);
    eprintln!("Checking {} for {} ({})", cache, output, hash);
    let response = if let Some(token) = auth {
        ureq::get(request)
            .header("authorization", format!("bearer {}", token))
            .call()
            .map_err(|e| format!("Unable to check binary cache ({})", e))?
    } else {
        ureq::get(request)
            .call()
            .map_err(|e| format!("Unable to check binary cache ({})", e))?
    };

    Ok(response.status() == ureq::http::status::StatusCode::OK)
}

fn check(output: String, cache: String) -> Result<(), String> {
    let path = check_cache(&output, &cache, None)?;
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
