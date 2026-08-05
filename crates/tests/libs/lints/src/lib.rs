//! Collects the framework crates' hand-written Rust source and matches source rules
//! against it.
//!
//! Three exclusions bound what a rule sees, so it fires only on production code:
//!
//! - Generated files are excluded. The exempt set is read from the binding tools' own
//!   `--out` declarations, so a hand-written file cannot join it by being called
//!   `bindings.rs`.
//! - Comments are blanked before matching, so a rule does not fire on the prose that
//!   describes it.
//! - `#[cfg(test)]` modules are blanked before matching, because a test constructs the
//!   values production is forbidden to construct — quantization is tested by handing it
//!   out-of-range inputs.

use std::path::{Path, PathBuf};

/// A hand-written source file, stripped and ready to match against.
pub struct Source {
    /// Path relative to the workspace root, with forward slashes. Failure messages print
    /// this path and allowlists name it.
    pub path: String,
    /// The file with comments and `#[cfg(test)]` modules blanked out.
    pub code: String,
    /// The file as written. Line numbers resolve against this text.
    pub raw: String,
}

impl Source {
    /// Returns the one-based line that byte offset `at` falls on.
    ///
    /// [`Source::code`] blanks comments and test modules in place rather than removing
    /// them, so it keeps every newline of [`Source::raw`] and a line number found in one
    /// names the same line in the other.
    #[must_use]
    pub fn line(&self, at: usize) -> usize {
        self.code[..at].matches('\n').count() + 1
    }

    /// Returns every occurrence of `needle` in [`Source::code`] as `(line, text)`, where
    /// `text` is that line of [`Source::raw`], trimmed.
    #[must_use]
    pub fn find(&self, needle: &str) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        let mut at = 0;
        while let Some(found) = self.code[at..].find(needle) {
            let offset = at + found;
            let line = self.line(offset);
            out.push((
                line,
                self.raw
                    .lines()
                    .nth(line - 1)
                    .unwrap_or("")
                    .trim()
                    .to_owned(),
            ));
            at = offset + needle.len();
        }
        out
    }

    /// Returns `true` when [`Source::path`] starts with any of `prefixes`.
    #[must_use]
    pub fn under(&self, prefixes: &[&str]) -> bool {
        prefixes.iter().any(|prefix| self.path.starts_with(prefix))
    }
}

/// Returns the workspace root, derived from this crate's manifest directory.
///
/// # Panics
///
/// Panics if this crate does not sit four directories below the workspace root.
#[must_use]
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("the lints crate sits four levels below the workspace root")
        .to_path_buf()
}

/// The crate directories these rules govern, relative to the workspace root.
///
/// The set covers the UI framework crates only. Crates outside it are out of scope for
/// every rule.
pub const FRAMEWORK: [&str; 8] = [
    "crates/libs/color",
    "crates/libs/composition",
    "crates/libs/d2d",
    "crates/libs/present",
    "crates/libs/scene",
    "crates/libs/text",
    "crates/libs/ui",
    "crates/libs/window",
];

/// Returns every hand-written `.rs` file under [`FRAMEWORK`], with the files listed by
/// [`generated`] excluded.
///
/// # Panics
///
/// Panics if no source file is found, which means the crate directories were not located.
#[must_use]
pub fn framework() -> Vec<Source> {
    let root = root();
    let generated = generated();
    let mut out = Vec::new();
    for crate_dir in FRAMEWORK {
        collect(
            &root,
            &root.join(crate_dir).join("src"),
            &generated,
            &mut out,
        );
    }
    assert!(
        !out.is_empty(),
        "no framework source was found under {}",
        root.display()
    );
    out
}

/// Returns the output paths the binding tools declare, relative to the workspace root.
///
/// The paths come from the `--out` lines of the tools' filter files and from the string
/// literals in `tool_composition`'s own source, so the exempt set is exactly what the
/// tools declare rather than a filename pattern.
///
/// # Panics
///
/// Panics if fewer than eleven outputs are declared, which means the filter files were not
/// found.
#[must_use]
pub fn generated() -> Vec<String> {
    let root = root();
    let mut out = Vec::new();
    for tool in ["bindings", "composition"] {
        let dir = root.join("crates/tools").join(tool).join("src");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "txt") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                if let Some(declared) = line.strip_prefix("--out ") {
                    out.push(declared.trim().replace('\\', "/"));
                }
            }
        }
    }
    // `tool_composition` writes two files from one filter and names them in its own source
    // rather than on an `--out` line, so read those paths from its string literals.
    let dir = root.join("crates/tools/composition/src");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for literal in text.split('"').skip(1).step_by(2) {
                if literal.starts_with("crates/libs/") && literal.ends_with(".rs") {
                    out.push(literal.to_owned());
                }
            }
        }
    }
    assert!(
        out.len() > 10,
        "the binding filters declared only {} outputs, which means they were not found",
        out.len()
    );
    out
}

