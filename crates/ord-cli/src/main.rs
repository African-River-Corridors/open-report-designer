//! The `ord` command (spec 0001 §11.2): the input and output around the pure core.
//!
//! Exit codes: 0 done (warnings printed), 1 the document has errors, 2 a usage or input-output error.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ord::{Finding, Format, Register};

#[derive(Parser)]
#[command(
    name = "ord",
    version,
    about = "Open Report Designer: one typed report document to PDF, HTML and Word"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render a document to PDF, HTML, Word, or all three.
    Render {
        /// The document (JSON).
        document: PathBuf,
        /// pdf, html, docx or all.
        #[arg(long, short, default_value = "pdf")]
        format: String,
        /// A register of document classes that replaces the built-in one.
        #[arg(long)]
        register: Option<PathBuf>,
        /// The folder to write into (default: the current folder). Files are named after the document.
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
    /// Check a document and print its findings.
    Check {
        document: PathBuf,
        #[arg(long)]
        register: Option<PathBuf>,
        /// Print the findings as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Build one document from a manifest and a folder of chapter files.
    Assemble {
        manifest: PathBuf,
        /// The folder of chapter files (*.json, searched recursively). Chapters are found by id.
        #[arg(long)]
        chapters: PathBuf,
        /// The document to write.
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long)]
        register: Option<PathBuf>,
    },
    /// Print the JSON Schema of a document.
    Schema,
    /// Print the register of document classes (the built-in one, or the one given).
    Register {
        #[arg(long)]
        register: Option<PathBuf>,
    },
}

/// Why the command stopped.
enum Fail {
    /// The document has errors (exit 1).
    Findings(Vec<Finding>),
    /// Usage or input-output (exit 2).
    Io(String),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail::Findings(f)) => {
            print_findings(&f);
            ExitCode::from(1)
        }
        Err(Fail::Io(msg)) => {
            eprintln!("ord: {msg}");
            ExitCode::from(2)
        }
    }
}

fn run(cmd: Command) -> Result<(), Fail> {
    match cmd {
        Command::Render { document, format, register, out } => {
            let formats: Vec<Format> =
                if format.eq_ignore_ascii_case("all") {
                    Format::ALL.to_vec()
                } else {
                    vec![Format::parse(&format).ok_or_else(|| {
                        Fail::Io(format!("unknown format {format:?}: pdf, html, docx or all"))
                    })?]
                };
            let register = load_register(register.as_deref())?;
            let (doc, assets) = load_document(&document)?;
            let out = out.unwrap_or_else(|| PathBuf::from("."));
            std::fs::create_dir_all(&out)
                .map_err(|e| Fail::Io(format!("cannot make {}: {e}", out.display())))?;
            let stem = document.file_stem().and_then(|s| s.to_str()).unwrap_or("report");
            for f in formats {
                let o = ord::render(&doc, &assets, &register, f).map_err(Fail::Findings)?;
                print_findings(&o.warnings);
                let path = out.join(format!("{stem}.{}", f.extension()));
                std::fs::write(&path, &o.bytes)
                    .map_err(|e| Fail::Io(format!("cannot write {}: {e}", path.display())))?;
                println!("{}", path.display());
            }
            Ok(())
        }
        Command::Check { document, register, json } => {
            let register = load_register(register.as_deref())?;
            let (doc, assets) = load_document(&document)?;
            let f = ord::check(&doc, &assets, &register);
            if json {
                println!("{}", serde_json::to_string_pretty(&f).expect("findings serialise"));
            } else if f.is_empty() {
                println!("{}: no findings", document.display());
            } else {
                print_findings(&f);
            }
            if ord::finding::has_errors(&f) { Err(Fail::Findings(Vec::new())) } else { Ok(()) }
        }
        Command::Assemble { manifest, chapters, out, register } => {
            let register = load_register(register.as_deref())?;
            let text = read(&manifest)?;
            let mut m: ord::assemble::Manifest =
                ord::parse::parse_as(&text).map_err(|f| Fail::Findings(in_file(&manifest, f)))?;
            let mut found = Vec::new();
            for path in json_files(&chapters)? {
                let c: ord::assemble::Chapter =
                    ord::parse::parse_as(&read(&path)?).map_err(|f| Fail::Findings(in_file(&path, f)))?;
                found.push(c);
            }
            // Asset paths are relative to the manifest; rewrite them relative to the new document.
            let to = out.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
            std::fs::create_dir_all(to)
                .map_err(|e| Fail::Io(format!("cannot make {}: {e}", to.display())))?;
            let real = |d: &Path| {
                std::fs::canonicalize(d).map_err(|e| Fail::Io(format!("cannot read {}: {e}", d.display())))
            };
            let from =
                real(manifest.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new(".")))?;
            let to = real(to)?;
            for a in m.assets.values_mut() {
                if let Some(p) = &a.path {
                    a.path = Some(rebase(p, &from, &to));
                }
            }
            let doc = ord::assemble(&m, &found, &register).map_err(Fail::Findings)?;
            let mut json = serde_json::to_string_pretty(&doc).expect("a document serialises");
            json.push('\n');
            if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir)
                    .map_err(|e| Fail::Io(format!("cannot make {}: {e}", dir.display())))?;
            }
            std::fs::write(&out, json)
                .map_err(|e| Fail::Io(format!("cannot write {}: {e}", out.display())))?;
            println!("{}", out.display());
            Ok(())
        }
        Command::Schema => {
            print!("{}", ord::schema());
            Ok(())
        }
        Command::Register { register } => {
            let r = load_register(register.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&r).expect("a register serialises"));
            Ok(())
        }
    }
}

