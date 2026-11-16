//! Snapshot tests: what the page parsing helpers return for every saved page
//! under `tests/fixtures` and for generated input, recorded under
//! `tests/snapshots` before their regular expressions were compiled once, so
//! any change in what they return fails a test.
//!
//! `GOOSE_EGGS_BLESS=1 cargo test` rewrites the snapshots from the current code.

use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};

// Every saved page, as (path under tests/fixtures, contents), sorted by path.
pub(crate) fn fixtures() -> Vec<(String, String)> {
    fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, found);
            } else {
                found.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut paths = Vec::new();
    walk(&root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (name, fs::read_to_string(&path).unwrap())
        })
        .collect()
}

// One snapshot line: a long result is kept as its length, hash and start.
pub(crate) fn line(input: &str, result: impl Debug) -> String {
    let result = format!("{:?}", result);
    if result.len() <= 200 {
        format!("{}\t{}\n", input, result)
    } else {
        let start: String = result.chars().take(60).collect();
        format!(
            "{}\tlen={} fnv={:016x} start={}\n",
            input,
            result.len(),
            fnv(&result),
            start
        )
    }
}

// FNV-1a, a hash that never changes between Rust releases.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x100000001b3)
    })
}

// Compare `actual` with the snapshot `name`, or record it when blessing.
pub(crate) fn check(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{}.txt", name));
    if std::env::var_os("GOOSE_EGGS_BLESS").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, actual).unwrap();
        return;
    }
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} snapshot missing: {}: {}", name, path.display(), e));
    if expected != actual {
        let expected_lines: Vec<&str> = expected.lines().collect();
        let actual_lines: Vec<&str> = actual.lines().collect();
        let differs = (0..expected_lines.len().max(actual_lines.len()))
            .find(|&i| expected_lines.get(i) != actual_lines.get(i))
            .unwrap_or(0);
        panic!(
            "{} snapshot differs at line {}:\n  expected: {:?}\n  actual:   {:?}",
            name,
            differs + 1,
            expected_lines.get(differs),
            actual_lines.get(differs)
        );
    }
}

// A xorshift generator, so generated input is the same on every machine and
// with every version of every dependency.
pub(crate) struct Generator(u64);

impl Generator {
    pub(crate) fn new(seed: u64) -> Self {
        Generator(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    // One of `items`.
    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next() % items.len() as u64) as usize]
    }

    // Up to `most` of `tokens`, joined.
    pub(crate) fn text(&mut self, tokens: &[&str], most: u64) -> String {
        let count = self.next() % (most + 1);
        (0..count).map(|_| *self.pick(tokens)).collect()
    }
}
