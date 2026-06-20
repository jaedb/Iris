#!/usr/bin/env python3
#
# check-readme-against-plugins-list.py:
#
# Check that the README.md covers all plugins in the plugin list.

import json
import sys

# Read README.md
with open('README.md') as f:
    readme = f.read()

# Read plugins doc cache
with open('docs/plugins/gst_plugins_cache.json', 'r') as f:
    docs = json.load(f)

plugins = list(docs.keys())

missing_plugins = []
covered_plugins = []

# Some plugins are listed with their subdir which doesn't match the plugin name
plugins_remap = {
    'rsflv': 'flavors',
    'textahead': 'ahead',
    'textwrap': 'wrap',
    'textaccumulate': 'accumulate',
}

for plugin_name in plugins:
    if plugin_name in plugins_remap:
        plugin_name = plugins_remap[plugin_name]

    plugin_bullet_line = f'- `{plugin_name}`:'
    if plugin_name.startswith('rs'):
        name2 = plugin_name[2:]
        plugin_bullet_line2 = f'- `{name2}`:'
    else:
        plugin_bullet_line2 = plugin_bullet_line
    if not plugin_bullet_line in readme and not plugin_bullet_line2 in readme:
        print(f'{plugin_name}: Missing!')
        missing_plugins += [plugin_name]
    else:
        print(f'{plugin_name}: Ok')

if missing_plugins:
    sys.exit(
        '\nMissing plugins in README.md:\n - {}'.format('\n - '.join(missing_plugins)))

sys.exit(0)

# FIXME: for later if we want full coverage of all element names too
'''
factories = []

for plugin_name in plugins:
    plugin = docs[plugin_name]
    #print(json.dumps(plugin, indent=4))
    factories += list(plugin['elements'].keys())

print()
print('Elements:', factories)
'''
