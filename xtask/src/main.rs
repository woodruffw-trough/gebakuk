#![forbid(unsafe_code)]

use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() {
    if let Err(error) = execute() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| io::Error::other("xtask must be inside the workspace"))?;
    let mut arguments = env::args().skip(1);
    let task = arguments.next();
    if arguments.next().is_some() {
        return Err(
            io::Error::other("Usage: cargo xtask <build|install-tools|tool-versions>").into(),
        );
    }
    match task.as_deref() {
        Some("build") => build(root),
        Some("install-tools") => install_tools(root),
        Some("tool-versions") => {
            let versions = ToolVersions::read(root)?;
            println!("wasm_bindgen={}", versions.bindgen);
            println!("wasm_tools={}", versions.validator);
            Ok(())
        }
        None | Some("help" | "--help" | "-h") => {
            println!("Usage: cargo xtask <build|install-tools|tool-versions>");
            Ok(())
        }
        Some(other) => Err(io::Error::other(format!("Unknown task: {other}")).into()),
    }
}

struct ToolVersions {
    bindgen: String,
    validator: String,
}

impl ToolVersions {
    fn read(root: &Path) -> Result<Self> {
        Ok(Self {
            bindgen: locked_version(
                &fs::read_to_string(root.join("Cargo.lock"))?,
                "wasm-bindgen",
            )?,
            validator: quoted_value(
                &fs::read_to_string(root.join("xtask/Cargo.toml"))?,
                "[package.metadata.tools]",
                "wasm-tools",
            )?,
        })
    }

    fn check(&self) -> Result<()> {
        require_version("wasm-bindgen", &self.bindgen, "cargo xtask install-tools")?;
        require_version("wasm-tools", &self.validator, "cargo xtask install-tools")
    }
}

fn install_tools(root: &Path) -> Result<()> {
    let versions = ToolVersions::read(root)?;
    for (binary, package, version, extra) in [
        (
            "wasm-bindgen",
            "wasm-bindgen-cli",
            versions.bindgen.as_str(),
            &[][..],
        ),
        (
            "wasm-tools",
            "wasm-tools",
            versions.validator.as_str(),
            &["--no-default-features", "--features", "validate"][..],
        ),
    ] {
        if installed_version(binary)?.as_deref() == Some(&format!("{binary} {version}")) {
            println!("{binary} {version} is installed");
            continue;
        }
        let mut command = Command::new(cargo());
        command.current_dir(root).args([
            "install",
            "--locked",
            "--force",
            package,
            "--version",
            version,
        ]);
        command.args(extra);
        run(&mut command)?;
    }
    versions.check()
}

fn build(root: &Path) -> Result<()> {
    ToolVersions::read(root)?.check()?;
    run(Command::new(cargo()).current_dir(root).args([
        "build",
        "--locked",
        "-p",
        "presenter-web",
        "--release",
        "--target",
        "wasm32-unknown-unknown",
    ]))?;

    let dist = root.join("dist");
    if dist.exists() {
        fs::remove_dir_all(&dist)?;
    }
    fs::create_dir_all(dist.join("pkg"))?;
    copy_directory(&root.join("web"), &dist)?;
    let target =
        env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), |path| root.join(path));
    run(Command::new("wasm-bindgen")
        .current_dir(root)
        .args([
            "--target",
            "web",
            "--remove-name-section",
            "--remove-producers-section",
            "--out-dir",
        ])
        .arg(dist.join("pkg"))
        .args(["--out-name", "presenter_web"])
        .arg(target.join("wasm32-unknown-unknown/release/presenter_web.wasm")))?;

    let wasm = dist.join("pkg/presenter_web_bg.wasm");
    run(Command::new("wasm-tools").arg("validate").arg(&wasm))?;
    // Validate the packaged module, including a check that it requires SIMD.
    let scalar = Command::new("wasm-tools")
        .args(["validate", "--features=-simd,-relaxed-simd"])
        .arg(&wasm)
        .output()?;
    validate_simd(&scalar)?;
    fs::write(dist.join(".nojekyll"), [])?;
    println!("Validated SIMD128 application built in {}", dist.display());
    Ok(())
}

