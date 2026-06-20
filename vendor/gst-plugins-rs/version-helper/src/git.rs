// Copyright (C) 2020 Guillaume Desmottes <guillaume.desmottes@collabora.com>
//
// Licensed under the MIT license, see the LICENSE file or <http://opensource.org/licenses/MIT>
//
// SPDX-License-Identifier: MIT
use std::path::Path;
use std::process::Command;

pub fn repo_hash<P: AsRef<Path>>(path: P) -> Option<(String, String)> {
    let git_path = path.as_ref().to_str();
    let mut args = match git_path {
        Some(path) => vec!["-C", path],
        None => vec![],
    };
    args.extend(["log", "-1", "--format=%h_%cd", "--date=short"]);
    let output = Command::new("git").args(&args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8(output.stdout).ok()?;
    let output = output.trim_end_matches('\n');
    let mut s = output.split('_');
    let hash = s.next()?;
    let date = s.next()?;

    let hash = if dirty(path) {
        format!("{hash}+")
    } else {
        hash.into()
    };

    Some((hash, date.into()))
}

fn dirty<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref().to_str();
    let mut args = match path {
        Some(path) => vec!["-C", path],
        None => vec![],
    };
    args.extend(["ls-files", "-m"]);
    match Command::new("git").args(&args).output() {
        Ok(modified_files) => !modified_files.stdout.is_empty(),
        Err(_) => false,
    }
}
