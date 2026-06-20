import argparse
import csv
import re
import statistics

import matplotlib
import matplotlib.pyplot as plt

parser = argparse.ArgumentParser()
parser.add_argument("file", help="Input file with pad push timings")
parser.add_argument("--include-filter", help="Regular expression for element:pad names that should be included")
parser.add_argument("--exclude-filter", help="Regular expression for element:pad names that should be excluded")
args = parser.parse_args()

include_filter = None
if args.include_filter is not None:
    include_filter = re.compile(args.include_filter)
exclude_filter = None
if args.exclude_filter is not None:
    exclude_filter = re.compile(args.exclude_filter)

pads = {}

with open(args.file, mode='r', encoding='utf_8', newline='') as csvfile:
    reader = csv.reader(csvfile, delimiter=',', quotechar='|')
    for row in reader:
        if len(row) != 4:
            continue

        if include_filter is not None and not include_filter.match(row[1]):
            continue
        if exclude_filter is not None and exclude_filter.match(row[1]):
            continue

        if not row[1] in pads:
            pads[row[1]] = {
                'push-duration': [],
            }

        push_duration = float(row[3]) / 1000000.0

        wallclock = float(row[0]) / 1000000000.0
        pads[row[1]]['push-duration'].append((wallclock, push_duration))

matplotlib.rcParams['figure.dpi'] = 200

prop_cycle = plt.rcParams['axes.prop_cycle']
colors = prop_cycle.by_key()['color']

fig, ax1 = plt.subplots()

ax1.set_xlabel("wallclock (s)")
ax1.set_ylabel("push duration (ms)")
ax1.tick_params(axis='y')

for (i, (pad, values)) in enumerate(pads.items()):
    # cycle colors
    i = i % len(colors)

    push_durations = [x[1] for x in values['push-duration']]
    ax1.plot(
        [x[0] for x in values['push-duration']],
        push_durations,
        '.', label = pad,
        color = colors[i],
    )

    print("{} push durations: min: {}ms max: {}ms mean: {}ms".format(
        pad,
        min(push_durations),
        max(push_durations),
        statistics.mean(push_durations)))

fig.tight_layout()
plt.legend(loc='best', prop={'size': 6})

plt.show()
