use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=web/dist");
    println!("cargo:rerun-if-changed=web/index.html");
    println!("cargo:rerun-if-changed=web/package.json");

    if env::var("CARGO_FEATURE_WEB_UI").is_err() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dist_dir = manifest_dir.join("web").join("dist");
    let index_file = dist_dir.join("index.html");

    if dist_dir.is_dir() && index_file.is_file() {
        return;
    }

    panic!(
        "Web UI assets not found.\n\
Expected: {}\n\
\n\
Build the web UI assets:\n\
  cd crates/pocket-tts-cli/web\n\
  npm install\n\
  npm run build\n\
\n\
Or with bun:\n\
  bun install\n\
  bun run build\n\
\n\
Or build without the web UI:\n\
  cargo build --release --no-default-features\n",
        dist_dir.display()
    );
}
