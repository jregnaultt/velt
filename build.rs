use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/branding/velt.rc");
    println!("cargo:rerun-if-changed=assets/branding/velt.ico");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "gnu" {
        return;
    }

    let out_dir = match env::var_os("OUT_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => return,
    };
    let res_path = out_dir.join("velt_icon.res");

    let Ok(status) = Command::new("windres")
        .args([
            "-I",
            "assets/branding",
            "-O",
            "coff",
            "assets/branding/velt.rc",
        ])
        .arg(&res_path)
        .status()
    else {
        return;
    };

    if status.success() {
        println!("cargo:rustc-link-arg={}", res_path.display());
    }
}
