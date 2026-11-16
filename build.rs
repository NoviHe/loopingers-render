use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/logo/favicon.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let Ok(out_dir) = env::var("OUT_DIR") else {
        return;
    };

    let ico_path = Path::new(&manifest_dir).join("assets/logo/favicon.ico");
    if !ico_path.is_file() {
        return;
    }

    let Some(rc_bin) = find_resource_compiler() else {
        return;
    };

    let out_path = Path::new(&out_dir);
    let rc_file = out_path.join("app.rc");
    let res_file = out_path.join("app.res");

    let ico_forward = ico_path.to_string_lossy().replace('\\', "/");
    let rc_content = format!("1 ICON \"{ico_forward}\"\n");
    if fs::write(&rc_file, rc_content).is_err() {
        return;
    }

    let status = Command::new(&rc_bin)
        .arg("/fo")
        .arg(&res_file)
        .arg(&rc_file)
        .status();

    if matches!(status, Ok(s) if s.success()) && res_file.is_file() {
        println!("cargo:rustc-link-arg-bins={}", res_file.display());
    }
}

fn find_resource_compiler() -> Option<PathBuf> {
    if let Some(path) = find_in_path(&["rc.exe", "llvm-rc.exe"]) {
        return Some(path);
    }

    if let Some(sdk_rc) = find_windows_kits_rc() {
        return Some(sdk_rc);
    }

    let llvm_candidates = [
        env::var_os("ProgramFiles").map(|p| PathBuf::from(p).join(r"LLVM\bin\llvm-rc.exe")),
        Some(PathBuf::from(r"C:\Program Files\LLVM\bin\llvm-rc.exe")),
    ];
    llvm_candidates
        .into_iter()
        .flatten()
        .find(|candidate| candidate.is_file())
}

fn find_in_path(names: &[&str]) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn find_windows_kits_rc() -> Option<PathBuf> {
    let pf86 = env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"));
    let kits_bin = pf86.join(r"Windows Kits\10\bin");
    let entries = fs::read_dir(kits_bin).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path().join(r"x64\rc.exe")))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort();
    candidates.pop()
}
