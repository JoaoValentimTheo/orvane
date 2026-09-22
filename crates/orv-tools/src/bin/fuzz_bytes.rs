//! Byte-level fuzz harness for the whole Orvane pipeline.
//!
//! Feeds arbitrary bytes through `lex` -> parse -> check -> run and asserts that
//! nothing panics or aborts. Strings are the primary input because that is what
//! a real fuzzer produces; a deterministic pseudo-random generator keeps runs
//! reproducible without a dependency (only the crates in SPEC §3.3 are allowed).
//!
//! This is the 0.1.1 work queue: run it with a large count, fix what it finds.
//!
//! Usage: `cargo run --release -p orv-tools --bin fuzz-bytes -- [iterations]`

use orv_runtime::driver;
use orv_sema::check as sema_check;
use orv_syntax::{FileId, Parser, SourceFile, SourceMap, lex};

fn main() {
    let iterations: u64 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(10_000);

    // A 64-bit xorshift: deterministic, dependency-free, good enough to explore
    // the input space.
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    // Fragments that reach the interesting code paths far more often than
    // random bytes do, mixed with raw bytes.
    const FRAGMENTS: [&str; 48] = [
        "fn ", "main", "(", ")", "{", "}", "[", "]", "#{", ",", ":", ".", "..", "..=", "=>", "->",
        "?", "?.", "??", "=", "==", "+", "-", "*", "/", "%", "as", "let", "mut", "if", "else",
        "match", "for", "in", "while", "return", "break", "continue", "fail", "try", "data",
        "enum", "true", "false", "none", "and", "or", "not",
    ];

    for iteration in 0..iterations {
        let mut text = String::new();
        let pieces = (next() % 40) as usize;
        for _ in 0..pieces {
            let roll = next() % 100;
            if roll < 60 {
                text.push_str(FRAGMENTS[(next() % FRAGMENTS.len() as u64) as usize]);
            } else if roll < 80 {
                // A random byte, which may or may not be valid UTF-8.
                let byte = (next() % 256) as u8;
                text.push(byte as char);
            } else {
                text.push_str(&format!("{}", next() % 1000));
            }
            if next() % 4 == 0 {
                text.push('\n');
            }
        }
        exercise(&text);
        if iteration % 10_000 == 0 && iteration > 0 {
            eprintln!("{iteration} iterations");
        }
    }
    eprintln!("{iterations} iterations completed without a panic");
}

/// Pushes `text` through every stage, asserting only that nothing panics.
fn exercise(text: &str) {
    let mut sources = SourceMap::new();
    let id = sources.add("fuzz.orv", text);
    let file = match sources.file(id).cloned() {
        Some(file) => file,
        None => SourceFile::new(FileId(0), "fuzz.orv", text),
    };

    let (tokens, lexical) = lex(&file);
    let mut parser = Parser::new(&tokens, lexical);
    let parsed = parser.parse_program();

    let Some(program) = parsed.program else {
        return;
    };
    // Sema and the runtime are only reached when the front end produced an AST;
    // the diagnostics themselves are not asserted on, only that no stage
    // panics.
    let _ = sema_check(&program);
    let _ = driver::run(&program);
}
