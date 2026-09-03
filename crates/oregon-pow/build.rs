use std::{
    env, fs,
    path::{Path, PathBuf},
};

const UPSTREAM_SALT_LINE: &str = "#define RANDOMX_ARGON_SALT         \"RandomX\\x03\"";
const OREGON_SALT_LINE: &str = "#define RANDOMX_ARGON_SALT         \"OREGON-RANDOMX-V1\"";

fn copy_dir(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create RandomX build copy directory");
    for entry in fs::read_dir(source).expect("read RandomX source directory") {
        let entry = entry.expect("read RandomX source entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().expect("read RandomX source entry type");
        if file_type.is_dir() {
            copy_dir(&source_path, &destination_path);
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path).expect("copy RandomX source file");
        }
    }
}

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("oregon-pow must live under crates/");
    let upstream = repo_root.join("vendor/RandomX");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let patched = out_dir.join("randomx-oregon");

    println!("cargo:rerun-if-changed={}", upstream.display());

    if patched.exists() {
        fs::remove_dir_all(&patched).expect("remove stale RandomX build copy");
    }
    copy_dir(&upstream, &patched);

    let configuration = patched.join("src/configuration.h");
    let source = fs::read_to_string(&configuration).expect("read copied RandomX configuration.h");
    let matches = source.matches(UPSTREAM_SALT_LINE).count();
    assert_eq!(
        matches, 1,
        "RandomX provenance/config mismatch: expected exactly one upstream Argon salt line"
    );
    let patched_source = source.replacen(UPSTREAM_SALT_LINE, OREGON_SALT_LINE, 1);
    fs::write(&configuration, patched_source).expect("write Oregon RandomX configuration.h copy");

    let destination = cmake::Config::new(&patched).profile("Release").build();
    println!(
        "cargo:rustc-link-search=native={}",
        destination.join("lib").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        destination.join("lib64").display()
    );
    println!("cargo:rustc-link-lib=static=randomx");

    match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => {}
        _ => match env::var("CARGO_CFG_TARGET_OS").as_deref() {
            Ok("macos") | Ok("ios") => println!("cargo:rustc-link-lib=c++"),
            _ => println!("cargo:rustc-link-lib=stdc++"),
        },
    }
}
