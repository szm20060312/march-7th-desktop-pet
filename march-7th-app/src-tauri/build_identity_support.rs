// Keep this list aligned with G7-BUILD-IDENTITY.md. The same scope drives Git
// status and Cargo watches; never watch the app root (it contains build caches).
use std::{env, path::Path, process::Command};

// Project convention: keep both automatic desktop overrides present, even if
// empty. Missing optional paths cannot be watched without perpetual rebuilds.
const PLATFORM_CONFIGS: &[&str] = &[
    "src-tauri/tauri.windows.conf.json",
    "src-tauri/tauri.macos.conf.json",
];

const INPUTS: &[&str] = &[
    "src",
    "public",
    "index.html",
    "settings.html",
    "reminder.html",
    "package.json",
    "pnpm-lock.yaml",
    "tsconfig.json",
    "vite.config.ts",
    ".gitignore",
    "src-tauri/.gitignore",
    "src-tauri/src",
    "src-tauri/icons",
    "src-tauri/capabilities",
    "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock",
    "src-tauri/tauri.conf.json",
    "src-tauri/build.rs",
    "src-tauri/build_identity_support.rs",
];

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        // Ambient Git redirection must not assign another checkout's identity.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(args)
        .output()
        .ok()?;
    output.status.success().then_some(())?;
    String::from_utf8(output.stdout).ok()
}

fn watch(path: &Path) {
    // Watching a nonexistent packed ref would force every Cargo run to rebuild.
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn git_watch(root: &Path, name: &str) {
    if let Some(path) = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", name],
    ) {
        watch(Path::new(path.trim()));
    }
}

pub fn embed() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory");
    let root = Path::new(&manifest)
        .parent()
        .expect("application directory");
    for config in PLATFORM_CONFIGS {
        assert!(
            root.join(config).is_file(),
            "Missing required project build file: {config}; restore the tracked platform config (an empty object is valid)"
        );
    }
    for input in INPUTS.iter().chain(PLATFORM_CONFIGS) {
        watch(&root.join(input));
    }
    println!("cargo:rerun-if-env-changed=PATH");
    for control in ["HEAD", "index", "packed-refs", "config", "info/exclude"] {
        git_watch(root, control);
    }
    if let Some(reference) = git(root, &["symbolic-ref", "-q", "HEAD"]) {
        git_watch(root, reference.trim());
        // Packed branches can acquire a new loose ref without touching HEAD or
        // packed-refs. Watch the existing ref directory, never the Git root.
        if let Some(path) = git(
            root,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                reference.trim(),
            ],
        ) {
            if let Some(parent) = Path::new(path.trim())
                .ancestors()
                .skip(1)
                .find(|parent| parent.is_dir())
            {
                watch(parent);
            }
        }
    }
    // A linked worktree's .git is a file, not a directory.
    for ancestor in root.ancestors() {
        let marker = ancestor.join(".git");
        if marker.is_file() {
            watch(&marker);
            break;
        }
        if marker.is_dir() {
            break;
        }
    }
    let mut scope = INPUTS
        .iter()
        .chain(PLATFORM_CONFIGS)
        .map(|input| (*input).to_owned())
        .collect::<Vec<_>>();
    if let Some(repo) = git(root, &["rev-parse", "--show-toplevel"]) {
        // Repository attributes affect the bytes Git compares (e.g. CRLF).
        for name in [".gitattributes", ".gitignore"] {
            let file = Path::new(repo.trim()).join(name);
            watch(&file);
            scope.push(file.to_string_lossy().into_owned());
        }
    }
    let source = (|| {
        // An untracked export inside an unrelated repository must stay unknown.
        git(
            root,
            &["ls-files", "--error-unmatch", "--", "src-tauri/Cargo.toml"],
        )?;
        let commit = git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
        let commit = commit.trim();
        if commit.len() != 40
            || !commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        let mut args = vec![
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignored=matching",
            "--",
        ];
        args.extend(scope.iter().map(String::as_str));
        let changed = git(root, &args)?;
        Some((
            commit.to_owned(),
            if changed.is_empty() {
                "clean"
            } else {
                "modified"
            },
        ))
    })();
    let (commit, state) = source.unwrap_or_else(|| (String::new(), "unknown"));
    println!(
        "cargo:rustc-env=MARCH_BUILD_TARGET={}",
        env::var("TARGET").expect("Cargo target")
    );
    println!("cargo:rustc-env=MARCH_SOURCE_COMMIT={commit}");
    println!("cargo:rustc-env=MARCH_SOURCE_STATE={state}");
}
