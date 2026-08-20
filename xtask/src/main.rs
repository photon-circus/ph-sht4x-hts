use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const EXPECTED_VERSION: &str = "0.1.0-incubating.1";
const DRIVER_MANIFEST: &str = "crates/sht4x/Cargo.toml";
const LIFECYCLE_MANIFESTS: &[&str] = &[
    "crates/sht4x/Cargo.toml",
    "crates/sht4x-model/Cargo.toml",
    "crates/sht4x-conformance/Cargo.toml",
];
const SUPPORTED_TARGETS: &[&str] = &["thumbv7em-none-eabihf", "thumbv6m-none-eabi"];

fn main() {
    let mut args = env::args().skip(1);

    let result = match args.next().as_deref() {
        None | Some("--help" | "-h") => {
            print_usage();
            Ok(())
        }
        Some("ci") if args.next().is_none() => run_ci(),
        Some("ci") => Err("the `ci` command does not accept arguments".to_owned()),
        Some(command) => Err(format!("unknown xtask command `{command}`")),
    };

    if let Err(error) = result {
        eprintln!("failed: {error}");
        std::process::exit(1);
    }
}

fn print_usage() {
    println!("usage:");
    println!("  cargo xtask ci");
}

fn run_ci() -> Result<(), String> {
    let workspace = workspace_dir()?;

    println!("check: formatting");
    run_cargo(&workspace, ["fmt", "--all", "--", "--check"])?;

    println!("check: lifecycle version and publication lock");
    for relative_path in LIFECYCLE_MANIFESTS {
        validate_manifest(&workspace, relative_path)?;
    }

    println!("check: clippy");
    run_cargo(
        &workspace,
        [
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )?;

    println!("check: tests");
    run_cargo(
        &workspace,
        ["test", "--locked", "--workspace", "--all-features"],
    )?;

    println!("check: supported target compilation");
    let installed_targets = installed_targets(&workspace);
    for target in SUPPORTED_TARGETS {
        match target_decision(&installed_targets, target) {
            TargetDecision::Build => run_cargo(
                &workspace,
                [
                    "build",
                    "--locked",
                    "-p",
                    "ph-sht4x-hts",
                    "--target",
                    target,
                ],
            )?,
            TargetDecision::RustupUnavailable => {
                println!("skipped: target {target}, rustup is unavailable");
            }
            TargetDecision::QueryFailed => {
                println!(
                    "indeterminate: target {target}, the installed-target list could not be read"
                );
            }
            TargetDecision::NotInstalled => {
                println!("skipped: target {target} is not installed");
            }
        }
    }

    println!("check: documentation");
    let mut documentation = cargo_command(&workspace);
    documentation.env("RUSTDOCFLAGS", "-D warnings").args([
        "doc",
        "--locked",
        "--workspace",
        "--all-features",
        "--no-deps",
    ]);
    run(&mut documentation)?;

    let allow_dirty = package_allow_dirty(&workspace);

    println!("check: package construction");
    run_package_command(&workspace, allow_dirty, false)?;

    println!("check: package contents");
    run_package_command(&workspace, allow_dirty, true)?;

    if executable_available("cargo-deny") {
        println!("check: dependencies and licenses");
        let mut deny = Command::new("cargo");
        deny.current_dir(&workspace).args(["deny", "check"]);
        run(&mut deny)?;
    } else {
        println!("skipped: cargo-deny is not installed");
    }

    println!("passed: routine local software gate");
    Ok(())
}

fn workspace_dir() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask must be located directly under the workspace root".to_owned())
}

fn validate_manifest(workspace: &Path, relative_path: &str) -> Result<(), String> {
    let path = workspace.join(relative_path);
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("could not read {relative_path}: {error}"))?;
    validate_manifest_contents(relative_path, &contents)
}

fn validate_manifest_contents(relative_path: &str, contents: &str) -> Result<(), String> {
    let package_name = package_field(contents, "name");
    let actual_version = package_field(contents, "version");
    let (Some(package_name), Some(actual_version)) = (package_name, actual_version) else {
        return Err(format!(
            "could not read a package name and version from {relative_path}"
        ));
    };

    if actual_version != EXPECTED_VERSION {
        return Err(format!(
            "expected {package_name} version {EXPECTED_VERSION}, found {actual_version}"
        ));
    }

    let publish_setting = package_field(contents, "publish");
    if publish_setting.as_deref() != Some("false") {
        return Err(format!(
            "{relative_path} [package] must retain publish = false, found {}",
            publish_setting.as_deref().unwrap_or("no publish key")
        ));
    }

    Ok(())
}

fn package_field(contents: &str, field: &str) -> Option<String> {
    let mut in_package = false;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != field {
            continue;
        }

        let value = value_without_comment(value).trim();
        return Some(
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap_or(value)
                .to_owned(),
        );
    }

    None
}

fn value_without_comment(value: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;

    for (index, character) in value.char_indices() {
        match character {
            '\\' if quoted => escaped = !escaped,
            '"' if !escaped => quoted = !quoted,
            '#' if !quoted => return &value[..index],
            _ => escaped = false,
        }
    }

    value
}

enum InstalledTargets {
    RustupUnavailable,
    QueryFailed,
    Available(String),
}

#[derive(Debug, PartialEq, Eq)]
enum TargetDecision {
    Build,
    RustupUnavailable,
    QueryFailed,
    NotInstalled,
}

