//! Copie `vendor/interception/interception.dll` a cote du binaire compile
//! (target/debug ou target/release) a chaque build, pour que `cargo build`
//! / `cargo run` fonctionnent sans etape manuelle : le driver noyau doit
//! toujours etre installe separement (voir README), mais le DLL utilisateur
//! est desormais embarque avec le code source.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dll_src = manifest_dir
        .join("..")
        .join("vendor")
        .join("interception")
        .join("interception.dll");

    println!("cargo:rerun-if-changed={}", dll_src.display());

    // OUT_DIR ressemble a target/<profile>/build/<pkg>-<hash>/out ; les 3
    // parents remontent a target/<profile>, la ou atterrit l'executable.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let Some(target_profile_dir) = out_dir.ancestors().nth(3) else {
        println!("cargo:warning=impossible de localiser le dossier de sortie pour copier interception.dll");
        return;
    };

    let dll_dst = target_profile_dir.join("interception.dll");
    if let Err(e) = fs::copy(&dll_src, &dll_dst) {
        println!(
            "cargo:warning=echec de copie de {} vers {}: {e}",
            dll_src.display(),
            dll_dst.display()
        );
    }
}
