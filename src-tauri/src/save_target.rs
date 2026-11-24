//! Checks the target of a file the app writes on request of the UI (session
//! export, chart image). The path comes from the save dialog; the backend
//! accepts it only if it is a plain file with the expected extension in an
//! existing folder outside the app's own folders.

use std::path::{Component, Path, PathBuf};

/// The checked target: the canonical folder plus the file name.
pub fn check(path: &Path, extension: &str, own_dirs: &[&Path]) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("choose the file in the save dialog".into());
    }
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err("invalid file path".into());
    }
    let ext_ok = path
        .extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| x.eq_ignore_ascii_case(extension));
    if !ext_ok {
        return Err(format!("only .{extension} files"));
    }
    let name = path
        .file_name()
        .filter(|n| !n.to_string_lossy().starts_with('.'))
        .ok_or("invalid file name")?;
    let parent = path
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .filter(|p| p.is_dir())
        .ok_or("the folder does not exist")?;
    for dir in own_dirs {
        let inside = |d: &Path| parent.starts_with(d);
        if inside(dir) || dir.canonicalize().is_ok_and(|d| inside(&d)) {
            return Err("not inside the app's own folders".into());
        }
    }
    let target = parent.join(name);
    if let Ok(m) = std::fs::symlink_metadata(&target)
        && !m.file_type().is_file()
    {
        return Err("the target is not a regular file".into());
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ps2-save-target-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn accepts_a_plain_file_with_the_extension() {
        let d = tmp("ok");
        let own = d.join("own");
        std::fs::create_dir_all(&own).unwrap();
        assert_eq!(
            check(&d.join("s.json"), "json", &[&own]).unwrap(),
            d.join("s.json")
        );
        assert!(check(&d.join("S.XLSX"), "xlsx", &[&own]).is_ok());
        // an existing file may be replaced
        std::fs::write(d.join("old.csv"), "x").unwrap();
        assert!(check(&d.join("old.csv"), "csv", &[&own]).is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn refuses_other_targets() {
        let d = tmp("bad");
        let own = d.join("own");
        std::fs::create_dir_all(own.join("sub")).unwrap();
        let own_dirs: [&Path; 1] = [&own];
        // wrong or missing extension
        assert!(check(&d.join("s.toml"), "json", &own_dirs).is_err());
        assert!(check(&d.join("s"), "json", &own_dirs).is_err());
        assert!(check(&d.join("s.json.sh"), "json", &own_dirs).is_err());
        // relative, with .., hidden, missing folder
        assert!(check(Path::new("s.json"), "json", &own_dirs).is_err());
        assert!(check(&d.join("own/../s.json"), "json", &own_dirs).is_err());
        assert!(check(&d.join(".s.json"), "json", &own_dirs).is_err());
        assert!(check(&d.join("missing/s.json"), "json", &own_dirs).is_err());
        // inside the app's own folders
        assert!(check(&own.join("s.json"), "json", &own_dirs).is_err());
        assert!(check(&own.join("sub/s.json"), "json", &own_dirs).is_err());
        // a folder with the name of the file
        std::fs::create_dir_all(d.join("dir.json")).unwrap();
        assert!(check(&d.join("dir.json"), "json", &own_dirs).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_links() {
        let d = tmp("link");
        let own = d.join("own");
        std::fs::create_dir_all(&own).unwrap();
        let own_dirs: [&Path; 1] = [&own];
        std::fs::write(d.join("real.json"), "x").unwrap();
        std::os::unix::fs::symlink(d.join("real.json"), d.join("link.json")).unwrap();
        assert!(check(&d.join("link.json"), "json", &own_dirs).is_err());
        // a linked folder that leads into the own folders
        std::os::unix::fs::symlink(&own, d.join("to-own")).unwrap();
        assert!(check(&d.join("to-own/s.json"), "json", &own_dirs).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
