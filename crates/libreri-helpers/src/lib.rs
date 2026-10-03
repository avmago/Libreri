//! Helper programs Libreri runs rather than links: DjVuLibre (DjVu pages
//! and text, GPL-2.0), Tesseract (OCR, Apache-2.0) and, optionally, unar
//! (ACE comic archives, LGPL). Keeping them separate programs keeps their
//! licences separate from Libreri's (see docs/adr/0016).
//!
//! This crate finds them — on `PATH` and in the places their installers use,
//! which apps started from the Finder or the Start menu do not see — and
//! installs them on request with the computer's own package manager
//! (Homebrew, winget, apt, dnf, pacman, zypper). In the Flatpak they are
//! built in instead.

pub mod download;
pub mod fonts;
pub mod ink;
pub mod speech;
pub mod tessdata;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// A helper program (or a set of programs from one package).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Helper {
    #[serde(rename = "djvulibre")]
    DjVuLibre,
    Tesseract,
    Unar,
    #[serde(rename = "espeak")]
    Espeak,
}

impl Helper {
    pub const ALL: [Helper; 4] = [
        Helper::DjVuLibre,
        Helper::Tesseract,
        Helper::Unar,
        Helper::Espeak,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::DjVuLibre => "DjVuLibre",
            Self::Tesseract => "Tesseract",
            Self::Unar => "unar",
            Self::Espeak => "eSpeak NG",
        }
    }

    /// What Libreri uses it for, for people.
    pub fn purpose(self) -> &'static str {
        match self {
            Self::DjVuLibre => "Opens DjVu books: pages, text and contents.",
            Self::Tesseract => "Reads the text of scanned books (OCR) so they can be searched.",
            Self::Unar => "Opens comics packed as ACE archives (.cba). Optional.",
            Self::Espeak => {
                "Reads books aloud where the system has no voices of its own (often on Linux), and turns words into sounds for natural voices. Optional."
            }
        }
    }

    pub fn licence(self) -> &'static str {
        match self {
            Self::DjVuLibre => "GPL-2.0",
            Self::Tesseract => "Apache-2.0",
            Self::Unar => "LGPL-2.1",
            Self::Espeak => "GPL-3.0",
        }
    }

    /// The programs Libreri runs; all must be found.
    pub fn programs(self) -> &'static [&'static str] {
        match self {
            Self::DjVuLibre => &["ddjvu", "djvutxt", "djvused"],
            Self::Tesseract => &["tesseract"],
            Self::Unar => &["unar", "lsar"],
            Self::Espeak => &["espeak-ng"],
        }
    }
}

/// Where program files are usually installed, beyond `PATH`.
fn extra_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        dirs.extend(
            ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"]
                .iter()
                .map(PathBuf::from),
        );
    } else if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var) {
                let base = PathBuf::from(base);
                dirs.push(base.join("Tesseract-OCR"));
                dirs.push(base.join("DjVuLibre"));
                dirs.push(base.join("eSpeak NG"));
                dirs.push(base.join("Programs").join("Tesseract-OCR"));
            }
        }
    } else {
        dirs.extend(
            [
                "/usr/bin",
                "/usr/local/bin",
                "/home/linuxbrew/.linuxbrew/bin",
                "/snap/bin",
            ]
            .iter()
            .map(PathBuf::from),
        );
    }
    dirs
}

fn executable_name(program: &str) -> String {
    if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_owned()
    }
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

/// Searches `dirs` for `program`.
pub fn find_in(program: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let name = executable_name(program);
    dirs.iter()
        .map(|d| d.join(&name))
        .find(|p| is_executable(p))
}

