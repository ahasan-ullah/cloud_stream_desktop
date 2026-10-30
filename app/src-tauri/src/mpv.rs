//! Hands a stream to an external mpv player, for formats the webview cannot decode
//! (MKV, HEVC, DASH, ...).

use std::{collections::HashMap, path::PathBuf, process::Command};

pub fn open(
    mpv_path: Option<&str>,
    url: &str,
    title: &str,
    headers: &HashMap<String, String>,
    subtitles: &[String],
) -> Result<(), String> {
    let mpv = mpv_path
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
        .or_else(find_mpv)
        .ok_or("mpv was not found. Install it (https://mpv.io) or set its path in Settings.")?;

    let mut cmd = Command::new(&mpv);
    cmd.arg(format!("--force-media-title={title}"));
    for (key, value) in headers {
        match key.to_ascii_lowercase().as_str() {
            "referer" => cmd.arg(format!("--referrer={value}")),
            "user-agent" => cmd.arg(format!("--user-agent={value}")),
            // `-append` adds one entry, so values containing commas stay intact.
            _ => cmd.arg(format!("--http-header-fields-append={key}: {value}")),
        };
    }
    for sub in subtitles {
        cmd.arg(format!("--sub-file={sub}"));
    }
    cmd.arg("--").arg(url);

    cmd.spawn()
        .map(drop)
        .map_err(|e| format!("Failed to launch {}: {e}", mpv.display()))
}

fn find_mpv() -> Option<PathBuf> {
    let name = if cfg!(windows) { "mpv.exe" } else { "mpv" };
    let on_path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(name));

    let common: Vec<PathBuf> = if cfg!(target_os = "macos") {
        ["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv", "/Applications/mpv.app/Contents/MacOS/mpv"]
            .iter()
            .map(PathBuf::from)
            .collect()
    } else if cfg!(windows) {
        ["ProgramFiles", "LOCALAPPDATA", "USERPROFILE"]
            .iter()
            .filter_map(std::env::var_os)
            .flat_map(|base| {
                let base = PathBuf::from(base);
                [base.join("mpv").join(name), base.join("scoop").join("shims").join(name)]
            })
            .collect()
    } else {
        Vec::new()
    };

    on_path.chain(common).find(|p| p.is_file())
}
