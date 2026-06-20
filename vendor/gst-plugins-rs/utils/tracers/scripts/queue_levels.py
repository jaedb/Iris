import argparse
import csv
import re

import matplotlib
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches

parser = argparse.ArgumentParser()
parser.add_argument("file", help="Input file with queue levels")
parser.add_argument("--include-filter", help="Regular expression for queue names that should be included")
parser.add_argument("--exclude-filter", help="Regular expression for queue names that should be excluded")
parser.add_argument("--bytes", help="include bytes levels", action="store_true")
parser.add_argument("--time", help="include time levels (default if none of the others are enabled)", action="store_true")
parser.add_argument("--buffers", help="include buffers levels", action="store_true")
parser.add_argument("--no-max", help="do not include max levels (enabled by default)", action="store_true")
args = parser.parse_args()

include_filter = None
if args.include_filter is not None:
    include_filter = re.compile(args.include_filter)
exclude_filter = None
if args.exclude_filter is not None:
    exclude_filter = re.compile(args.exclude_filter)

queues = {}

with open(args.file, mode='r', encoding='utf_8', newline='') as csvfile:
    reader = csv.reader(csvfile, delimiter=',', quotechar='|')
    for row in reader:
        if len(row) != 9:
            continue

        if include_filter is not None and not include_filter.match(row[1]):
            continue
        if exclude_filter is not None and exclude_filter.match(row[1]):
            continue

        if not row[1] in queues:
            queues[row[1]] = {
                'cur-level-bytes': [],
                'cur-level-time': [],
                'cur-level-buffers': [],
                'max-size-bytes': [],
                'max-size-time': [],
                'max-size-buffers': [],
            }

        wallclock = float(row[0]) / 1000000000.0
        queues[row[1]]['cur-level-bytes'].append((wallclock, int(row[3])))
        queues[row[1]]['cur-level-time'].append((wallclock, float(row[4]) / 1000000000.0))
        queues[row[1]]['cur-level-buffers'].append((wallclock, int(row[5])))
        queues[row[1]]['max-size-bytes'].append((wallclock, int(row[6])))
        queues[row[1]]['max-size-time'].append((wallclock, float(row[7]) / 1000000000.0))
        queues[row[1]]['max-size-buffers'].append((wallclock, int(row[8])))

matplotlib.rcParams['figure.dpi'] = 200

prop_cycle = plt.rcParams['axes.prop_cycle']
colors = prop_cycle.by_key()['color']

num_plots = 0
axes_names = []
if args.buffers:
    num_plots += 1
    axes_names.append("buffers")
if args.time:
    num_plots += 1
    axes_names.append("time (s)")
if args.bytes:
    num_plots += 1
    axes_names.append("bytes")

if num_plots == 0:
    num_plots += 1
    axes_names.append("time (s)")

fig, axes = plt.subplots(num_plots, sharex=True)
if num_plots == 1:
    axes = [axes]

axes[0].set_xlabel("wallclock (s)")
axes[0].set_ylabel(axes_names[0])
axes[0].tick_params(axis='y')

if num_plots > 1:
    axes[1].set_ylabel(axes_names[1])
if num_plots > 2:
    axes[2].set_ylabel(axes_names[2])

patches = []

for (i, (queue, values)) in enumerate(queues.items()):
    # cycle colors
    i = i % len(colors)
    axis = 0

    if args.buffers:
        axes[axis].plot(
            [x[0] for x in values['cur-level-buffers']],
            [x[1] for x in values['cur-level-buffers']],
            '.', label = '{}: cur-level-buffers'.format(queue),
            color = colors[i],
        )

        if not args.no_max:
            axes[axis].plot(
                [x[0] for x in values['max-size-buffers']],
                [x[1] for x in values['max-size-buffers']],
                '-', label = '{}: max-size-buffers'.format(queue),
                color = colors[i],
            )

        axis += 1

    if args.time:
        axes[axis].plot(
            [x[0] for x in values['cur-level-time']],
            [x[1] for x in values['cur-level-time']],
            '.', label = '{}: cur-level-time'.format(queue),
            color = colors[i],
        )

        if not args.no_max:
            axes[axis].plot(
                [x[0] for x in values['max-size-time']],
                [x[1] for x in values['max-size-time']],
                '-', label = '{}: max-size-time'.format(queue),
                color = colors[i],
            )

        axis += 1

    if args.bytes:
        axes[axis].plot(
            [x[0] for x in values['cur-level-bytes']],
            [x[1] for x in values['cur-level-bytes']],
            '.', label = '{}: cur-level-bytes'.format(queue),
            color = colors[i],
        )

        if not args.no_max:
            axes[axis].plot(
                [x[0] for x in values['max-size-bytes']],
                [x[1] for x in values['max-size-bytes']],
                '-', label = '{}: max-size-bytes'.format(queue),
                color = colors[i],
            )

        axis += 1

    patches.append(mpatches.Patch(color=colors[i], label=queue))

fig.tight_layout()

plt.legend(handles=patches, loc='best', prop={'size': 6})
plt.show()
