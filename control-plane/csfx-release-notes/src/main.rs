use anyhow::{bail, ensure, Context, Result};
use csfx_release_notes::{draft, parse, to_markdown, Block, Commit};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::{env, fs};

const USAGE: &str = "usage: csfx-release-notes build <version> --asset-url <url> [--dir releases] [--out .] | draft <version> [--since <tag>] [--dir releases]";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[ERROR] release notes failed error=\"{error:#}\"");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("build") => build(&args[1..]),
        Some("draft") => write_draft(&args[1..]),
        _ => bail!(USAGE),
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

fn version_arg(args: &[String]) -> Result<&str> {
    let version = args.first().map(String::as_str).context(USAGE)?;
    Ok(version.trim_start_matches('v'))
}

fn build(args: &[String]) -> Result<()> {
    let version = version_arg(args)?;
    let asset_url = flag(args, "--asset-url").context(USAGE)?;
    let dir = Path::new(flag(args, "--dir").unwrap_or("releases"));
    let out = Path::new(flag(args, "--out").unwrap_or("."));
    let path = dir.join(format!("{version}.md"));
    let source = fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let notes = parse(&source).with_context(|| format!("invalid {}", path.display()))?;
    ensure!(notes.version == version, "version {} does not match file name {version}", notes.version);
    ensure_images_exist(&notes, &dir.join("assets").join(version))?;
    fs::write(out.join("release-notes.json"), serde_json::to_string_pretty(&notes)?)?;
    fs::write(out.join("release-notes.md"), to_markdown(&notes, asset_url))?;
    println!("[INFO] release notes built version={version} channel={:?}", notes.channel);
    Ok(())
}

fn ensure_images_exist(notes: &csfx_release_notes::ReleaseNotes, asset_dir: &Path) -> Result<()> {
    for block in notes.sections.values().flatten() {
        if let Block::Image { src, .. } = block {
            let file: PathBuf = asset_dir.join(src);
            ensure!(file.is_file(), "missing image {}", file.display());
        }
    }
    Ok(())
}

fn write_draft(args: &[String]) -> Result<()> {
    let version = version_arg(args)?;
    let dir = Path::new(flag(args, "--dir").unwrap_or("releases"));
    let path = dir.join(format!("{version}.md"));
    ensure!(!path.exists(), "{} already exists", path.display());
    let since = match flag(args, "--since") {
        Some(tag) => tag.to_string(),
        None => previous_tag(version)?,
    };
    let commits = commits_since(&since)?;
    let date = git(&["log", "-1", "--format=%cs"])?;
    fs::create_dir_all(dir)?;
    fs::write(&path, draft(version, date.trim(), &commits)?)?;
    println!("[INFO] release notes draft written path={} since={since} commits={}", path.display(), commits.len());
    Ok(())
}

fn previous_tag(version: &str) -> Result<String> {
    let mut args = vec!["describe", "--tags", "--abbrev=0"];
    if !version.contains('-') {
        args.extend(["--exclude", "*-*"]);
    }
    Ok(git(&args)?.trim().to_string())
}

fn commits_since(tag: &str) -> Result<Vec<Commit>> {
    let range = format!("{tag}..HEAD");
    let log = git(&["log", "--format=%s%x1f%b%x1e", &range])?;
    Ok(log
        .split('\x1e')
        .filter_map(|record| record.trim().split_once('\x1f'))
        .map(|(subject, body)| Commit {
            subject: subject.trim().to_string(),
            body: body.to_string(),
        })
        .collect())
}

fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git").args(args).output().context("cannot run git")?;
    ensure!(output.status.success(), "git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr).trim());
    Ok(String::from_utf8(output.stdout)?)
}
