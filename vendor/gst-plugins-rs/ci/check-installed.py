#!/usr/bin/env python3
# Check that all available plugins have been build and installed in the prefix
# directory passed in argument.
import sys
import os
import glob

from utils import iterate_plugins

prefix = sys.argv[1]

plugins = glob.glob(os.path.join(
    prefix, '**', 'gstreamer-1.0', '*.so'), recursive=True)
plugins = list(map(os.path.basename, plugins))
print("Built plugins:", plugins)

pc_files = glob.glob(os.path.join(
    prefix, '**', 'gstreamer-1.0', 'pkgconfig', '*.pc'), recursive=True)
pc_files = list(map(os.path.basename, pc_files))
print("Built .pc files:", pc_files)

success = True

for name in iterate_plugins():
    plugin = "libgst{}.so".format(name)
    pc_file = "gst{}.pc".format(name)

    if plugin not in plugins:
        print(name, ".so missing in", prefix)
        success = False

    if pc_file not in pc_files:
        print(name, ".pc missing in", prefix)
        success = False

if len(glob.glob(os.path.join(prefix, '**', 'bin', 'gst-webrtc-signalling-server'), recursive=True)) != 1:
    print("gst-webrtc-signalling-serverm is missing in", prefix)
    success = False

if not success:
    sys.exit(1)
