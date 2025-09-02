use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let target = env::var("TARGET").unwrap_or_else(|_| "wasm32-unknown-unknown".to_string());

    println!("cargo:rerun-if-changed=src/");

    // Handle both possible targets that Zed might use
    if target == "wasm32-unknown-unknown" || target == "wasm32-wasip2" {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

        // Try both possible filenames that might be generated
        let possible_sources = vec![
            manifest_dir
                .join("target")
                .join(&target)
                .join(&profile)
                .join("odin_debugger.wasm"),
            manifest_dir
                .join("target")
                .join(&target)
                .join(&profile)
                .join("extension.wasm"),
        ];

        let out_wasm = manifest_dir.join("extension.wasm");

        for source_wasm in possible_sources {
            if source_wasm.exists() {
                if let Err(e) = fs::copy(&source_wasm, &out_wasm) {
                    println!("cargo:warning=Failed to copy wasm: {}", e);
                } else {
                    println!("cargo:warning=Copied {:?} -> {:?}", source_wasm, out_wasm);
                    return;
                }
            }
        }

        println!(
            "cargo:warning=No wasm found in target/{}/{}/",
            target, profile
        );

        // List what files actually exist
        let target_dir = manifest_dir.join("target").join(&target).join(&profile);
        if target_dir.exists() {
            if let Ok(entries) = fs::read_dir(&target_dir) {
                println!("cargo:warning=Files in target directory:");
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.ends_with(".wasm") {
                            println!("cargo:warning=  {}", name);
                        }
                    }
                }
            }
        }
    }
}
