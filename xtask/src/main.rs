use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const CONFIG_FILE: &str = "ci.ron";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CiConfig {
    expected_version: String,
    lifecycle_packages: Vec<LifecyclePackage>,
    driver: DriverPackage,
    supported_targets: Vec<String>,
    coverage: CoverageConfig,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct LifecyclePackage {
    manifest: String,
    require_unpublished: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct DriverPackage {
    package: String,
    manifest: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CoverageConfig {
    unit_packages: Vec<String>,
    conformance: ConformanceCoverage,
    infrastructure_packages: Vec<String>,
    ignored_filename_regex: String,
    output_directory: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ConformanceCoverage {
    package: String,
    test: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct CoverageExport {
    data: Vec<CoverageData>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct CoverageData {
    files: Vec<CoverageFile>,
    totals: CoverageTotals,
}

#[derive(Debug, Deserialize, PartialEq)]
struct CoverageFile {
    filename: String,
    #[serde(default)]
    summary: Option<CoverageTotals>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct CoverageTotals {
    functions: CoverageMetric,
    lines: CoverageMetric,
    regions: CoverageMetric,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct CoverageMetric {
    count: u64,
    covered: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CheckRecord {
    name: String,
    status: CheckStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CheckStatus {
    Passed,
    Skipped(String),
    Indeterminate(String),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
struct CoverageSnapshot {
    label: String,
    report: PathBuf,
    totals: CoverageTotals,
    files: Vec<CoverageFileSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
struct CoverageFileSnapshot {
    path: String,
    totals: CoverageTotals,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct CiSummary {
    checks: Vec<CheckRecord>,
    coverage: Vec<CoverageSnapshot>,
}

impl CiSummary {
    fn record(
        &mut self,
        name: impl Into<String>,
        result: Result<(), String>,
    ) -> Result<(), String> {
        match result {
            Ok(()) => {
                self.pass(name);
                Ok(())
            }
            Err(error) => {
                self.fail(name, &error);
                Err(error)
            }
        }
    }

    fn pass(&mut self, name: impl Into<String>) {
        self.checks.push(CheckRecord {
            name: name.into(),
            status: CheckStatus::Passed,
        });
    }

    fn skip(&mut self, name: impl Into<String>, reason: impl Into<String>) {
        self.checks.push(CheckRecord {
            name: name.into(),
            status: CheckStatus::Skipped(reason.into()),
        });
    }

    fn indeterminate(&mut self, name: impl Into<String>, reason: impl Into<String>) {
        self.checks.push(CheckRecord {
            name: name.into(),
            status: CheckStatus::Indeterminate(reason.into()),
        });
    }

    fn fail(&mut self, name: impl Into<String>, error: impl Into<String>) {
        self.checks.push(CheckRecord {
            name: name.into(),
            status: CheckStatus::Failed(error.into()),
        });
    }
}

fn main() {
    let mut args = env::args().skip(1);

    let result = match args.next().as_deref() {
        None | Some("--help" | "-h") => {
            print_usage();
            Ok(())
        }
        Some("ci") if args.next().is_none() => run_ci(),
        Some("ci") => Err("the `ci` command does not accept arguments".to_owned()),
        Some("fmt") if args.next().is_none() => run_fmt(),
        Some("fmt") => Err("the `fmt` command does not accept arguments".to_owned()),
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
    println!("  cargo xtask fmt");
}

macro_rules! ci_step {
    ($summary:ident, $name:expr, $body:expr) => {{
        let name = $name;
        println!("check: {name}");
        if let Err(error) = $summary.record(name, $body) {
            emit_ci_summary(&$summary);
            return Err(error);
        }
    }};
}

fn run_ci() -> Result<(), String> {
    let workspace = workspace_dir()?;
    let config = load_config()?;
    let mut summary = CiSummary::default();

    ci_step!(
        summary,
        "formatting",
        run_cargo(&workspace, rustfmt_args(true))
    );
    ci_step!(summary, "lifecycle version and publication lock", {
        config.lifecycle_packages.iter().try_for_each(|package| {
            validate_manifest(&workspace, package, config.expected_version.as_str())
        })
    });
    ci_step!(
        summary,
        "clippy",
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
        )
    );
    ci_step!(summary, "tests", run_cargo(&workspace, test_args(false)));
    ci_step!(
        summary,
        "release tests",
        run_cargo(&workspace, test_args(true))
    );

    match run_coverage(&workspace, &config.coverage) {
        Ok(coverage) => {
            summary.pass("unit test coverage");
            summary.pass("model-conformance coverage");
            summary.coverage = coverage;
            if let Err(error) =
                write_coverage_summary_file(&workspace, &config.coverage, &summary.coverage)
            {
                summary.fail("coverage summary file", &error);
                emit_ci_summary(&summary);
                return Err(error);
            }
        }
        Err(error) => {
            summary.fail("coverage", &error);
            emit_ci_summary(&summary);
            return Err(error);
        }
    }

    println!("check: supported target compilation");
    let installed_targets = installed_targets(&workspace);
    for target in &config.supported_targets {
        let name = format!("target {target} (release)");
        match target_decision(&installed_targets, target) {
            TargetDecision::Build => ci_step!(
                summary,
                &name,
                run_cargo(
                    &workspace,
                    [
                        "build",
                        "--locked",
                        "--release",
                        "-p",
                        config.driver.package.as_str(),
                        "--target",
                        target.as_str(),
                    ],
                )
            ),
            TargetDecision::RustupUnavailable => {
                println!("skipped: target {target}, rustup is unavailable");
                summary.skip(&name, "rustup is unavailable");
            }
            TargetDecision::QueryFailed => {
                println!(
                    "indeterminate: target {target}, the installed-target list could not be read"
                );
                summary.indeterminate(&name, "the installed-target list could not be read");
            }
            TargetDecision::NotInstalled => {
                println!("skipped: target {target} is not installed");
                summary.skip(&name, "not installed");
            }
        }
    }

    ci_step!(summary, "documentation", {
        let mut documentation = cargo_command(&workspace);
        documentation.env("RUSTDOCFLAGS", "-D warnings").args([
            "doc",
            "--locked",
            "--workspace",
            "--all-features",
            "--no-deps",
        ]);
        run(&mut documentation)
    });

    let allow_dirty = package_allow_dirty(&workspace);

    ci_step!(
        summary,
        "package construction",
        run_package_command(
            &workspace,
            config.driver.manifest.as_str(),
            allow_dirty,
            false,
        )
    );
    ci_step!(
        summary,
        "package contents",
        run_package_command(
            &workspace,
            config.driver.manifest.as_str(),
            allow_dirty,
            true,
        )
    );

    if executable_available("cargo-deny") {
        ci_step!(summary, "dependencies and licenses", {
            let mut deny = Command::new("cargo");
            deny.current_dir(&workspace).args(["deny", "check"]);
            run(&mut deny)
        });
    } else {
        println!("skipped: cargo-deny is not installed");
        summary.skip("dependencies and licenses", "cargo-deny is not installed");
    }

    emit_ci_summary(&summary);
    Ok(())
}

fn run_fmt() -> Result<(), String> {
    let workspace = workspace_dir()?;
    println!("fmt: applying rustfmt");
    run_cargo(&workspace, rustfmt_args(false))?;
    Ok(())
}

fn rustfmt_args(check: bool) -> Vec<&'static str> {
    let mut args = vec!["fmt", "--all"];
    if check {
        args.extend(["--", "--check"]);
    }
    args
}

fn test_args(release: bool) -> Vec<&'static str> {
    let mut args = vec!["test", "--locked", "--workspace", "--all-features"];
    if release {
        args.push("--release");
    }
    args
}

fn emit_ci_summary(summary: &CiSummary) {
    print!("{}", format_ci_summary(summary));
}

fn format_ci_summary(summary: &CiSummary) -> String {
    let mut out = String::from("\nci summary\n----------\n");
    for check in &summary.checks {
        match &check.status {
            CheckStatus::Passed => {
                writeln!(out, "  {:<13} {}", "passed", check.name).unwrap();
            }
            CheckStatus::Skipped(reason) => {
                writeln!(out, "  {:<13} {}: {reason}", "skipped", check.name).unwrap();
            }
            CheckStatus::Indeterminate(reason) => {
                writeln!(out, "  {:<13} {}: {reason}", "indeterminate", check.name).unwrap();
            }
            CheckStatus::Failed(error) => {
                writeln!(out, "  {:<13} {}: {error}", "failed", check.name).unwrap();
            }
        }
    }

    writeln!(out).unwrap();
    out.push_str(&format_coverage_section(&summary.coverage));

    let passed = count_status(&summary.checks, |status| {
        matches!(status, CheckStatus::Passed)
    });
    let skipped = count_status(&summary.checks, |status| {
        matches!(status, CheckStatus::Skipped(_))
    });
    let indeterminate = count_status(&summary.checks, |status| {
        matches!(status, CheckStatus::Indeterminate(_))
    });

    writeln!(out).unwrap();
    if let Some(failed) = summary
        .checks
        .iter()
        .find(|check| matches!(check.status, CheckStatus::Failed(_)))
    {
        writeln!(
            out,
            "result  failed  {}  ({passed} passed, {skipped} skipped, {indeterminate} indeterminate)",
            failed.name
        )
        .unwrap();
    } else {
        writeln!(
            out,
            "result  passed  {passed} passed, {skipped} skipped, {indeterminate} indeterminate"
        )
        .unwrap();
    }
    out
}

fn count_status(checks: &[CheckRecord], predicate: impl Fn(&CheckStatus) -> bool) -> usize {
    checks
        .iter()
        .filter(|check| predicate(&check.status))
        .count()
}

fn format_coverage_section(snapshots: &[CoverageSnapshot]) -> String {
    let mut out = String::new();
    if snapshots.is_empty() {
        writeln!(
            out,
            "coverage  not recorded (software execution; not a threshold or physical evidence)"
        )
        .unwrap();
        return out;
    }

    writeln!(
        out,
        "coverage  software execution only; not a threshold or physical evidence"
    )
    .unwrap();
    for snapshot in snapshots {
        writeln!(
            out,
            "  {}  lines {}  functions {}  regions {}",
            snapshot.label,
            format_metric(&snapshot.totals.lines),
            format_metric(&snapshot.totals.functions),
            format_metric(&snapshot.totals.regions),
        )
        .unwrap();
        for file in &snapshot.files {
            writeln!(
                out,
                "    {}  lines {}",
                file.path,
                format_metric(&file.totals.lines),
            )
            .unwrap();
        }
        writeln!(
            out,
            "    report  {}",
            snapshot.report.display().to_string().replace('\\', "/")
        )
        .unwrap();
    }
    out
}

fn write_coverage_summary_file(
    workspace: &Path,
    config: &CoverageConfig,
    snapshots: &[CoverageSnapshot],
) -> Result<(), String> {
    let path = workspace.join(&config.output_directory).join("summary.txt");
    fs::write(&path, format_coverage_section(snapshots)).map_err(|error| {
        format!(
            "could not write coverage summary {}: {error}",
            path.display()
        )
    })
}

fn workspace_dir() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask must be located directly under the workspace root".to_owned())
}

fn load_config() -> Result<CiConfig, String> {
    load_config_from(&Path::new(env!("CARGO_MANIFEST_DIR")).join(CONFIG_FILE))
}

fn load_config_from(path: &Path) -> Result<CiConfig, String> {
    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "could not read CI configuration {}: {error}",
            path.display()
        )
    })?;
    parse_config(path, &contents)
}

fn parse_config(path: &Path, contents: &str) -> Result<CiConfig, String> {
    let config: CiConfig = ron::from_str(contents).map_err(|error| {
        format!(
            "could not parse CI configuration {}: {error}",
            path.display()
        )
    })?;
    config
        .validate()
        .map_err(|error| format!("invalid CI configuration {}: {error}", path.display()))?;
    Ok(config)
}

impl CiConfig {
    fn validate(&self) -> Result<(), String> {
        require_nonempty("expected_version", &self.expected_version)?;
        require_nonempty("driver.package", &self.driver.package)?;
        validate_workspace_path("driver.manifest", &self.driver.manifest)?;

        if self.lifecycle_packages.is_empty() {
            return Err("lifecycle_packages must not be empty".to_owned());
        }
        if self.supported_targets.is_empty() {
            return Err("supported_targets must not be empty".to_owned());
        }

        let mut manifests = HashSet::new();
        for package in &self.lifecycle_packages {
            validate_workspace_path("lifecycle_packages[].manifest", &package.manifest)?;
            if !manifests.insert(package.manifest.as_str()) {
                return Err(format!(
                    "lifecycle package manifest `{}` is duplicated",
                    package.manifest
                ));
            }
        }

        if !manifests.contains(self.driver.manifest.as_str()) {
            return Err(format!(
                "driver manifest `{}` must appear in lifecycle_packages",
                self.driver.manifest
            ));
        }

        let mut targets = HashSet::new();
        for target in &self.supported_targets {
            require_nonempty("supported_targets[]", target)?;
            if !targets.insert(target.as_str()) {
                return Err(format!("supported target `{target}` is duplicated"));
            }
        }

        self.coverage.validate(&self.driver.package)?;

        Ok(())
    }
}

impl CoverageConfig {
    fn validate(&self, driver_package: &str) -> Result<(), String> {
        if self.unit_packages.is_empty() {
            return Err("coverage.unit_packages must not be empty".to_owned());
        }
        if self.infrastructure_packages.is_empty() {
            return Err("coverage.infrastructure_packages must not be empty".to_owned());
        }
        require_nonempty("coverage.conformance.package", &self.conformance.package)?;
        require_nonempty("coverage.conformance.test", &self.conformance.test)?;
        require_nonempty(
            "coverage.ignored_filename_regex",
            &self.ignored_filename_regex,
        )?;
        validate_workspace_path("coverage.output_directory", &self.output_directory)?;

        let unit_packages = unique_values("coverage.unit_packages", &self.unit_packages)?;
        let infrastructure_packages = unique_values(
            "coverage.infrastructure_packages",
            &self.infrastructure_packages,
        )?;

        if !unit_packages.contains(driver_package) {
            return Err(format!(
                "coverage.unit_packages must include driver package `{driver_package}`"
            ));
        }
        if unit_packages.contains(self.conformance.package.as_str()) {
            return Err("coverage conformance package must not be a unit package".to_owned());
        }
        if infrastructure_packages.contains(self.conformance.package.as_str()) {
            return Err(
                "coverage conformance package must not be an infrastructure package".to_owned(),
            );
        }
        if let Some(package) = unit_packages.intersection(&infrastructure_packages).next() {
            return Err(format!(
                "coverage package `{package}` cannot be both a unit and infrastructure package"
            ));
        }

        Ok(())
    }
}

fn unique_values<'a>(field: &str, values: &'a [String]) -> Result<HashSet<&'a str>, String> {
    let mut unique = HashSet::new();
    for value in values {
        require_nonempty(&format!("{field}[]"), value)?;
        if !unique.insert(value.as_str()) {
            return Err(format!("{field} entry `{value}` is duplicated"));
        }
    }
    Ok(unique)
}

fn require_nonempty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_workspace_path(field: &str, value: &str) -> Result<(), String> {
    require_nonempty(field, value)?;
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_)
                    | Component::RootDir
                    | Component::ParentDir
                    | Component::CurDir
            )
        })
    {
        return Err(format!(
            "{field} must be a workspace-relative path without `.` or `..`: `{value}`"
        ));
    }
    Ok(())
}

fn validate_manifest(
    workspace: &Path,
    package: &LifecyclePackage,
    expected_version: &str,
) -> Result<(), String> {
    let relative_path = package.manifest.as_str();
    let path = workspace.join(relative_path);
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("could not read {relative_path}: {error}"))?;
    validate_manifest_contents(
        relative_path,
        &contents,
        expected_version,
        package.require_unpublished,
    )
}