fn collect(root: &Path, dir: &Path, generated: &[String], out: &mut Vec<Source>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, generated, out);
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if generated.contains(&rel) {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        out.push(Source {
            code: strip_tests(&strip_comments(&raw)),
            path: rel,
            raw,
        });
    }
}

/// Replaces every comment with spaces, keeping each newline in place.
///
/// String, character and raw-string literals are copied through, so a `//` inside a
/// literal is not treated as a comment. Blanking rather than removing is what preserves
/// the line count ahead of any offset, so a match in the result names the line it was
/// written on.
#[must_use]
pub fn strip_comments(text: &str) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum In {
        Code,
        Line,
        Block(u32),
        Str,
        Raw(usize),
        Char,
    }

    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut state = In::Code;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied().unwrap_or(0);
        match state {
            In::Code => {
                // Count a raw string's hashes: without the count its closing quote is
                // missed and the rest of the file reads as one string.
                if b == b'r' && (next == b'"' || next == b'#') {
                    let mut hashes = 0;
                    while bytes.get(i + 1 + hashes) == Some(&b'#') {
                        hashes += 1;
                    }
                    if bytes.get(i + 1 + hashes) == Some(&b'"') {
                        out.push_str(&text[i..=i + 1 + hashes]);
                        i += hashes + 2;
                        state = In::Raw(hashes);
                        continue;
                    }
                }
                match (b, next) {
                    (b'/', b'/') => {
                        state = In::Line;
                        out.push_str("  ");
                        i += 2;
                        continue;
                    }
                    (b'/', b'*') => {
                        state = In::Block(1);
                        out.push_str("  ");
                        i += 2;
                        continue;
                    }
                    (b'"', _) => state = In::Str,
                    (b'\'', _) => state = In::Char,
                    _ => {}
                }
                out.push(b as char);
            }
            In::Line => {
                if b == b'\n' {
                    state = In::Code;
                    out.push('\n');
                } else {
                    out.push(' ');
                }
            }
            In::Block(depth) => {
                if b == b'/' && next == b'*' {
                    state = In::Block(depth + 1);
                    out.push_str("  ");
                    i += 2;
                    continue;
                }
                if b == b'*' && next == b'/' {
                    state = if depth == 1 {
                        In::Code
                    } else {
                        In::Block(depth - 1)
                    };
                    out.push_str("  ");
                    i += 2;
                    continue;
                }
                out.push(if b == b'\n' { '\n' } else { ' ' });
            }
            In::Str | In::Char => {
                if b == b'\\' {
                    out.push_str(&text[i..(i + 2).min(text.len())]);
                    i += 2;
                    continue;
                }
                if (state == In::Str && b == b'"') || (state == In::Char && b == b'\'') {
                    state = In::Code;
                }
                out.push(b as char);
            }
            In::Raw(hashes) => {
                if b == b'"' && bytes[i + 1..].iter().take(hashes).all(|h| *h == b'#') {
                    state = In::Code;
                    out.push_str(&text[i..=i + hashes]);
                    i += hashes + 1;
                    continue;
                }
                out.push(b as char);
            }
        }
        i += 1;
    }
    out
}

/// Replaces each `#[cfg(test)]` module with spaces, keeping newlines.
///
/// Rules match production code only: a test may construct the values a rule forbids, as
/// quantization is tested by handing it out-of-range inputs.
#[must_use]
pub fn strip_tests(code: &str) -> String {
    let mut out = code.to_owned();
    while let Some(at) = out.find("#[cfg(test)]") {
        let bytes = out.as_bytes();
        let Some(open) = out[at..].find('{').map(|off| at + off) else {
            break;
        };
        let mut depth = 0_i32;
        let mut end = out.len();
        for (i, b) in bytes.iter().enumerate().skip(open) {
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        let blanked: String = out[at..end]
            .chars()
            .map(|c| if c == '\n' { '\n' } else { ' ' })
            .collect();
        out.replace_range(at..end, &blanked);
    }
    out
}

/// Asserts that `hits` is empty, naming `rule` and `why` in the failure.
///
/// One assertion carries the whole list, so a run reports every violation of the rule
/// rather than stopping at the first.
///
/// # Panics
///
/// Panics when `hits` is not empty.
pub fn deny(rule: &str, why: &str, hits: &[String]) {
    assert!(
        hits.is_empty(),
        "\n{rule} — {why}\n\n{}\n\n{} violation(s).\n",
        hits.join("\n"),
        hits.len()
    );
}
