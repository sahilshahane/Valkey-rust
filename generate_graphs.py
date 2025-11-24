#!/usr/bin/env python3
"""
Script to generate graphs from results.csv
Generates line graphs showing throughput vs number of clients,
grouped by workload and server core count.
"""

import pandas as pd
import matplotlib.pyplot as plt
import seaborn as sns
from pathlib import Path
import os

# Set style for better-looking graphs
sns.set_style("whitegrid")
plt.rcParams['figure.figsize'] = (12, 7)
plt.rcParams['font.size'] = 11

def load_and_process_data(csv_path):
    """Load CSV and prepare data for graphing."""
    print(f"Loading data from {csv_path}...")
    df = pd.read_csv(csv_path)
    
    # Group by workload, num_clients, server_core_count and calculate averages
    grouped = df.groupby(['workload', 'num_clients', 'server_core_count']).agg({
        'throughput_req_per_sec': 'mean',
        'average_latency_us': 'mean',
        'success_rate_percent': 'mean',
        'overall_cpu_percent': 'mean',
        'overall_server_cpu_percent': 'mean',
        'overall_benchmark_cpu_percent': 'mean',
        'max_ram_mb': 'mean',
        'io_write_speed_bytes_per_sec': 'mean',
        'max_voluntary_ctx_switches': 'mean',
        'max_nonvoluntary_ctx_switches': 'mean',
        'max_ctxt_total': 'mean',
        'max_minor_faults': 'mean'
    }).reset_index()
    
    print(f"Loaded {len(df)} rows, grouped into {len(grouped)} averaged data points")
    print(f"Workloads found: {sorted(grouped['workload'].unique())}")
    print(f"Number of clients: {sorted(grouped['num_clients'].unique())}")
    print(f"Server core counts: {sorted(grouped['server_core_count'].unique())}")
    
    # Filter to only show server_core_count = 1
    grouped = grouped[grouped['server_core_count'] == 1].copy()
    print(f"\nFiltering for server_core_count = 1 only")
    print(f"Filtered data points: {len(grouped)}")
    
    return grouped

