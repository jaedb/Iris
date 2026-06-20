import sys
import numpy as np
import pandas as pd
import plotly.graph_objects as go
from collections import defaultdict


# Read the CSV file
def parse_memory_data(csv_data):
    # Read only required columns and use dtype specifications for better performance
    df = pd.read_csv(
        csv_data,
        names=["timestamp", "operation", "pointer", "parent", "type", "size"],
        dtype={
            "timestamp": np.int64,
            "operation": str,
            "pointer": str,
            "parent": str,
            "type": str,
            "size": np.float64,
        },
        usecols=["timestamp", "operation", "pointer", "parent", "type", "size"],
    )

    # Filter out entries with parents early
    df = df[df["parent"].apply(lambda x: int(x, 16) == 0)]

    # Convert size to MB once
    df["size"] = df["size"] / (1024 * 1024)

    # Sort by timestamp
    df = df.sort_values("timestamp")

    # Initialize data structures
    memory_by_type = defaultdict(lambda: defaultdict(float))
    memory_values_by_type = defaultdict(list)

    # Process in chunks for better memory usage
    chunk_size = 10000
    for chunk_start in range(0, len(df), chunk_size):
        chunk = df.iloc[chunk_start : chunk_start + chunk_size]

        for _, row in chunk.iterrows():
            mem_type = row["type"]
            pointer = row["pointer"]
            timestamp = row["timestamp"] / 1000000000.0  # Convert to seconds

            # Update memory tracking
            if row["operation"] == "alloc":
                memory_by_type[mem_type][pointer] = row["size"]
            elif pointer in memory_by_type[mem_type]:
                del memory_by_type[mem_type][pointer]

            # Calculate metrics for this timestamp
            current_memory = sum(memory_by_type[mem_type].values())
            buffer_count = len(memory_by_type[mem_type])

            extra_info = ""
            if row["operation"] == "alloc" or row["operation"] == "free":
                extra_info = f"{row['operation']}(0x{pointer}={row['size']:.3f}MB)"

            memory_values_by_type[mem_type].append(
                (timestamp, current_memory, buffer_count, extra_info)
            )
    return memory_values_by_type


def create_memory_graph(csv_data):
    memory_values_by_type = parse_memory_data(csv_data)

    fig = go.Figure()

    colors = {
        "SystemMemory": "blue",
        "GPU Memory": "red",
        "GLBuffer": "orange",
        "GLMemoryPBO": "darkorange",
        "GLRenderBuffer": "darkturquoise",
        "gst.cuda.memory": "green",
        "DMABuf": "purple",
        "fd": "magenta",
        "shm": "darkviolet",
    }

    # Add traces for each memory type
    for mem_type, values in memory_values_by_type.items():
        color = colors.get(mem_type, "gray")  # Default to gray for unknown types

        decimation_factor = max(1, len(values) // 10000)

        # Decimate the data points
        decimated_values = values[::decimation_factor]
        decimated_timestamps = [v[0] for v in decimated_values]
        memory_values = [v[1] for v in decimated_values]  # Extract memory values
        buffer_counts = [v[2] for v in decimated_values]  # Extract buffer counts
        extra_info = [v[3] for v in decimated_values]  # Extract extra info
        extra_info = [v[2] for v in decimated_values]  # Extract buffer counts

        # Optimize hover text - only create it for visible points
        hover_text = [
            f"{mem_type}:<br>• Size: {mem:.2f}MB<br>• Buffers: {count}"
            for mem, count, _ in zip(memory_values, buffer_counts, extra_info)
        ]

        fig.add_trace(
            go.Scatter(
                x=decimated_timestamps,
                y=memory_values,
                mode="lines",  # Removed markers for better performance
                name=f"{mem_type}",
                line=dict(color=color, width=2),
                hovertext=hover_text,
                hoverinfo="text",
                hovertemplate="%{hovertext}<extra></extra>",  # Optimized hover template
            )
        )

    # Customize the layout
    fig.update_layout(
        title="GStreamer Memory Usage Over Time",
        xaxis_title="Timestamp",
        yaxis_title="Memory Usage (MB)",
        hovermode="x unified",
        showlegend=True,
        template="plotly_white",
        yaxis=dict(rangemode="nonnegative"),  # Ensure y-axis doesn't go below 0
    )

    return fig


if __name__ == "__main__":
    csv_data = sys.argv[1]
    fig = create_memory_graph(csv_data)
    fig.show()