fn cache() -> &'static Mutex<HashMap<String, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Where `program` is, if installed. Remembered until [`refresh`].
pub fn find_program(program: &str) -> Option<PathBuf> {
    let mut c = cache().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(found) = c.get(program) {
        return found.clone();
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend(extra_dirs());
    let found = find_in(program, &dirs);
    c.insert(program.to_owned(), found.clone());
    found
}

/// Forgets what was found (after installing).
pub fn refresh() {
    cache().lock().unwrap_or_else(|p| p.into_inner()).clear();
}

/// A command for `program` that works when the app was not started from a
/// terminal: its folder is added to `PATH` so helpers can find each other.
pub fn command(program: &str) -> Option<Command> {
    let path = find_program(program)?;
    let mut cmd = Command::new(&path);
    if let Some(dir) = path.parent() {
        let mut dirs = vec![dir.to_path_buf()];
        if let Some(p) = std::env::var_os("PATH") {
            dirs.extend(std::env::split_paths(&p));
        }
        if let Ok(joined) = std::env::join_paths(dirs) {
            cmd.env("PATH", joined);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up.
        cmd.creation_flags(0x0800_0000);
    }
    Some(cmd)
}

/// Is a helper there, and which version.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HelperStatus {
    pub helper: Helper,
    pub installed: bool,
    /// Where the main program is.
    pub path: Option<String>,
    pub version: Option<String>,
    /// Programs that are missing.
    pub missing: Vec<String>,
}

fn version_of(helper: Helper) -> Option<String> {
    let (program, args): (&str, &[&str]) = match helper {
        Helper::Tesseract => ("tesseract", &["--version"]),
        Helper::DjVuLibre => ("ddjvu", &["--help"]),
        Helper::Unar => ("unar", &["-v"]),
        Helper::Espeak => ("espeak-ng", &["--version"]),
    };
    let out = command(program)?.args(args).output().ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // "tesseract 5.3.4", "DDJVU --- DjVuLibre-3.5.28", "unar v1.10.8".
    text.lines().find_map(|l| {
        let l = l.trim();
        if let Some(v) = l.strip_prefix("tesseract ") {
            return Some(v.to_owned());
        }
        if let Some(i) = l.find("DjVuLibre-") {
            return Some(l[i + "DjVuLibre-".len()..].trim().to_owned());
        }
        if let Some(rest) = l.strip_prefix("eSpeak NG text-to-speech: ") {
            return rest.split_whitespace().next().map(str::to_owned);
        }
        l.strip_prefix("unar v").map(str::to_owned)
    })
}

pub fn status(helper: Helper) -> HelperStatus {
    let missing: Vec<String> = helper
        .programs()
        .iter()
        .filter(|p| find_program(p).is_none())
        .map(|p| (*p).to_owned())
        .collect();
    let installed = missing.is_empty();
    HelperStatus {
        helper,
        installed,
        path: find_program(helper.programs()[0]).map(|p| p.to_string_lossy().into_owned()),
        version: installed.then(|| version_of(helper)).flatten(),
        missing,
    }
}

/// The package manager Libreri can use on this computer.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Installer {
    Homebrew,
    Winget,
    Apt,
    Dnf,
    Pacman,
    Zypper,
}

