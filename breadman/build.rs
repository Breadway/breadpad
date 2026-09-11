use std::path::Path;
use std::process::Command;

fn main() {
    build_frontend();
    tauri_build::build();
}

/// Build the Svelte frontend into `ui/dist` so a plain `cargo build`
/// (including CI's `cargo build --release --locked`, which never runs Tauri's
/// `beforeBuildCommand`) produces a complete binary. Set
/// `BREADMAN_SKIP_UI_BUILD=1` to manage `ui/dist` yourself during fast local
/// iteration (e.g. `pnpm -C ui dev`).
fn build_frontend() {
    println!("cargo:rerun-if-env-changed=BREADMAN_SKIP_UI_BUILD");
    println!("cargo:rerun-if-changed=ui/src");
    println!("cargo:rerun-if-changed=ui/package.json");
    println!("cargo:rerun-if-changed=ui/pnpm-lock.yaml");
    println!("cargo:rerun-if-changed=ui/index.html");
    println!("cargo:rerun-if-changed=ui/vite.config.ts");

    if std::env::var_os("BREADMAN_SKIP_UI_BUILD").is_some() {
        return;
    }

    let ui = Path::new("ui");
    let Some(pnpm) = find_pnpm() else {
        println!(
            "cargo:warning=pnpm not found; skipping the breadman frontend build. \
             Install pnpm, or run `pnpm -C breadman/ui install && pnpm -C breadman/ui build` \
             and re-run with BREADMAN_SKIP_UI_BUILD=1."
        );
        return;
    };

    run(&pnpm, &["install", "--frozen-lockfile"], ui);
    run(&pnpm, &["run", "build"], ui);
}

fn find_pnpm() -> Option<String> {
    for cand in ["pnpm", "pnpm.cmd"] {
        if Command::new(cand).arg("--version").output().is_ok() {
            return Some(cand.to_string());
        }
    }
    None
}

fn run(pnpm: &str, args: &[&str], dir: &Path) {
    let status = Command::new(pnpm)
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|e| panic!("failed to run `{pnpm} {}`: {e}", args.join(" ")));
    if !status.success() {
        panic!("`{pnpm} {}` failed with {status}", args.join(" "));
    }
}