fn validate_simd(output: &Output) -> Result<()> {
    if output.status.success() {
        return Err(
            io::Error::other("The packaged WASM contains no required SIMD instructions").into(),
        );
    }
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    if !diagnostics.contains("SIMD support is not enabled") {
        return Err(
            io::Error::other(format!("Unexpected SIMD validation failure: {diagnostics}")).into(),
        );
    }
    Ok(())
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn cargo() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

fn run(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    if !status.success() {
        return Err(io::Error::other(format!("{command:?} failed with {status}")).into());
    }
    Ok(())
}

fn installed_version(binary: &str) -> Result<Option<String>> {
    match Command::new(binary).arg("--version").output() {
        Ok(output) if output.status.success() => {
            Ok(Some(String::from_utf8(output.stdout)?.trim().to_owned()))
        }
        Ok(_) => Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn require_version(binary: &str, version: &str, instruction: &str) -> Result<()> {
    if installed_version(binary)?.as_deref() != Some(&format!("{binary} {version}")) {
        return Err(io::Error::other(format!(
            "{binary} must be exactly {version}. Run `{instruction}`.",
        ))
        .into());
    }
    Ok(())
}

// Cargo emits these package records using quoted strings. Read only that stable
// generated format rather than adding a TOML dependency to the build task.
fn locked_version(lock: &str, package: &str) -> Result<String> {
    let mut found = None;
    for record in lock.split("[[package]]").skip(1) {
        if string_field(record, "name").as_deref() != Some(package) {
            continue;
        }
        let version = string_field(record, "version")
            .ok_or_else(|| io::Error::other(format!("Cargo.lock has no version for {package}")))?;
        if found.replace(version).is_some() {
            return Err(io::Error::other(format!(
                "Cargo.lock contains multiple versions of {package}"
            ))
            .into());
        }
    }
    found.ok_or_else(|| io::Error::other(format!("Cargo.lock does not contain {package}")).into())
}

fn quoted_value(manifest: &str, section: &str, field: &str) -> Result<String> {
    let body = manifest
        .split_once(section)
        .map(|(_, body)| body)
        .ok_or_else(|| io::Error::other(format!("xtask/Cargo.toml has no {section}")))?;
    string_field(body, field).ok_or_else(|| {
        io::Error::other(format!("xtask/Cargo.toml has no {field} in {section}")).into()
    })
}

fn string_field(record: &str, field: &str) -> Option<String> {
    for line in record.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == field
        {
            return value
                .trim()
                .strip_prefix('"')?
                .strip_suffix('"')
                .map(str::to_owned);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{locked_version, quoted_value};

    #[test]
    fn bindgen_version_tracks_the_resolved_lockfile() {
        let lock = "version = 4\n[[package]]\nname = \"wasm-bindgen-macro\"\nversion = \"0.2.1\"\n[[package]]\nversion = \"0.2.130\"\nname = \"wasm-bindgen\"\n";
        assert_eq!(locked_version(lock, "wasm-bindgen").unwrap(), "0.2.130");
    }

    #[test]
    fn ambiguous_or_missing_bindgen_versions_fail() {
        assert!(locked_version("version = 4", "wasm-bindgen").is_err());
        let package = "[[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.130\"\n";
        assert!(locked_version(&package.repeat(2), "wasm-bindgen").is_err());
        assert!(locked_version("[[package]]\nname = \"wasm-bindgen\"\n", "wasm-bindgen").is_err());
        assert!(
            locked_version(
                "[[package]]\nname = \"wasm-bindgen\"\nversion = 130\n",
                "wasm-bindgen",
            )
            .is_err()
        );
    }

    #[test]
    fn tool_configuration_stays_in_its_table() {
        let manifest = "[package]\nname = \"xtask\"\n[package.metadata.tools]\nwasm-tools = \"1.245.1\"\n[next]\nother = \"value\"\n";
        assert_eq!(
            quoted_value(manifest, "[package.metadata.tools]", "wasm-tools").unwrap(),
            "1.245.1"
        );
        assert!(quoted_value(manifest, "[package.metadata.tools]", "other").is_err());
    }
}