fn validate_manifest_contents(
    relative_path: &str,
    contents: &str,
    expected_version: &str,
    require_unpublished: bool,
) -> Result<(), String> {
    let package_name = package_field(contents, "name");
    let actual_version = package_field(contents, "version");
    let (Some(package_name), Some(actual_version)) = (package_name, actual_version) else {
        return Err(format!(
            "could not read a package name and version from {relative_path}"
        ));
    };

    if actual_version != expected_version {
        return Err(format!(
            "expected {package_name} version {expected_version}, found {actual_version}"
        ));
    }

    if require_unpublished {
        let publish_setting = package_field(contents, "publish");
        if publish_setting.as_deref() != Some("false") {
            return Err(format!(
                "{relative_path} [package] must retain publish = false, found {}",
                publish_setting.as_deref().unwrap_or("no publish key")
            ));
        }
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

fn run_coverage(
    workspace: &Path,
    config: &CoverageConfig,
) -> Result<Vec<CoverageSnapshot>, String> {
    if !executable_available("cargo-llvm-cov") {
        return Err(
            "cargo-llvm-cov is required for coverage; install it with `cargo install cargo-llvm-cov --locked`"
                .to_owned(),
        );
    }

    let output_directory = workspace.join(&config.output_directory);
    fs::create_dir_all(&output_directory).map_err(|error| {
        format!(
            "could not create coverage output directory {}: {error}",
            output_directory.display()
        )
    })?;

    let unit_report = output_directory.join("unit.json");
    println!("check: unit test coverage");
    remove_stale_report(&unit_report)?;
    run_cargo(workspace, unit_coverage_args(config, &unit_report))?;
    let unit = coverage_snapshot(workspace, "unit tests", &unit_report)?;

    let conformance_report = output_directory.join("conformance.json");
    println!("check: model-conformance coverage");
    remove_stale_report(&conformance_report)?;
    run_cargo(
        workspace,
        conformance_coverage_args(config, &conformance_report),
    )?;
    let conformance = coverage_snapshot(workspace, "model conformance", &conformance_report)?;

    Ok(vec![unit, conformance])
}

fn coverage_snapshot(
    workspace: &Path,
    label: &str,
    path: &Path,
) -> Result<CoverageSnapshot, String> {
    let contents = fs::read_to_string(path).map_err(|error| {
        format!(
            "could not read {label} coverage report {}: {error}",
            path.display()
        )
    })?;
    let summary = parse_coverage_summary(path, &contents)?;
    Ok(CoverageSnapshot {
        label: label.to_owned(),
        report: path.strip_prefix(workspace).unwrap_or(path).to_path_buf(),
        totals: summary.totals.clone(),
        files: summary
            .files
            .iter()
            .filter_map(|file| {
                Some(CoverageFileSnapshot {
                    path: display_coverage_path(workspace, &file.filename),
                    totals: file.summary.clone()?,
                })
            })
            .collect(),
    })
}

fn display_coverage_path(workspace: &Path, filename: &str) -> String {
    let path = Path::new(filename);
    path.strip_prefix(workspace)
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn unit_coverage_args(config: &CoverageConfig, output: &Path) -> Vec<OsString> {
    let mut args = vec![OsString::from("llvm-cov"), OsString::from("--locked")];
    for package in &config.unit_packages {
        args.push(OsString::from("--package"));
        args.push(OsString::from(package));
    }
    args.extend([
        OsString::from("--lib"),
        OsString::from("--all-features"),
        OsString::from("--json"),
        OsString::from("--summary-only"),
        OsString::from("--ignore-filename-regex"),
        OsString::from(&config.ignored_filename_regex),
        OsString::from("--output-path"),
        output.as_os_str().to_owned(),
    ]);
    args
}

fn conformance_coverage_args(config: &CoverageConfig, output: &Path) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("llvm-cov"),
        OsString::from("--locked"),
        OsString::from("--workspace"),
    ];
    for package in config
        .unit_packages
        .iter()
        .chain(&config.infrastructure_packages)
    {
        args.push(OsString::from("--exclude-from-test"));
        args.push(OsString::from(package));
    }
    for package in
        std::iter::once(&config.conformance.package).chain(&config.infrastructure_packages)
    {
        args.push(OsString::from("--exclude-from-report"));
        args.push(OsString::from(package));
    }
    args.extend([
        OsString::from("--test"),
        OsString::from(&config.conformance.test),
        OsString::from("--all-features"),
        OsString::from("--json"),
        OsString::from("--summary-only"),
        OsString::from("--ignore-filename-regex"),
        OsString::from(&config.ignored_filename_regex),
        OsString::from("--output-path"),
        output.as_os_str().to_owned(),
    ]);
    args
}

fn remove_stale_report(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "could not remove stale coverage report {}: {error}",
            path.display()
        )),
    }
}

