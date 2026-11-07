use std::{env, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=packaging/windows/filespring.manifest");
    println!("cargo::rerun-if-env-changed=FILESPRING_WINDOWS_RESOURCE");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("packaging/windows/filespring.manifest");
    let mut bins = vec!["filespring"];
    if env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        bins.push("filespring-desktop");
    }
    for bin in bins {
        if let Some(resource) = env::var_os("FILESPRING_WINDOWS_RESOURCE") {
            println!(
                "cargo::rustc-link-arg-bin={bin}={}",
                PathBuf::from(resource).display()
            );
            continue;
        }
        println!("cargo::rustc-link-arg-bin={bin}=/MANIFEST:EMBED");
        println!(
            "cargo::rustc-link-arg-bin={bin}=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