def create_throughput_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: throughput_req_per_sec
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        plt.plot(
            core_data['num_clients'],
            core_data['throughput_req_per_sec'],
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('Throughput (requests/sec)', fontsize=13, fontweight='bold')
    plt.title(f'Throughput vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    # Only show legend if there are multiple core counts
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                   title_fontsize=12, 
                   fontsize=11, 
                   loc='best',
                   frameon=True,
                   shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Format y-axis with comma separators
    ax = plt.gca()
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{int(x):,}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / 'throughput.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def create_latency_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: average_latency_us
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        plt.plot(
            core_data['num_clients'],
            core_data['average_latency_us'],
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('Average Latency (μs)', fontsize=13, fontweight='bold')
    plt.title(f'Average Latency vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                title_fontsize=12, 
                fontsize=11, 
                loc='best',
                frameon=True,
                shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Format y-axis with comma separators
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{int(x):,}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / 'latency.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def create_io_write_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: io_write_speed_bytes_per_sec
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        # Convert bytes/sec to KB/sec
        io_write_kb_per_sec = core_data['io_write_speed_bytes_per_sec'] / 1024
        
        plt.plot(
            core_data['num_clients'],
            io_write_kb_per_sec,
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('I/O Write Speed (KB/sec)', fontsize=13, fontweight='bold')
    plt.title(f'I/O Write Speed vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                title_fontsize=12, 
                fontsize=11, 
                loc='best',
                frameon=True,
                shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Format y-axis with 2 decimal places
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{x:.2f}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / f'io_write_{workload}.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def create_voluntary_ctx_switches_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: max_voluntary_ctx_switches
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        plt.plot(
            core_data['num_clients'],
            core_data['max_voluntary_ctx_switches'],
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('Max Voluntary Context Switches', fontsize=13, fontweight='bold')
    plt.title(f'Voluntary Context Switches vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                title_fontsize=12, 
                fontsize=11, 
                loc='best',
                frameon=True,
                shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Format y-axis with comma separators
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{int(x):,}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / f'voluntary_ctx_switches_{workload}.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def create_nonvoluntary_ctx_switches_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: max_nonvoluntary_ctx_switches
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        plt.plot(
            core_data['num_clients'],
            core_data['max_nonvoluntary_ctx_switches'],
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('Max Non-Voluntary Context Switches', fontsize=13, fontweight='bold')
    plt.title(f'Non-Voluntary Context Switches vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                title_fontsize=12, 
                fontsize=11, 
                loc='best',
                frameon=True,
                shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Format y-axis with comma separators
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{int(x):,}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / f'nonvoluntary_ctx_switches_{workload}.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def create_cpu_percent_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: overall_cpu_percent
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        plt.plot(
            core_data['num_clients'],
            core_data['overall_cpu_percent'],
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('Overall CPU Usage (%)', fontsize=13, fontweight='bold')
    plt.title(f'CPU Usage vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                title_fontsize=12, 
                fontsize=11, 
                loc='best',
                frameon=True,
                shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Set y-axis range from 0 to 100
    plt.ylim(0, 100)
    
    # Format y-axis with 2 decimal places
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{x:.2f}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / f'cpu_percent_{workload}.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def create_server_cpu_percent_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: overall_server_cpu_percent
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        plt.plot(
            core_data['num_clients'],
            core_data['overall_server_cpu_percent'],
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('Server CPU Usage (%)', fontsize=13, fontweight='bold')
    plt.title(f'Server CPU Usage vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                title_fontsize=12, 
                fontsize=11, 
                loc='best',
                frameon=True,
                shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Set y-axis range from 0 to 100
    plt.ylim(0, 100)
    
    # Format y-axis with 2 decimal places
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{x:.2f}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / f'server_cpu_percent_{workload}.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def create_benchmark_cpu_percent_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: overall_benchmark_cpu_percent
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        plt.plot(
            core_data['num_clients'],
            core_data['overall_benchmark_cpu_percent'],
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('Benchmark CPU Usage (%)', fontsize=13, fontweight='bold')
    plt.title(f'Benchmark CPU Usage vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                title_fontsize=12, 
                fontsize=11, 
                loc='best',
                frameon=True,
                shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Set y-axis range from 0 to 100
    plt.ylim(0, 100)
    
    # Format y-axis with 2 decimal places
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{x:.2f}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / f'benchmark_cpu_percent_{workload}.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def create_ctxt_total_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: max_ctxt_total
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        plt.plot(
            core_data['num_clients'],
            core_data['max_ctxt_total'],
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('Max Total Context Switches', fontsize=13, fontweight='bold')
    plt.title(f'Total Context Switches vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                title_fontsize=12, 
                fontsize=11, 
                loc='best',
                frameon=True,
                shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Format y-axis with comma separators
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{int(x):,}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / f'ctxt_total_{workload}.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def create_minor_faults_graph(df, workload, output_dir):
    """
    Create line graph with:
    - X axis: num_clients
    - Y axis: max_minor_faults
    - Multiple lines for different server_core_count
    """
    # Filter data for specific workload
    workload_data = df[df['workload'] == workload].copy()
    
    if workload_data.empty:
        print(f"No data found for workload: {workload}")
        return
    
    # Create figure
    plt.figure(figsize=(12, 7))
    
    # Get unique server core counts and sort them
    core_counts = sorted(workload_data['server_core_count'].unique())
    
    # Color palette
    colors = sns.color_palette("husl", len(core_counts))
    
    # Plot line for each server_core_count
    for idx, core_count in enumerate(core_counts):
        core_data = workload_data[workload_data['server_core_count'] == core_count].sort_values('num_clients')
        
        plt.plot(
            core_data['num_clients'],
            core_data['max_minor_faults'],
            marker='o',
            linewidth=2.5,
            markersize=8,
            label=f'{int(core_count)} cores',
            color=colors[idx]
        )
    
    # Customize graph
    plt.xlabel('Number of Clients', fontsize=13, fontweight='bold')
    plt.ylabel('Max Minor Page Faults', fontsize=13, fontweight='bold')
    plt.title(f'Minor Page Faults vs Number of Clients - {workload.upper()}', 
              fontsize=15, fontweight='bold', pad=20)
    if len(core_counts) > 1:
        plt.legend(title='Server Core Count', 
                title_fontsize=12, 
                fontsize=11, 
                loc='best',
                frameon=True,
                shadow=True)
    plt.grid(True, alpha=0.3)
    
    # Set x-axis ticks with bin size of 2
    ax = plt.gca()
    x_min = workload_data['num_clients'].min()
    x_max = workload_data['num_clients'].max()
    plt.xticks(range(int(x_min), int(x_max) + 1, 2))
    
    # Format y-axis with comma separators
    ax.yaxis.set_major_formatter(plt.FuncFormatter(lambda x, p: f'{int(x):,}'))
    
    # Tight layout
    plt.tight_layout()
    
    # Create workload-specific directory
    workload_dir = output_dir / workload
    workload_dir.mkdir(exist_ok=True)
    
    # Save figure
    output_path = workload_dir / f'minor_faults_{workload}.png'
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved: {output_path}")
    
    # Close figure to free memory
    plt.close()

def main():
    """Main function to generate all graphs."""
    # Define paths
    csv_path = Path(__file__).parent / 'results.csv'
    output_dir = Path(__file__).parent / 'graphs'
    
    # Create output directory if it doesn't exist
    output_dir.mkdir(exist_ok=True)
    print(f"Output directory: {output_dir}")
    
    # Check if CSV file exists
    if not csv_path.exists():
        print(f"Error: {csv_path} not found!")
        return
    
    # Load and process data
    df = load_and_process_data(csv_path)
    
    # Get unique workloads
    workloads = sorted(df['workload'].unique())
    
    print(f"\nGenerating graphs for {len(workloads)} workload(s)...")
    
    # Generate all graphs for each workload
    for workload in workloads:
        print(f"\nGenerating graphs for: {workload}")
        print("  - Creating throughput graph...")
        create_throughput_graph(df, workload, output_dir)
        print("  - Creating latency graph...")
        create_latency_graph(df, workload, output_dir)
        print("  - Creating I/O write speed graph...")
        create_io_write_graph(df, workload, output_dir)
        print("  - Creating CPU percentage graph...")
        create_cpu_percent_graph(df, workload, output_dir)
        print("  - Creating server CPU percentage graph...")
        create_server_cpu_percent_graph(df, workload, output_dir)
        print("  - Creating benchmark CPU percentage graph...")
        create_benchmark_cpu_percent_graph(df, workload, output_dir)
        print("  - Creating voluntary context switches graph...")
        create_voluntary_ctx_switches_graph(df, workload, output_dir)
        print("  - Creating non-voluntary context switches graph...")
        create_nonvoluntary_ctx_switches_graph(df, workload, output_dir)
        print("  - Creating total context switches graph...")
        create_ctxt_total_graph(df, workload, output_dir)
        print("  - Creating minor page faults graph...")
        create_minor_faults_graph(df, workload, output_dir)
    
    print(f"\n✓ All graphs generated successfully!")
    print(f"✓ Graphs saved in: {output_dir}")

if __name__ == '__main__':
    main()
