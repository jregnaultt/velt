use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/branding/velt.rc");
    println!("cargo:rerun-if-changed=assets/branding/velt.ico");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let Some(out_dir) = env::var_os("OUT_DIR").map(PathBuf::from) else {
        return;
    };
    let res_path = out_dir.join("velt_icon.res");

    let status = match target_env.as_str() {
        "gnu" => Command::new("windres")
            .args([
                "-I",
                "assets/branding",
                "-O",
                "coff",
                "assets/branding/velt.rc",
            ])
            .arg(&res_path)
            .status(),
        "msvc" => Command::new("rc.exe")
            .args(["/nologo", "/I", "assets/branding", "/fo"])
            .arg(&res_path)
            .arg("assets/branding/velt.rc")
            .status(),
        _ => return,
    };

    if let Ok(exit) = status {
        if exit.success() {
            println!("cargo:rustc-link-arg={}", res_path.display());
        }
    }
}
