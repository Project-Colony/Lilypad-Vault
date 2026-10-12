//! Where Lilypad keeps its files, and the one-time move from where earlier
//! releases kept them.
//!
//! | Root | What | Linux | Windows | macOS |
//! |---|---|---|---|---|
//! | config | `preferences.json`, `oauth_tokens.json` | `~/.config/Colony/Lilypad` | `%LOCALAPPDATA%\Colony\Lilypad` | `~/Library/Application Support/Colony/Lilypad` |
//! | data | `vaults/`, `backups/`, `sync/`, `locks/`, `key.json` | `~/.local/share/Colony/Lilypad` | same as config | same as config |
//!
//! Config and data are the same directory on Windows and macOS; no file name
//! appears in both rows, so they never collide.
//!
//! An explicit directory (`--data-dir`, then `LILYPAD_DATA_DIR`) replaces both
//! roots, so a whole profile, GitHub sign-in included, lives in one place.
//!
//! Releases up to 0.2 kept everything in one directory, the one
//! `directories::ProjectDirs::from_path("Colony/Lilypad").config_dir()` gives:
//! `~/.config/Colony/Lilypad` on Linux, `%APPDATA%\Colony\Lilypad\config` (the
//! Roaming profile) on Windows and `~/Library/Application Support/Colony/Lilypad`
//! on macOS, with the settings in `settings.json`. The first start of this
//! release moves those files to the table above (see [`migrate`]).

use crate::error::{AppError, Result};
use crate::settings::Settings;
use fs2::FileExt;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The program name under `Colony/`, capitalised as Lilypad spells it.
const PROGRAM: &str = "Lilypad";

/// The preferences file, in the config root.
pub const PREFERENCES_FILE: &str = "preferences.json";

/// The GitHub token store, in the config root.
pub const TOKEN_FILE: &str = "oauth_tokens.json";

/// Written in the data root once the legacy files have been moved.
const MIGRATED_MARKER: &str = ".migrated";

/// The settings file of releases up to 0.2, converted to [`PREFERENCES_FILE`].
const LEGACY_SETTINGS_FILE: &str = "settings.json";

/// What releases up to 0.2 wrote beside their settings and that now belongs
/// in the data root. `locks/` is left behind: it only holds empty lock files.
const LEGACY_DATA_ENTRIES: [&str; 4] = ["vaults", "backups", "sync", "key.json"];

/// The two roots a profile lives in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    /// What the user chose: [`PREFERENCES_FILE`].
    pub config: PathBuf,
    /// What Lilypad produced: vaults, backups, sync state, locks, `key.json`.
    pub data: PathBuf,
}

/// Resolves the profile directories, creating them if necessary.
///
/// `override_dir` (the `--data-dir` flag) wins over `LILYPAD_DATA_DIR`; either
/// one holds the whole profile. Without them, the platform roots are used,
/// after moving the files of earlier releases there.
pub fn resolve(override_dir: Option<PathBuf>) -> Result<Dirs> {
    let explicit = override_dir.or_else(|| std::env::var_os("LILYPAD_DATA_DIR").map(PathBuf::from));
    let Some(dir) = explicit else {
        return platform_dirs();
    };
    check(&dir)?;
    fs::create_dir_all(&dir)
        .map_err(|e| AppError::Io(format!("failed to create data dir {}: {e}", dir.display())))?;
    let dirs = Dirs {
        config: dir.clone(),
        data: dir,
    };
    // A directory used by an earlier release holds `settings.json`: convert
    // it in place. Nothing else moves, and a failure only costs preferences.
    if let Err(e) = migrate(&dirs.data, &dirs) {
        eprintln!(
            "lilypad: could not convert the settings in {}: {e}",
            dirs.data.display()
        );
    }
    Ok(dirs)
}

/// The platform roots, resolved and migrated once per process.
fn platform_dirs() -> Result<Dirs> {
    static DIRS: OnceLock<std::result::Result<Dirs, String>> = OnceLock::new();
    DIRS.get_or_init(|| {
        let new = Dirs {
            config: colony_ui::paths::config_dir(PROGRAM).map_err(|e| e.to_string())?,
            data: colony_ui::paths::data_dir(PROGRAM).map_err(|e| e.to_string())?,
        };
        let Some(legacy) = legacy_root() else {
            return Ok(new);
        };
        match migrate(&legacy, &new) {
            Ok(()) => Ok(new),
            Err(e) => {
                // Never an empty profile: keep using the old directory this
                // run, and try again next start.
                eprintln!(
                    "lilypad: could not move your files from {} to {} ({e}); using {} for now",
                    legacy.display(),
                    new.data.display(),
                    legacy.display()
                );
                Ok(Dirs {
                    config: legacy.clone(),
                    data: legacy,
                })
            }
        }
    })
    .clone()
    .map_err(|e| AppError::Io(format!("could not determine the Lilypad directories: {e}")))
    .and_then(|dirs| {
        check(&dirs.config)?;
        check(&dirs.data)?;
        Ok(dirs)
    })
}

