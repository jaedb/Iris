#!/usr/bin/env python3

# Set [package.metadata.gstreamer] release_date to the date of the last commit.
# Failing that, to today.

try:
    # Python11 stdlib
    import tomllib
except ImportError:
    import tomli as tomllib

from datetime import datetime, timezone
from pathlib import Path
import re
import subprocess

path = Path(__file__).parent

try:
    release_date = subprocess.check_output(['git', 'show', '-s', '--format=%cd', '--date=short', 'HEAD'], shell=False, cwd=path, stderr=None, text=True).strip()
except subprocess.CalledProcessError:
    release_date = datetime.now(timezone.utc).strftime('%Y-%m-%d')

with (path / 'Cargo.toml').open('rb') as f:
    members = tomllib.load(f)['workspace']['members']

    for lib in members:
        g = path / lib /'Cargo.toml'
        manifest = g.open('r', encoding='utf-8').read()

        if 'package.metadata.gstreamer' not in manifest:
            # add release date
            manifest += f'\n[package.metadata.gstreamer]\nrelease_date = "{release_date}"\n'
        else:
            manifest = re.sub(r'release_date = "[0-9]+-[0-9]+-[0-9]+"', f'release_date = "{release_date}"', manifest)

        g.open('w', encoding='utf-8').write(manifest)