impl Installer {
    pub fn name(self) -> &'static str {
        match self {
            Self::Homebrew => "Homebrew",
            Self::Winget => "winget",
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Pacman => "pacman",
            Self::Zypper => "zypper",
        }
    }

    fn program(self) -> &'static str {
        match self {
            Self::Homebrew => "brew",
            Self::Winget => "winget",
            Self::Apt => "apt-get",
            Self::Dnf => "dnf",
            Self::Pacman => "pacman",
            Self::Zypper => "zypper",
        }
    }

    /// The package that provides `helper`, if this package manager has one.
    pub fn package(self, helper: Helper) -> Option<&'static str> {
        Some(match (self, helper) {
            (Self::Homebrew, Helper::DjVuLibre) => "djvulibre",
            (Self::Homebrew, Helper::Tesseract) => "tesseract",
            (Self::Homebrew, Helper::Unar) => "unar",
            (Self::Winget, Helper::DjVuLibre) => "DjVuLibre.DjView",
            (Self::Winget, Helper::Tesseract) => "UB-Mannheim.TesseractOCR",
            (Self::Winget, Helper::Unar) => return None,
            (Self::Apt, Helper::DjVuLibre) => "djvulibre-bin",
            (Self::Apt, Helper::Tesseract) => "tesseract-ocr",
            (Self::Apt, Helper::Unar) => "unar",
            (Self::Dnf, Helper::DjVuLibre) => "djvulibre",
            (Self::Dnf, Helper::Tesseract) => "tesseract",
            (Self::Dnf, Helper::Unar) => "unar",
            (Self::Pacman, Helper::DjVuLibre) => "djvulibre",
            (Self::Pacman, Helper::Tesseract) => "tesseract",
            (Self::Pacman, Helper::Unar) => "unarchiver",
            (Self::Zypper, Helper::DjVuLibre) => "djvulibre",
            (Self::Zypper, Helper::Tesseract) => "tesseract-ocr",
            (Self::Zypper, Helper::Unar) => "unar",
            (Self::Winget, Helper::Espeak) => "eSpeak-NG.eSpeak-NG",
            (_, Helper::Espeak) => "espeak-ng",
        })
    }
}

/// Running as a Flatpak: helpers come built in, and the sandbox cannot use
/// the computer's package manager.
pub fn in_flatpak() -> bool {
    std::env::var_os("FLATPAK_ID").is_some()
}

/// The package manager of this computer, if Libreri knows one that is
/// installed. On macOS without Homebrew, `None`: the app explains how to
/// install Homebrew first. Never one inside a Flatpak.
pub fn installer() -> Option<Installer> {
    if in_flatpak() {
        return None;
    }
    let candidates: &[Installer] = if cfg!(target_os = "macos") {
        &[Installer::Homebrew]
    } else if cfg!(windows) {
        &[Installer::Winget]
    } else {
        &[
            Installer::Apt,
            Installer::Dnf,
            Installer::Pacman,
            Installer::Zypper,
            Installer::Homebrew,
        ]
    };
    candidates
        .iter()
        .copied()
        .find(|i| find_program(i.program()).is_some())
}

/// What will be run to install a helper, shown to the user first.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub helper: Helper,
    pub installer: Installer,
    /// The command as a person would type it.
    pub display: String,
    /// The system asks for an administrator password.
    pub needs_admin: bool,
    #[serde(skip)]
    program: String,
    #[serde(skip)]
    args: Vec<String>,
}

/// How to install `helper` with `installer`; `None` if it has no package.
pub fn install_plan(helper: Helper, installer: Installer) -> Option<InstallPlan> {
    let package = installer.package(helper)?;
    // apt: refresh the package lists first, or a computer whose lists are
    // old (or were never fetched) cannot find the package. One `pkexec`,
    // so the password is asked for once. A failed refresh (one broken
    // source) does not stop the install.
    let apt_script = format!(
        "apt-get update || true; DEBIAN_FRONTEND=noninteractive apt-get install -y {package}"
    );
    let (program, args, needs_admin): (String, Vec<&str>, bool) = match installer {
        Installer::Homebrew => ("brew".into(), vec!["install", package], false),
        Installer::Winget => (
            "winget".into(),
            vec![
                "install",
                "--id",
                package,
                "--exact",
                "--silent",
                "--accept-package-agreements",
                "--accept-source-agreements",
            ],
            false,
        ),
        // Linux: pkexec shows the desktop's own password prompt.
        Installer::Apt => ("pkexec".into(), vec!["sh", "-c", &apt_script], true),
        Installer::Dnf => ("pkexec".into(), vec!["dnf", "install", "-y", package], true),
        Installer::Pacman => (
            "pkexec".into(),
            vec!["pacman", "-S", "--noconfirm", "--needed", package],
            true,
        ),
        Installer::Zypper => (
            "pkexec".into(),
            vec!["zypper", "--non-interactive", "install", package],
            true,
        ),
    };
    let mut display = if needs_admin {
        format!("sudo {}", args.join(" "))
    } else {
        format!("{program} {}", args.join(" "))
    };
    if installer == Installer::Winget {
        display = format!("winget install --id {package}");
    }
    if installer == Installer::Apt {
        display = format!("sudo apt-get update && sudo apt-get install -y {package}");
    }
    Some(InstallPlan {
        helper,
        installer,
        display,
        needs_admin,
        program,
        args: args.into_iter().map(str::to_owned).collect(),
    })
}

