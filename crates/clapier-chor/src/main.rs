//! chor-encode: Violet choreography text in, binary .chor out.
//!
//! Usage: chor-encode "10,0,led,2,0,238,0,..." output.chor
//! The text may also arrive on stdin with `-` in its place.

use std::io::Read;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cdl_arg, out_path] = args.as_slice() else {
        eprintln!("usage: chor-encode <cdl-text|-> <output.chor>");
        return std::process::ExitCode::FAILURE;
    };

    let cdl = if cdl_arg == "-" {
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
            eprintln!("stdin: {e}");
            return std::process::ExitCode::FAILURE;
        }
        buf
    } else {
        cdl_arg.clone()
    };

    match clapier_chor::encode_cdl(&cdl) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(out_path, &bytes) {
                eprintln!("write {out_path}: {e}");
                return std::process::ExitCode::FAILURE;
            }
            println!("{} bytes -> {out_path}", bytes.len());
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("choreography rejected: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