/// Rejects an empty path (an unset-looking `LILYPAD_DATA_DIR=""` would resolve
/// to the current working directory, so the CLI would read and write vaults
/// wherever it was launched) and a non-UTF-8 path (the store keeps the data
/// directory as a `String`, so a lossy conversion could silently point at a
/// different directory).
fn check(dir: &Path) -> Result<()> {
    if dir.as_os_str().is_empty() {
        return Err(AppError::Validation(
            "data directory path is empty".to_string(),
        ));
    }
    if dir.to_str().is_none() {
        return Err(AppError::Validation(
            "data directory path is not valid UTF-8".to_string(),
        ));
    }
    Ok(())
}

/// The directory releases up to 0.2 used. This is `dirs::config_dir()`, the
/// Roaming profile on Windows, on purpose: it is where those releases wrote.
fn legacy_root() -> Option<PathBuf> {
    let root = dirs::config_dir()?.join("Colony").join(PROGRAM);
    Some(if cfg!(windows) {
        root.join("config")
    } else {
        root
    })
}

/// One move of the migration.
#[derive(Debug)]
enum Step {
    /// Move a file or directory as is.
    Move { from: PathBuf, to: PathBuf },
    /// Convert the legacy `settings.json` into the preferences file.
    Settings { from: PathBuf, to: PathBuf },
}

/// What a step did, so a failure can undo it.
enum Done {
    Renamed { from: PathBuf, to: PathBuf },
    Created(PathBuf),
}

/// Moves what an earlier release left in `legacy` to `new`, once.
///
/// Each entry moves only if its destination is absent: a rename, or a copy
/// checked byte for byte when the rename fails (another volume). Copied
/// sources and the legacy directory itself are left in place. The marker in
/// the data root makes later starts skip the check, and a lock keeps the
/// desktop app, the CLI and the TUI from migrating at the same time. On any
/// error every step already taken is undone, so the caller can keep using
/// `legacy` with nothing missing.
fn migrate(legacy: &Path, new: &Dirs) -> io::Result<()> {
    let marker = new.data.join(MIGRATED_MARKER);
    if marker.exists() || plan(legacy, new).is_empty() {
        return Ok(());
    }

    let locks = new.data.join("locks");
    fs::create_dir_all(&locks)?;
    let lock = File::create(locks.join("migrate.lock"))?;
    lock.lock_exclusive()?;
    // Another program may have finished the move while this one waited.
    if marker.exists() {
        return Ok(());
    }

    let mut done = Vec::new();
    let result = plan(legacy, new)
        .into_iter()
        .try_for_each(|step| apply(step, &mut done))
        .and_then(|()| fs::write(&marker, b""));
    if result.is_err() {
        undo(done);
    }
    result
}

/// The steps left to take: entries present in `legacy` whose destination is
/// absent. Empty on a fresh install, after a migration, and when `legacy` is
/// the new location already (macOS, or an explicit directory).
fn plan(legacy: &Path, new: &Dirs) -> Vec<Step> {
    let pending = |from: &Path, to: &Path| from.exists() && from != to && !to.exists();
    let mut steps = Vec::new();
    for entry in LEGACY_DATA_ENTRIES {
        let (from, to) = (legacy.join(entry), new.data.join(entry));
        if pending(&from, &to) {
            steps.push(Step::Move { from, to });
        }
    }
    let (from, to) = (legacy.join(TOKEN_FILE), new.config.join(TOKEN_FILE));
    if pending(&from, &to) {
        steps.push(Step::Move { from, to });
    }
    let (from, to) = (
        legacy.join(LEGACY_SETTINGS_FILE),
        new.config.join(PREFERENCES_FILE),
    );
    if pending(&from, &to) {
        steps.push(Step::Settings { from, to });
    }
    steps
}

fn apply(step: Step, done: &mut Vec<Done>) -> io::Result<()> {
    match step {
        Step::Move { from, to } => {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            if fs::rename(&from, &to).is_ok() {
                done.push(Done::Renamed { from, to });
            } else {
                copy_verified(&from, &to)?;
                done.push(Done::Created(to));
            }
        }
        Step::Settings { from, to } => {
            // Settings are a convenience: an unreadable legacy file becomes
            // the defaults rather than a reason to keep the old directory.
            let settings = fs::read(&from)
                .ok()
                .and_then(|bytes| Settings::from_legacy_json(&bytes).ok())
                .unwrap_or_default();
            settings
                .save_to(&to)
                .map_err(|e| io::Error::other(e.to_string()))?;
            done.push(Done::Created(to));
        }
    }
    Ok(())
}

