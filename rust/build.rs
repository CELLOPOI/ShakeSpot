use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=rust/resources.rc");
    println!("cargo:rerun-if-changed=rust/app.manifest");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let resource = out.join("shakespot.res");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let sdk = PathBuf::from(env::var_os("ProgramFiles(x86)").expect("ProgramFiles(x86)"))
        .join("Windows Kits/10/bin");
    let mut versions: Vec<_> = std::fs::read_dir(&sdk)
        .expect("Windows SDK missing")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.join("x64/rc.exe").is_file())
        .collect();
    versions.sort();
    let rc = versions
        .last()
        .expect("Windows SDK resource compiler missing")
        .join("x64/rc.exe");
    let status = Command::new(rc)
        .current_dir(root.join("rust"))
        .arg("/nologo")
        .arg(format!("/fo{}", resource.display()))
        .arg("resources.rc")
        .status()
        .expect("Resource compiler failed to start");
    assert!(status.success(), "Resource compilation failed");
    println!("cargo:rustc-link-arg-bin=ShakeSpot={}", resource.display());
}