fn installed_targets(workspace: &Path) -> InstalledTargets {
    // A failed query is not evidence that a target is absent, so preserve it as
    // indeterminate rather than reporting an ordinary skip.
    match Command::new("rustup")
        .current_dir(workspace)
        .args(["target", "list", "--installed"])
        .output()
    {
        Ok(output) if output.status.success() => {
            InstalledTargets::Available(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        Ok(_) => InstalledTargets::QueryFailed,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            InstalledTargets::RustupUnavailable
        }
        Err(_) => InstalledTargets::QueryFailed,
    }
}

fn target_decision(installed: &InstalledTargets, target: &str) -> TargetDecision {
    match installed {
        InstalledTargets::RustupUnavailable => TargetDecision::RustupUnavailable,
        InstalledTargets::QueryFailed => TargetDecision::QueryFailed,
        InstalledTargets::Available(list) if list.lines().any(|line| line.trim() == target) => {
            TargetDecision::Build
        }
        InstalledTargets::Available(_) => TargetDecision::NotInstalled,
    }
}

fn package_allow_dirty(workspace: &Path) -> bool {
    // Cargo reads repository state without the git CLI. If this check cannot
    // establish cleanliness, package the working tree and say so rather than
    // allowing Cargo to abort or implying that the committed tree was checked.
    match Command::new("git")
        .current_dir(workspace)
        .args(["status", "--porcelain"])
        .output()
    {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            println!("notice: git is unavailable; package checks cover the working tree");
            true
        }
        Err(_) => {
            println!(
                "notice: repository status is unreadable; package checks cover the working tree"
            );
            true
        }
        Ok(output) if !output.status.success() => {
            println!(
                "notice: repository status is unreadable; package checks cover the working tree"
            );
            true
        }
        Ok(output) if !output.stdout.is_empty() => {
            println!(
                "notice: working tree is dirty; package checks cover it, not the committed tree"
            );
            true
        }
        Ok(_) => false,
    }
}

fn run_package_command(workspace: &Path, allow_dirty: bool, list: bool) -> Result<(), String> {
    // Construction performs Cargo's verification build from the unpacked
    // archive; listing alone would not detect a required excluded source file.
    let mut command = cargo_command(workspace);
    command.args(["package", "--locked", "--manifest-path", DRIVER_MANIFEST]);
    if allow_dirty {
        command.arg("--allow-dirty");
    }
    if list {
        command.arg("--list");
    }
    run(&mut command)
}

fn run_cargo<I, S>(workspace: &Path, args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut command = cargo_command(workspace);
    command.args(args);
    run(&mut command)
}

fn cargo_command(workspace: &Path) -> Command {
    let mut command = Command::new(env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo")));
    command.current_dir(workspace);
    command
}

fn run(command: &mut Command) -> Result<(), String> {
    let description = format!("{command:?}");
    let status = command
        .status()
        .map_err(|error| format!("could not run {description}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command {description} exited with {status}"))
    }
}

fn executable_available(name: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };

    executable_names(name).iter().any(|candidate| {
        env::split_paths(&path).any(|directory| is_executable(&directory.join(candidate)))
    })
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(windows)]
fn executable_names(name: &str) -> Vec<OsString> {
    let extensions =
        env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
    let mut names = vec![OsString::from(name)];
    names.extend(
        extensions
            .to_string_lossy()
            .split(';')
            .filter(|extension| !extension.is_empty())
            .map(|extension| OsString::from(format!("{name}{extension}"))),
    );
    names
}

#[cfg(not(windows))]
fn executable_names(name: &str) -> Vec<OsString> {
    vec![OsString::from(name)]
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_MANIFEST: &str = r#"
[workspace.package]
version = "9.9.9"
publish = true

[package]
name = "example"
version = "0.1.0-incubating.1" # lifecycle version
publish = false

[package.metadata.example]
publish = true
"#;

    #[test]
    fn package_fields_come_only_from_the_package_table() {
        assert_eq!(
            package_field(VALID_MANIFEST, "name").as_deref(),
            Some("example")
        );
        assert_eq!(
            package_field(VALID_MANIFEST, "version").as_deref(),
            Some(EXPECTED_VERSION)
        );
        assert_eq!(
            package_field(VALID_MANIFEST, "publish").as_deref(),
            Some("false")
        );
    }

    #[test]
    fn missing_package_fields_are_rejected() {
        let error = validate_manifest_contents(
            "Cargo.toml",
            "[package]\nname = \"example\"\npublish = false\n",
        )
        .unwrap_err();
        assert!(error.contains("could not read a package name and version"));
    }

    #[test]
    fn wrong_version_is_rejected() {
        let error = validate_manifest_contents(
            "Cargo.toml",
            "[package]\nname = \"example\"\nversion = \"1.0.0\"\npublish = false\n",
        )
        .unwrap_err();
        assert!(error.contains("expected example version"));
    }

    #[test]
    fn missing_or_enabled_publication_is_rejected() {
        for publish_line in ["", "publish = true\n"] {
            let manifest = format!(
                "[package]\nname = \"example\"\nversion = \"{EXPECTED_VERSION}\"\n{publish_line}"
            );
            let error = validate_manifest_contents("Cargo.toml", &manifest).unwrap_err();
            assert!(error.contains("must retain publish = false"));
        }
    }

    #[test]
    fn installed_target_matching_is_exact() {
        let installed =
            InstalledTargets::Available("thumbv6m-none-eabi\nthumbv7em-none-eabihf\n".to_owned());
        assert_eq!(
            target_decision(&installed, "thumbv7em-none-eabihf"),
            TargetDecision::Build
        );
        assert_eq!(
            target_decision(&installed, "thumbv7em-none-eabi"),
            TargetDecision::NotInstalled
        );
    }

    #[test]
    fn target_prerequisite_failures_remain_distinct() {
        assert_eq!(
            target_decision(&InstalledTargets::RustupUnavailable, "target"),
            TargetDecision::RustupUnavailable
        );
        assert_eq!(
            target_decision(&InstalledTargets::QueryFailed, "target"),
            TargetDecision::QueryFailed
        );
    }
}
