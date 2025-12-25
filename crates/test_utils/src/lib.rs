use std::sync::LazyLock;

use regex::Regex;

// TempDir uses https://docs.rs/fastrand/latest/fastrand/struct.Rng.html#method.alphanumeric
// This regex only matches on unix paths,
// will need to do something else if anyone ever runs these tests on Windows.
static TMP_DIR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/.*?\.tmp[a-zA-Z0-9]{6}").unwrap());

pub fn redacted_text(out: &str) -> String {
    TMP_DIR_RE.replace_all(out, "[TMP_DIR]").to_string()
}

pub fn format_diff(chunks: Vec<dissimilar::Chunk>) -> String {
    let mut buf = String::new();
    for chunk in chunks {
        let formatted = match chunk {
            dissimilar::Chunk::Equal(text) => text.into(),
            dissimilar::Chunk::Delete(text) => format!("\x1b[41m{}\x1b[0m", text),
            dissimilar::Chunk::Insert(text) => format!("\x1b[42m{}\x1b[0m", text),
        };
        buf.push_str(&formatted);
    }
    buf
}

pub use dissimilar::diff as __diff;

// Copied from https://github.com/rust-lang/rust-analyzer/blob/1dbdac8f518e5d3400a0bbc0478a606ab70d8a44/crates/test_utils/src/lib.rs#L38
#[macro_export]
macro_rules! assert_eq_text {
    ($left:expr, $right:expr) => {
        assert_eq_text!($left, $right,)
    };
    ($left:expr, $right:expr, $($tt:tt)*) => {{
        let left = $left;
        let right = $right;
        if left != right {
            if left.trim() == right.trim() {
                std::eprintln!("Left:\n{:?}\n\nRight:\n{:?}\n\nWhitespace difference\n", left, right);
            } else {
                let diff = $crate::__diff(left, right);
                std::eprintln!("Left:\n{}\n\nRight:\n{}\n\nDiff:\n{}\n", left, right, $crate::format_diff(diff));
            }
            std::eprintln!($($tt)*);
            panic!("text differs");
        }
    }};
}

// Copied from https://github.com/assert-rs/assert_cmd/blob/v2.1.1/src/macros.rs#L56
#[macro_export]
macro_rules! cargo_bin {
    ($bin_target_name:expr) => {
        ::std::path::Path::new(env!(concat!("CARGO_BIN_EXE_", $bin_target_name)))
    };
}
