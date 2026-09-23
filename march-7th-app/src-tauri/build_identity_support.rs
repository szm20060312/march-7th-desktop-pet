// Keep this list aligned with G7-BUILD-IDENTITY.md. The same scope drives Git
// status and Cargo watches; never watch the app root (it contains build caches).
use std::{env, path::Path, process::Command};

// Project convention: keep both automatic desktop overrides present, even if
// empty. Missing optional paths cannot be watched without perpetual rebuilds.
const PLATFORM_CONFIGS: &[(&str, &str)] = &[
    ("src-tauri/tauri.windows.conf.json", "windows-config"),
    ("src-tauri/tauri.macos.conf.json", "macos-config"),
];

// Labels are fixed public vocabulary, never derived from filesystem names.
const INPUTS: &[(&str, &str)] = &[
    ("src", "frontend-source"),
    ("public", "public-assets"),
    ("index.html", "main-entry"),
    ("settings.html", "settings-entry"),
    ("reminder.html", "reminder-entry"),
    ("package.json", "frontend-manifest"),
    ("pnpm-lock.yaml", "frontend-lock"),
    ("tsconfig.json", "typescript-config"),
    ("vite.config.ts", "vite-config"),
    (".gitignore", "app-ignore"),
    ("src-tauri/.gitignore", "native-ignore"),
    ("src-tauri/src", "native-source"),
    ("src-tauri/icons", "native-icons"),
    ("src-tauri/capabilities", "native-capabilities"),
    ("src-tauri/Cargo.toml", "native-manifest"),
    ("src-tauri/Cargo.lock", "native-lock"),
    ("src-tauri/tauri.conf.json", "tauri-config"),
    ("src-tauri/build.rs", "native-build-script"),
    (
        "src-tauri/build_identity_support.rs",
        "identity-build-support",
    ),
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

fn diagnose_scopes(root: &Path, scopes: &[(String, &str)]) {
    for (input, label) in scopes {
        // Quoted porcelain keeps filenames (including newline names) inside one
        // record. Inspect only its status prefix; never print any Git output.
        let status = git(
            root,
            &[
                "-c",
                "core.quotePath=true",
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--ignored=matching",
                "--",
                input,
            ],
        );
        let Some(status) = status else {
            println!("cargo:warning=build-input-diagnostic scope={label} unavailable");
            continue;
        };
        let mut changed = [false; 3];
        for line in status.lines() {
            match line.get(..2) {
                Some("??") => changed[1] = true,
                Some("!!") => changed[2] = true,
                Some(_) => changed[0] = true,
                None => {}
            }
        }
        let categories = ["tracked", "untracked", "ignored"]
            .into_iter()
            .zip(changed)
            .filter_map(|(category, present)| present.then_some(category))
            .collect::<Vec<_>>();
        if !categories.is_empty() {
            println!(
                "cargo:warning=build-input-diagnostic scope={label} categories={}",
                categories.join(",")
            );
        }
    }
}

pub fn embed() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory");
    let root = Path::new(&manifest)
        .parent()
        .expect("application directory");
    for (config, _) in PLATFORM_CONFIGS {
        assert!(
            root.join(config).is_file(),
            "Missing required project build file: {config}; restore the tracked platform config (an empty object is valid)"
        );
    }
    for (input, _) in INPUTS.iter().chain(PLATFORM_CONFIGS) {
        watch(&root.join(input));
    }
    println!("cargo:rerun-if-env-changed=PATH");
    println!("cargo:rerun-if-env-changed=MARCH_BUILD_INPUT_DIAGNOSTICS");
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
        .map(|(input, label)| ((*input).to_owned(), *label))
        .collect::<Vec<_>>();
    if let Some(repo) = git(root, &["rev-parse", "--show-toplevel"]) {
        // Repository attributes affect the bytes Git compares (e.g. CRLF).
        for (name, label) in [
            (".gitattributes", "repository-attributes"),
            (".gitignore", "repository-ignore"),
        ] {
            let file = Path::new(repo.trim()).join(name);
            watch(&file);
            scope.push((file.to_string_lossy().into_owned(), label));
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
        args.extend(scope.iter().map(|(input, _)| input.as_str()));
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
    if state == "modified" && env::var("MARCH_BUILD_INPUT_DIAGNOSTICS").as_deref() == Ok("1") {
        diagnose_scopes(root, &scope);
    }
    println!(
        "cargo:rustc-env=MARCH_BUILD_TARGET={}",
        env::var("TARGET").expect("Cargo target")
    );
    println!("cargo:rustc-env=MARCH_SOURCE_COMMIT={commit}");
    println!("cargo:rustc-env=MARCH_SOURCE_STATE={state}");
}
