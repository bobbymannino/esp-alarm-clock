use std::{fs, path::Path};

fn main() {
    embuild::espidf::sysenv::output();

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=.env");

    load_dotenv(Path::new(".env"));
}

/// Reads a `.env` file and re-exports each entry as a compile time environment
/// variable, so that `env!` / `option_env!` can see it.
///
/// Lines may be blank, a `#` comment, or `KEY=VALUE` with an optional `export`
/// prefix and optional single or double quotes around the value. A missing file
/// is not an error.
fn load_dotenv(path: &Path) {
    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };

    for (idx, line) in contents.lines().enumerate() {
        let lineno = idx.saturating_add(1);
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line = line.strip_prefix("export ").unwrap_or(line);

        let Some((key, value)) = line.split_once('=') else {
            println!("cargo::warning=.env:{lineno}: ignoring line without '='");
            continue;
        };

        let key = key.trim();
        if key.is_empty() {
            println!("cargo::warning=.env:{lineno}: ignoring line with empty key");
            continue;
        }

        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);

        println!("cargo::rustc-env={key}={value}");
    }
}