fn parse_coverage_summary(path: &Path, contents: &str) -> Result<CoverageData, String> {
    let mut export: CoverageExport = serde_json::from_str(contents).map_err(|error| {
        format!(
            "could not parse coverage report {}: {error}",
            path.display()
        )
    })?;
    if export.data.len() != 1 {
        return Err(format!(
            "coverage report {} must contain exactly one data set, found {}",
            path.display(),
            export.data.len()
        ));
    }
    let summary = export.data.pop().expect("the length was checked above");
    if summary.files.is_empty()
        || summary
            .files
            .iter()
            .any(|file| file.filename.trim().is_empty())
        || summary.totals.lines.count == 0
    {
        return Err(format!(
            "coverage report {} contains no measured production lines",
            path.display()
        ));
    }
    for (name, metric) in [
        ("lines", &summary.totals.lines),
        ("functions", &summary.totals.functions),
        ("regions", &summary.totals.regions),
    ] {
        if metric.covered > metric.count {
            return Err(format!(
                "coverage report {} has {name} covered greater than total",
                path.display()
            ));
        }
    }
    Ok(summary)
}

fn format_metric(metric: &CoverageMetric) -> String {
    let percent = if metric.count == 0 {
        0.0
    } else {
        metric.covered as f64 * 100.0 / metric.count as f64
    };
    format!("{}/{} ({percent:.2}%)", metric.covered, metric.count)
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

fn run_package_command(
    workspace: &Path,
    driver_manifest: &str,
    allow_dirty: bool,
    list: bool,
) -> Result<(), String> {
    // Construction performs Cargo's verification build from the unpacked
    // archive; listing alone would not detect a required excluded source file.
    let mut command = cargo_command(workspace);
    command.args(["package", "--locked", "--manifest-path", driver_manifest]);
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

    const TEST_EXPECTED_VERSION: &str = "0.1.0-incubating.1";
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

    fn valid_config() -> CiConfig {
        CiConfig {
            expected_version: TEST_EXPECTED_VERSION.to_owned(),
            lifecycle_packages: vec![LifecyclePackage {
                manifest: "crates/sht4x/Cargo.toml".to_owned(),
                require_unpublished: true,
            }],
            driver: DriverPackage {
                package: "ph-sht4x-hts".to_owned(),
                manifest: "crates/sht4x/Cargo.toml".to_owned(),
            },
            supported_targets: vec!["thumbv7em-none-eabihf".to_owned()],
            coverage: CoverageConfig {
                unit_packages: vec!["ph-sht4x-hts".to_owned(), "ph-sht4x-hts-model".to_owned()],
                conformance: ConformanceCoverage {
                    package: "ph-sht4x-hts-conformance".to_owned(),
                    test: "conformance".to_owned(),
                },
                infrastructure_packages: vec!["xtask".to_owned()],
                ignored_filename_regex: "tests\\.rs$".to_owned(),
                output_directory: "target/coverage".to_owned(),
            },
        }
    }

    #[test]
    fn rustfmt_write_omits_check_and_the_gate_passes_it() {
        assert_eq!(rustfmt_args(false), ["fmt", "--all"]);
        assert_eq!(rustfmt_args(true), ["fmt", "--all", "--", "--check"]);
    }

    #[test]
    fn ci_summary_reports_checks_and_uses_coverage_totals() {
        let mut summary = CiSummary::default();
        summary.pass("formatting");
        summary.skip("dependencies and licenses", "cargo-deny is not installed");
        summary.indeterminate(
            "target thumbv7em-none-eabihf (release)",
            "the installed-target list could not be read",
        );
        summary.coverage.push(CoverageSnapshot {
            label: "unit tests".to_owned(),
            report: PathBuf::from("target").join("coverage").join("unit.json"),
            totals: CoverageTotals {
                functions: CoverageMetric {
                    count: 3,
                    covered: 2,
                },
                lines: CoverageMetric {
                    count: 10,
                    covered: 9,
                },
                regions: CoverageMetric {
                    count: 12,
                    covered: 10,
                },
            },
            files: vec![CoverageFileSnapshot {
                path: "crates/sht4x/src/lib.rs".to_owned(),
                totals: CoverageTotals {
                    functions: CoverageMetric {
                        count: 3,
                        covered: 2,
                    },
                    lines: CoverageMetric {
                        count: 10,
                        covered: 9,
                    },
                    regions: CoverageMetric {
                        count: 12,
                        covered: 10,
                    },
                },
            }],
        });

        let rendered = format_ci_summary(&summary);
        assert!(rendered.contains("passed        formatting"));
        assert!(
            rendered
                .contains("skipped       dependencies and licenses: cargo-deny is not installed")
        );
        assert!(rendered.contains("indeterminate target thumbv7em-none-eabihf (release): the installed-target list could not be read"));
        assert!(
            rendered.contains(
                "coverage  software execution only; not a threshold or physical evidence"
            )
        );
        assert!(rendered.contains(
            "unit tests  lines 9/10 (90.00%)  functions 2/3 (66.67%)  regions 10/12 (83.33%)"
        ));
        assert!(rendered.contains("crates/sht4x/src/lib.rs  lines 9/10 (90.00%)"));
        assert!(rendered.contains("report  target/coverage/unit.json"));
        assert!(rendered.contains("result  passed  1 passed, 1 skipped, 1 indeterminate"));
        assert!(!rendered.contains("result  failed"));
    }

    #[test]
    fn ci_summary_names_the_failed_check() {
        let mut summary = CiSummary::default();
        summary.pass("formatting");
        summary.fail("clippy", "warnings denied");
        let rendered = format_ci_summary(&summary);
        assert!(rendered.contains("failed        clippy: warnings denied"));
        assert!(rendered.contains("coverage  not recorded"));
        assert!(
            rendered.contains("result  failed  clippy  (1 passed, 0 skipped, 0 indeterminate)")
        );
    }

    #[test]
    fn test_invocation_runs_debug_and_optimized_profiles() {
        assert_eq!(
            test_args(false),
            ["test", "--locked", "--workspace", "--all-features"]
        );
        assert_eq!(
            test_args(true),
            [
                "test",
                "--locked",
                "--workspace",
                "--all-features",
                "--release",
            ]
        );
    }

    #[test]
    fn committed_configuration_loads_and_propagates_repository_policy() {
        let config = load_config().unwrap();

        assert_eq!(config.expected_version, TEST_EXPECTED_VERSION);
        assert_eq!(config.lifecycle_packages.len(), 3);
        assert!(
            config
                .lifecycle_packages
                .iter()
                .all(|package| package.require_unpublished)
        );
        assert_eq!(config.driver.package, "ph-sht4x-hts");
        assert_eq!(config.driver.manifest, "crates/sht4x/Cargo.toml");
        assert_eq!(
            config.supported_targets,
            [
                "thumbv6m-none-eabi",
                "thumbv7m-none-eabi",
                "thumbv7em-none-eabihf",
                "thumbv8m.main-none-eabihf",
                "riscv32imac-unknown-none-elf",
            ]
        );
        assert_eq!(
            config.coverage.unit_packages,
            ["ph-sht4x-hts", "ph-sht4x-hts-model"]
        );
        assert_eq!(
            config.coverage.conformance.package,
            "ph-sht4x-hts-conformance"
        );
        assert_eq!(config.coverage.conformance.test, "conformance");
        assert_eq!(config.coverage.infrastructure_packages, ["xtask"]);
        assert_eq!(config.coverage.ignored_filename_regex, "tests\\.rs$");
        assert_eq!(config.coverage.output_directory, "target/coverage");
    }

    #[test]
    fn missing_and_malformed_configuration_errors_name_the_path() {
        let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("missing-ci-config.ron");
        let error = load_config_from(&missing).unwrap_err();
        assert!(error.contains(missing.to_string_lossy().as_ref()));
        assert!(error.contains("could not read CI configuration"));

        let malformed = Path::new("malformed-ci-config.ron");
        let error = parse_config(malformed, "this is not RON").unwrap_err();
        assert!(error.contains("malformed-ci-config.ron"));
        assert!(error.contains("could not parse CI configuration"));

        let missing_field = Path::new("missing-field-ci-config.ron");
        let error = parse_config(missing_field, "(supported_targets: [])").unwrap_err();
        assert!(error.contains("missing-field-ci-config.ron"));
        assert!(error.contains("expected_version"));
    }

    #[test]
    fn configuration_rejects_empty_required_values_and_lists() {
        let mut config = valid_config();
        config.expected_version.clear();
        assert!(config.validate().unwrap_err().contains("expected_version"));

        let mut config = valid_config();
        config.driver.package = "  ".to_owned();
        assert!(config.validate().unwrap_err().contains("driver.package"));

        let mut config = valid_config();
        config.driver.manifest.clear();
        assert!(config.validate().unwrap_err().contains("driver.manifest"));

        let mut config = valid_config();
        config.lifecycle_packages[0].manifest.clear();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("lifecycle_packages[].manifest")
        );

        let mut config = valid_config();
        config.supported_targets[0] = "  ".to_owned();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("supported_targets[]")
        );

        let mut config = valid_config();
        config.lifecycle_packages.clear();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("lifecycle_packages must not be empty")
        );

        let mut config = valid_config();
        config.supported_targets.clear();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("supported_targets must not be empty")
        );
    }

    #[test]
    fn configuration_rejects_duplicate_entries() {
        let mut config = valid_config();
        config
            .lifecycle_packages
            .push(config.lifecycle_packages[0].clone());
        assert!(config.validate().unwrap_err().contains("is duplicated"));

        let mut config = valid_config();
        config
            .supported_targets
            .push(config.supported_targets[0].clone());
        assert!(config.validate().unwrap_err().contains("is duplicated"));
    }

    #[test]
    fn configuration_rejects_invalid_paths_and_missing_driver_entry() {
        let mut config = valid_config();
        config.lifecycle_packages[0].manifest = "../outside/Cargo.toml".to_owned();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("workspace-relative path")
        );

        let mut config = valid_config();
        config.driver.manifest = "crates/other/Cargo.toml".to_owned();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("must appear in lifecycle_packages")
        );
    }

    #[test]
    fn coverage_configuration_rejects_empty_duplicate_and_overlapping_roles() {
        let mut config = valid_config();
        config.coverage.unit_packages.clear();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("coverage.unit_packages must not be empty")
        );

        let mut config = valid_config();
        config.coverage.infrastructure_packages.clear();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("coverage.infrastructure_packages must not be empty")
        );

        let mut config = valid_config();
        config
            .coverage
            .unit_packages
            .push("ph-sht4x-hts".to_owned());
        assert!(config.validate().unwrap_err().contains("is duplicated"));

        let mut config = valid_config();
        config.coverage.unit_packages.remove(0);
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("must include driver package")
        );

        let mut config = valid_config();
        config.coverage.conformance.package = "ph-sht4x-hts".to_owned();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("must not be a unit package")
        );

        let mut config = valid_config();
        config.coverage.infrastructure_packages = vec!["ph-sht4x-hts-model".to_owned()];
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("both a unit and infrastructure package")
        );

        let mut config = valid_config();
        config.coverage.output_directory = "../coverage".to_owned();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("workspace-relative path")
        );

        let mut config = valid_config();
        config.coverage.ignored_filename_regex.clear();
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("coverage.ignored_filename_regex")
        );
    }

    #[test]
    fn coverage_commands_keep_unit_and_conformance_execution_separate() {
        let config = valid_config();
        let unit = unit_coverage_args(&config.coverage, Path::new("unit.json"));
        let unit = unit
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            unit,
            [
                "llvm-cov",
                "--locked",
                "--package",
                "ph-sht4x-hts",
                "--package",
                "ph-sht4x-hts-model",
                "--lib",
                "--all-features",
                "--json",
                "--summary-only",
                "--ignore-filename-regex",
                "tests\\.rs$",
                "--output-path",
                "unit.json",
            ]
        );

        let conformance =
            conformance_coverage_args(&config.coverage, Path::new("conformance.json"));
        let conformance = conformance
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            conformance,
            [
                "llvm-cov",
                "--locked",
                "--workspace",
                "--exclude-from-test",
                "ph-sht4x-hts",
                "--exclude-from-test",
                "ph-sht4x-hts-model",
                "--exclude-from-test",
                "xtask",
                "--exclude-from-report",
                "ph-sht4x-hts-conformance",
                "--exclude-from-report",
                "xtask",
                "--test",
                "conformance",
                "--all-features",
                "--json",
                "--summary-only",
                "--ignore-filename-regex",
                "tests\\.rs$",
                "--output-path",
                "conformance.json",
            ]
        );
    }

    #[test]
    fn stale_coverage_report_is_removed_before_a_run() {
        let directory =
            env::temp_dir().join(format!("ph-sht4x-hts-coverage-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let report = directory.join("stale.json");
        fs::write(&report, "stale").unwrap();

        remove_stale_report(&report).unwrap();
        assert!(!report.exists());
        remove_stale_report(&report).unwrap();

        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn coverage_summary_parser_rejects_malformed_empty_and_zero_line_reports() {
        let path = Path::new("coverage.json");
        assert!(parse_coverage_summary(path, "not json").is_err());
        assert!(parse_coverage_summary(path, r#"{"data":[]}"#).is_err());
        assert!(
            parse_coverage_summary(
                path,
                r#"{"data":[{"files":[{"filename":"driver.rs"}],"totals":{"functions":{"count":1,"covered":1},"lines":{"count":0,"covered":0},"regions":{"count":1,"covered":1}}}]}"#,
            )
            .is_err()
        );
    }

    #[test]
    fn coverage_summary_parser_retains_measured_metrics() {
        let summary = parse_coverage_summary(
            Path::new("coverage.json"),
            r#"{"data":[{"files":[{"filename":"driver.rs"}],"totals":{"functions":{"count":3,"covered":2},"lines":{"count":10,"covered":9},"regions":{"count":12,"covered":10}}}]}"#,
        )
        .unwrap();

        assert_eq!(summary.files[0].filename, "driver.rs");
        assert_eq!(summary.totals.lines.count, 10);
        assert_eq!(summary.totals.lines.covered, 9);
        assert_eq!(format_metric(&summary.totals.lines), "9/10 (90.00%)");
    }

    #[test]
    fn coverage_summary_parser_retains_per_file_metrics() {
        let summary = parse_coverage_summary(
            Path::new("coverage.json"),
            r#"{"data":[{"files":[{"filename":"/ws/crates/sht4x/src/lib.rs","summary":{"functions":{"count":4,"covered":3},"lines":{"count":20,"covered":18},"regions":{"count":8,"covered":7}}}],"totals":{"functions":{"count":4,"covered":3},"lines":{"count":20,"covered":18},"regions":{"count":8,"covered":7}}}]}"#,
        )
        .unwrap();

        assert_eq!(summary.files[0].summary.as_ref().unwrap().lines.covered, 18);
        assert_eq!(
            display_coverage_path(Path::new("/ws"), "/ws/crates/sht4x/src/lib.rs"),
            "crates/sht4x/src/lib.rs"
        );
    }

    #[test]
    fn package_fields_come_only_from_the_package_table() {
        assert_eq!(
            package_field(VALID_MANIFEST, "name").as_deref(),
            Some("example")
        );
        assert_eq!(
            package_field(VALID_MANIFEST, "version").as_deref(),
            Some(TEST_EXPECTED_VERSION)
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
            TEST_EXPECTED_VERSION,
            true,
        )
        .unwrap_err();
        assert!(error.contains("could not read a package name and version"));
    }

    #[test]
    fn wrong_version_is_rejected() {
        let error = validate_manifest_contents(
            "Cargo.toml",
            "[package]\nname = \"example\"\nversion = \"1.0.0\"\npublish = false\n",
            TEST_EXPECTED_VERSION,
            true,
        )
        .unwrap_err();
        assert!(error.contains("expected example version"));
    }

    #[test]
    fn missing_or_enabled_publication_is_rejected() {
        for publish_line in ["", "publish = true\n"] {
            let manifest = format!(
                "[package]\nname = \"example\"\nversion = \"{TEST_EXPECTED_VERSION}\"\n{publish_line}"
            );
            let error =
                validate_manifest_contents("Cargo.toml", &manifest, TEST_EXPECTED_VERSION, true)
                    .unwrap_err();
            assert!(error.contains("must retain publish = false"));
        }
    }

    #[test]
    fn publication_check_can_be_disabled_by_policy() {
        let manifest = format!(
            "[package]\nname = \"example\"\nversion = \"{TEST_EXPECTED_VERSION}\"\npublish = [\"crates-io\"]\n"
        );
        assert_eq!(
            validate_manifest_contents("Cargo.toml", &manifest, TEST_EXPECTED_VERSION, false,),
            Ok(())
        );
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