/// Copies a file or a directory tree to `to`, which must not exist, keeping
/// the permissions (vault, key and token files are owner-only), then checks
/// every file reads back the same as its source. On failure it removes what
/// it created, and only that.
fn copy_verified(from: &Path, to: &Path) -> io::Result<()> {
    let permissions = fs::metadata(from)?.permissions();
    let copied = if from.is_dir() {
        fs::create_dir(to)?;
        fs::read_dir(from).and_then(|entries| {
            entries.into_iter().try_for_each(|entry| {
                let entry = entry?;
                copy_verified(&entry.path(), &to.join(entry.file_name()))
            })
        })
    } else {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut target = options.open(to)?;
        io::copy(&mut File::open(from)?, &mut target)
            .and_then(|_| target.sync_all())
            .and_then(|()| {
                if fs::read(from)? == fs::read(to)? {
                    Ok(())
                } else {
                    Err(io::Error::other(format!(
                        "{} does not read back the same after copying",
                        to.display()
                    )))
                }
            })
    };
    let result = copied.and_then(|()| fs::set_permissions(to, permissions));
    if result.is_err() {
        let _ = if from.is_dir() {
            fs::remove_dir_all(to)
        } else {
            fs::remove_file(to)
        };
    }
    result
}

/// Undoes the steps of a failed migration, newest first. Removing a copy
/// loses nothing: a copy never deletes its source.
fn undo(done: Vec<Done>) {
    for step in done.into_iter().rev() {
        let _ = match step {
            Done::Renamed { from, to } => fs::rename(to, from),
            Done::Created(path) if path.is_dir() => fs::remove_dir_all(path),
            Done::Created(path) => fs::remove_file(path),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct Layout {
        _tmp: TempDir,
        legacy: PathBuf,
        new: Dirs,
    }

    /// A legacy root and the two new roots, all apart (the Windows and Linux
    /// case: the files really move).
    fn layout() -> Layout {
        let tmp = TempDir::new().unwrap();
        let legacy = tmp.path().join("legacy");
        let new = Dirs {
            config: tmp.path().join("config"),
            data: tmp.path().join("data"),
        };
        fs::create_dir_all(&legacy).unwrap();
        fs::create_dir_all(&new.config).unwrap();
        fs::create_dir_all(&new.data).unwrap();
        Layout {
            _tmp: tmp,
            legacy,
            new,
        }
    }

    /// What an earlier release left behind.
    fn write_legacy(legacy: &Path) {
        fs::create_dir_all(legacy.join("vaults")).unwrap();
        fs::write(legacy.join("vaults/personal.lily"), b"vault bytes").unwrap();
        fs::create_dir_all(legacy.join("backups")).unwrap();
        fs::write(legacy.join("backups/personal.bak"), b"backup bytes").unwrap();
        fs::create_dir_all(legacy.join("sync")).unwrap();
        fs::write(legacy.join("sync/personal.json"), b"{}").unwrap();
        fs::write(legacy.join("key.json"), b"key bytes").unwrap();
        fs::write(legacy.join(TOKEN_FILE), b"token bytes").unwrap();
        fs::write(
            legacy.join(LEGACY_SETTINGS_FILE),
            br#"{"auto_lock_minutes": 9, "theme": 8, "density": 2}"#,
        )
        .unwrap();
    }

    #[test]
    fn fresh_install_moves_nothing() {
        let l = layout();
        migrate(&l.legacy, &l.new).unwrap();
        assert!(!l.new.data.join(MIGRATED_MARKER).exists());
        assert!(!l.new.config.join(PREFERENCES_FILE).exists());
    }

    #[test]
    fn legacy_files_move_and_the_marker_is_written() {
        let l = layout();
        write_legacy(&l.legacy);
        migrate(&l.legacy, &l.new).unwrap();

        let data = &l.new.data;
        assert_eq!(
            fs::read(data.join("vaults/personal.lily")).unwrap(),
            b"vault bytes"
        );
        assert!(data.join("backups/personal.bak").exists());
        assert!(data.join("sync/personal.json").exists());
        assert_eq!(fs::read(data.join("key.json")).unwrap(), b"key bytes");
        assert_eq!(
            fs::read(l.new.config.join(TOKEN_FILE)).unwrap(),
            b"token bytes"
        );
        assert!(data.join(MIGRATED_MARKER).exists());
        assert!(!l.legacy.join("vaults").exists());
        // The legacy directory itself is never removed.
        assert!(l.legacy.exists());

        let settings = Settings::load(&l.new.config);
        assert_eq!(settings.auto_lock_minutes, 9);
        assert_eq!(settings.density, 2);
        assert_eq!(settings.standard.selected_theme, "gruvbox");
        assert_eq!(settings.standard.selected_variant, "light");
    }

    #[test]
    fn existing_destinations_are_left_alone() {
        let l = layout();
        write_legacy(&l.legacy);
        fs::create_dir_all(l.new.data.join("vaults")).unwrap();
        fs::write(l.new.data.join("vaults/personal.lily"), b"newer").unwrap();
        fs::write(l.new.config.join(PREFERENCES_FILE), b"{}").unwrap();
        migrate(&l.legacy, &l.new).unwrap();

        assert_eq!(
            fs::read(l.new.data.join("vaults/personal.lily")).unwrap(),
            b"newer"
        );
        assert!(l.legacy.join("vaults/personal.lily").exists());
        assert_eq!(
            fs::read(l.new.config.join(PREFERENCES_FILE)).unwrap(),
            b"{}"
        );
    }

    #[test]
    fn a_marked_profile_is_skipped() {
        let l = layout();
        fs::write(l.new.data.join(MIGRATED_MARKER), b"").unwrap();
        write_legacy(&l.legacy);
        migrate(&l.legacy, &l.new).unwrap();
        assert!(l.legacy.join("vaults/personal.lily").exists());
        assert!(!l.new.data.join("vaults").exists());
    }

    #[test]
    fn the_same_directory_only_converts_the_settings() {
        // macOS, or an explicit directory: legacy is the new location.
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_path_buf();
        write_legacy(&dir);
        let dirs = Dirs {
            config: dir.clone(),
            data: dir.clone(),
        };
        migrate(&dir, &dirs).unwrap();
        assert_eq!(
            fs::read(dir.join("vaults/personal.lily")).unwrap(),
            b"vault bytes"
        );
        assert_eq!(
            Settings::load(&dir).standard.selected_theme,
            "gruvbox",
            "settings.json converted to preferences.json"
        );
        assert!(dir.join(LEGACY_SETTINGS_FILE).exists());
    }

    #[test]
    fn copy_fallback_reads_back_the_same_tree() {
        let l = layout();
        write_legacy(&l.legacy);
        let to = l.new.data.join("vaults");
        copy_verified(&l.legacy.join("vaults"), &to).unwrap();
        assert_eq!(fs::read(to.join("personal.lily")).unwrap(), b"vault bytes");
        assert!(l.legacy.join("vaults/personal.lily").exists());
    }

    /// A destination that cannot be written: the migration fails, every step
    /// is undone and the legacy files are all still there, so the caller can
    /// keep using them.
    #[cfg(unix)]
    #[test]
    fn a_read_only_destination_keeps_everything_in_the_legacy_root() {
        use std::os::unix::fs::PermissionsExt;
        let l = layout();
        write_legacy(&l.legacy);
        // The data root is writable (the lock and the first moves succeed);
        // the config root is not, so the token move fails part way through.
        fs::set_permissions(&l.new.config, fs::Permissions::from_mode(0o555)).unwrap();
        let probe = l.new.config.join("probe");
        if fs::write(&probe, b"").is_ok() {
            // Running as root: permissions do not apply, nothing to test.
            return;
        }

        assert!(migrate(&l.legacy, &l.new).is_err());
        fs::set_permissions(&l.new.config, fs::Permissions::from_mode(0o755)).unwrap();

        for file in [
            "vaults/personal.lily",
            "backups/personal.bak",
            "sync/personal.json",
            "key.json",
            TOKEN_FILE,
            LEGACY_SETTINGS_FILE,
        ] {
            assert!(l.legacy.join(file).exists(), "{file} lost");
        }
        assert!(!l.new.data.join("vaults").exists());
        assert!(!l.new.data.join(MIGRATED_MARKER).exists());
    }

    #[test]
    fn legacy_root_is_where_earlier_releases_wrote() {
        let root = legacy_root().expect("legacy root");
        let suffix = if cfg!(windows) {
            Path::new("Colony").join("Lilypad").join("config")
        } else {
            Path::new("Colony").join("Lilypad")
        };
        assert!(root.ends_with(&suffix), "{}", root.display());
        if cfg!(windows) {
            assert!(
                root.starts_with(dirs::config_dir().unwrap()),
                "Roaming, as ProjectDirs gave"
            );
        }
    }

    #[test]
    fn an_explicit_directory_holds_the_whole_profile() {
        let tmp = TempDir::new().unwrap();
        let dirs = resolve(Some(tmp.path().to_path_buf())).unwrap();
        assert_eq!(dirs.config, tmp.path());
        assert_eq!(dirs.data, tmp.path());
    }

    #[test]
    fn an_empty_explicit_directory_is_refused() {
        assert!(matches!(
            resolve(Some(PathBuf::new())),
            Err(AppError::Validation(_))
        ));
    }
}