fn read(path: &Path) -> Result<String, Fail> {
    std::fs::read_to_string(path).map_err(|e| Fail::Io(format!("cannot read {}: {e}", path.display())))
}

fn load_register(path: Option<&Path>) -> Result<Register, Fail> {
    match path {
        None => Ok(ord::builtin_register()),
        Some(p) => Register::from_json(&read(p)?).map_err(|f| Fail::Findings(in_file(p, f))),
    }
}

/// Reads a document and the bytes of every `path` asset, relative to the document's folder.
fn load_document(path: &Path) -> Result<(ord::Document, ord::Assets), Fail> {
    let doc = ord::parse(&read(path)?).map_err(|f| Fail::Findings(in_file(path, f)))?;
    let dir = path.parent().unwrap_or(Path::new(""));
    let mut supplied = BTreeMap::new();
    for (id, a) in &doc.assets {
        if let Some(p) = &a.path {
            let full = dir.join(p);
            let bytes = std::fs::read(&full)
                .map_err(|e| Fail::Io(format!("cannot read asset {id:?} at {}: {e}", full.display())))?;
            supplied.insert(id.clone(), bytes);
        }
    }
    let (assets, findings) = ord::assets::collect(&doc, &supplied);
    if findings.is_empty() { Ok((doc, assets)) } else { Err(Fail::Findings(findings)) }
}

fn in_file(path: &Path, findings: Vec<Finding>) -> Vec<Finding> {
    findings
        .into_iter()
        .map(|mut f| {
            f.path = format!("{}:{}", path.display(), f.path);
            f
        })
        .collect()
}

fn print_findings(f: &[Finding]) {
    for x in f {
        eprintln!("{x}");
    }
}

/// Every `*.json` file under a folder, sorted, so the order never depends on the file system.
fn json_files(dir: &Path) -> Result<Vec<PathBuf>, Fail> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let entries =
            std::fs::read_dir(&d).map_err(|e| Fail::Io(format!("cannot read {}: {e}", d.display())))?;
        for e in entries {
            let p = e.map_err(|e| Fail::Io(e.to_string()))?.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "json") {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

/// `p`, relative to the folder `from`, rewritten relative to the folder `to`. Both folders are canonical
/// (absolute, links resolved), so `..` steps are true.
fn rebase(p: &str, from: &Path, to: &Path) -> String {
    if Path::new(p).is_absolute() {
        return p.to_string();
    }
    let target = normal(&from.join(p));
    let base = normal(to);
    let t: Vec<Component> = target.components().collect();
    let b: Vec<Component> = base.components().collect();
    let common = t.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut rel = PathBuf::new();
    for _ in common..b.len() {
        rel.push("..");
    }
    for c in &t[common..] {
        rel.push(c);
    }
    rel.to_string_lossy().replace('\\', "/")
}

/// A path with `.` and `..` worked out, without touching the file system.
fn normal(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}
