use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../../frontend/src");
    println!("cargo:rerun-if-changed=../../frontend/index.html");
    println!("cargo:rerun-if-changed=../../frontend/vite.config.ts");
    println!("cargo:rerun-if-changed=../../frontend/deno.json");

    let dist_index = Path::new("../../frontend/dist/index.html");
    if dist_index.exists() {
        return;
    }

    // Try to build frontend if dist doesn't exist
    let status = Command::new("deno")
        .args(["task", "build"])
        .current_dir("../../frontend")
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("cargo:warning=Frontend build failed with status: {s}");
            eprintln!(
                "cargo:warning=Run `cd frontend && deno install && deno task build` manually"
            );
        }
        Err(_) => {
            eprintln!("cargo:warning=deno not found. Run `cd frontend && deno install && deno task build` manually");
        }
    }
}
