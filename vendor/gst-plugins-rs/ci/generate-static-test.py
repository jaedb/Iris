#!/usr/bin/env python3
# Generate a meson project statically linking on all plugins

import sys
import os

from utils import iterate_plugins

# the csound version used on ci does not ship a .pc file
# threadshare we skip in meson static build as well
IGNORE = ['csound', 'threadshare', 'gtk4']

outdir = sys.argv[1]

plugins = list(filter(lambda p: p not in IGNORE, iterate_plugins()))
deps = list(
    map(lambda p: "    dependency('gst{}', static: true)".format(p), plugins))
deps = ',\n'.join(deps)

meson = """
project('test-gst-plugins-rs-static', 'c')

gst_deps = [
    dependency('gstreamer-1.0'),
%s
]

executable('test-gst-static', ['main.c'],
  dependencies: gst_deps,
)
""" % (deps)

declare = list(
    map(lambda p: "GST_PLUGIN_STATIC_DECLARE({});".format(p), plugins))
declare = '\n'.join(declare)

register = list(
    map(lambda p: "\tGST_PLUGIN_STATIC_REGISTER({});".format(p), plugins))
register = '\n'.join(register)

check = list(
    map(lambda p: "\tg_assert (gst_registry_find_plugin(registry, \"{}\"));".format(p), plugins))
check = '\n'.join(check)

main = """
#include <gst/gst.h>

%s

int main(int argc, char **argv)
{
	g_autoptr(GstRegistry) registry = NULL;

	gst_init(&argc, &argv);

%s

	registry = gst_registry_get();

%s

	return 0;
}
""" % (declare, register, check)

os.makedirs(outdir)

meson_file = open(os.path.join(outdir, 'meson.build'), 'w')
meson_file.write(meson)

main_file = open(os.path.join(outdir, 'main.c'), 'w')
main_file.write(main)
