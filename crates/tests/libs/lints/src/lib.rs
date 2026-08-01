//! Reading the framework's own source, so a rule no runtime test can catch has somewhere
//! to live.
//!
//! Three decisions shape everything here, and each of them is a class of false positive
//! that would otherwise make these rules unusable:
//!
//! - **Generated files are not audited.** The exempt set is not a filename heuristic; it
//!   is read from the binding tools' own `--out` declarations, so nothing can be smuggled
//!   into it by being called `bindings.rs`.
//! - **Comments are stripped before matching.** A rule that fires on the sentence
//!   explaining the rule is worse than no rule.
//! - **Test modules are stripped before matching.** A test may legitimately construct what
//!   production must not — quantization is tested by handing it out-of-range values, and
//!   forbidding that would be forbidding the test.

use std::path::{Path, PathBuf};

/// One hand-written source file, ready to match against.
pub struct Source {
    /// Path relative to the workspace root, with forward slashes. What a failure prints,
    /// and what an allowlist names.
    pub path: String,
    /// The file with comments and test modules removed.
    pub code: String,
    /// The file as written. What a line number is resolved against.
    pub raw: String,
}

impl Source {
    /// The one-based line `at` falls on, in the original file.
    ///
    /// The stripped text keeps every newline, so an offset into it is an offset into the
    /// original — which is what lets a failure name a line a reader can open.
    #[must_use]
    pub fn line(&self, at: usize) -> usize {
        self.code[..at].matches('\n').count() + 1
    }

    /// Every occurrence of `needle` in the stripped text, as `(line, the line's text)`.
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

    /// Whether the file's path starts with any of `prefixes`.
    #[must_use]
    pub fn under(&self, prefixes: &[&str]) -> bool {
        prefixes.iter().any(|prefix| self.path.starts_with(prefix))
    }
}

/// The workspace root, derived from this crate's own location.
#[must_use]
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("the lints crate sits four levels below the workspace root")
        .to_path_buf()
}

/// The crates these rules govern: the UI framework, and nothing upstream of it.
///
/// Upstream's own crates are not ours to constrain, and a rule that fired on them would be
/// a rule nobody could act on.
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

/// Every hand-written `.rs` file in the framework crates.
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

/// The files the binding tools declare they produce, relative to the workspace root.
///
/// Read from the filters rather than guessed, so the exemption is exactly what the tools
/// claim and a hand-written file cannot join it by being named `bindings.rs`.
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
    // `tool_composition` writes two files from one filter and names them in its own
    // source rather than on an `--out` line. Read them from there for the same reason the
    // rest are read from the filters: the tool is the declaration.
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

/// Blanks out comments, preserving every byte position and every newline.
///
/// Position-preserving rather than removing, so a match offset still resolves to the line
/// it was written on.
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
                // A raw string's hashes have to be counted, or its closing quote is missed
                // and everything after it reads as a string.
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

/// Blanks out `#[cfg(test)]` modules, preserving newlines.
///
/// A test may construct what production must not: quantization is tested by handing it
/// out-of-range values, and a rule that forbade that would be forbidding the test.
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

/// Fails with every hit listed, or passes silently.
///
/// One assertion per rule with the full list, rather than one per hit: a rule that stops at
/// the first violation is a rule you fix one recompile at a time.
pub fn deny(rule: &str, why: &str, hits: &[String]) {
    assert!(
        hits.is_empty(),
        "\n{rule} — {why}\n\n{}\n\n{} violation(s).\n",
        hits.join("\n"),
        hits.len()
    );
}