/// Runs an install, passing each line of output to `on_line`. Returns the
/// helper's status afterwards.
pub fn run_install(
    plan: &InstallPlan,
    mut on_line: impl FnMut(&str),
) -> Result<HelperStatus, String> {
    let mut cmd = if plan.program == "pkexec" {
        let mut c = Command::new("pkexec");
        // pkexec needs the full path of the program it runs.
        let target = find_program(&plan.args[0])
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| plan.args[0].clone());
        c.arg(target).args(&plan.args[1..]);
        c
    } else {
        let mut c = command(&plan.program)
            .ok_or_else(|| format!("{} was not found", plan.installer.name()))?;
        c.args(&plan.args);
        c
    };
    if plan.installer == Installer::Homebrew {
        cmd.env("NONINTERACTIVE", "1")
            .env("HOMEBREW_NO_ENV_HINTS", "1");
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", plan.installer.name()))?;
    let stderr = child.stderr.take();
    let err_lines = std::thread::spawn(move || {
        stderr
            .map(|s| {
                BufReader::new(s)
                    .lines()
                    .map_while(Result::ok)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            on_line(&line);
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let errors = err_lines.join().unwrap_or_default();
    for l in &errors {
        on_line(l);
    }
    refresh();
    let after = self::status(plan.helper);
    if after.installed {
        Ok(after)
    } else if status.success() {
        Err(format!(
            "{} finished, but Libreri still cannot find {}",
            plan.installer.name(),
            after.missing.join(", ")
        ))
    } else {
        let tail: Vec<&String> = errors.iter().rev().take(3).collect();
        let tail: Vec<&str> = tail.into_iter().rev().map(String::as_str).collect();
        Err(format!(
            "{} could not install {}{}",
            plan.installer.name(),
            plan.helper.name(),
            if tail.is_empty() {
                String::new()
            } else {
                format!(": {}", tail.join(" "))
            }
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_programs_in_given_folders() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(executable_name("ddjvu"));
        std::fs::write(&p, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(find_in("ddjvu", &[dir.path().to_path_buf()]), Some(p));
        assert_eq!(find_in("tesseract", &[dir.path().to_path_buf()]), None);
    }

    #[test]
    fn install_plans() {
        let p = install_plan(Helper::DjVuLibre, Installer::Homebrew).unwrap();
        assert_eq!(p.display, "brew install djvulibre");
        assert!(!p.needs_admin);
        let p = install_plan(Helper::Tesseract, Installer::Apt).unwrap();
        assert_eq!(
            p.display,
            "sudo apt-get update && sudo apt-get install -y tesseract-ocr"
        );
        // One password prompt: the refresh and the install run together.
        assert_eq!(p.program, "pkexec");
        assert_eq!(p.args[..2], ["sh", "-c"]);
        assert!(p.args[2].starts_with("apt-get update"));
        assert!(p.args[2].ends_with("apt-get install -y tesseract-ocr"));
        assert!(p.needs_admin);
        let p = install_plan(Helper::Tesseract, Installer::Winget).unwrap();
        assert_eq!(p.display, "winget install --id UB-Mannheim.TesseractOCR");
        assert!(install_plan(Helper::Unar, Installer::Winget).is_none());
    }

    #[test]
    fn status_reports_missing_programs() {
        // The build machine may or may not have them; the shape must hold.
        let s = status(Helper::DjVuLibre);
        assert_eq!(s.installed, s.missing.is_empty());
        if s.installed {
            assert!(s.path.is_some());
        }
    }
}
