# Benchmark Metrics Analyzer

A Rust tool to analyze system performance metrics from benchmark logs.

## Features

This tool analyzes JSON timeseries data and provides:

1. **Total Average IO Read & Write Speed per Second**
   - Calculates bytes per second for disk I/O operations
   - Automatically formats output (B/s, KB/s, MB/s, GB/s)

2. **Maximum RAM Used**
   - Tracks peak memory usage across the benchmark run
   - Reports in both MB and GB

3. **Average CPU Utilization per CPU**
   - Calculates CPU usage for each individual core
   - Provides overall CPU utilization percentage
   - Uses CPU jiffies differences for accurate measurements

## Usage

### Analyze a single metrics file:
```bash
cd metrics_analyzer
cargo run --release -- ../benchmark_logs/metrics-getall_100_20251123_041843.json
```

### Analyze all JSON files in a directory:
```bash
cd metrics_analyzer
cargo run --release -- ../benchmark_logs/
```

### Generate CSV output:
```bash
# Default output: benchmark_metrics.csv
cd metrics_analyzer
cargo run --release -- ../benchmark_logs/

# Custom output filename:
cargo run --release -- ../benchmark_logs/ my_results.csv

# Single file to CSV:
cargo run --release -- ../benchmark_logs/metrics-getall_100.json output.csv
```

## How It Works

The tool:
1. Parses JSON timeseries data from your benchmark logs
2. Each entry contains:
   - Timestamp (ts_ms)
   - Process ID (pid)
   - IO statistics (io_read_bytes_total, io_write_bytes_total)
   - Memory usage (rss_kb_total)
   - CPU jiffies per core (per_cpu_jiffies)
   - Context switches and page faults

3. Calculates metrics:
   - **IO Speed**: (final_bytes - initial_bytes) / time_elapsed
   - **Max RAM**: Maximum rss_kb_total across all samples
   - **CPU Usage**: Calculated from CPU jiffy differences between samples
     - Formula: 100 * (1 - idle_delta / total_delta)
     - Arithmetic Mean: Simple average of all CPU utilization samples
     - Geometric Mean: exp(mean(log(x))) - better represents sustained performance
       - Less sensitive to outliers and spikes
       - More representative of "typical" CPU usage throughout the run
       - For consistent workloads, arithmetic and geometric means are very close
       - For sporadic workloads (e.g., 1 client), geometric mean is much lower, showing many idle periods
       - Large difference between arithmetic and geometric means indicates inconsistent CPU usage

## Output Example

### Console Output:
```
📄 File: metrics-putall_1000_20251123_093323.json
   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   📊 IO Performance:
      Read Speed:  285.62 B/s
      Write Speed: 9.48 MB/s

   💾 Memory Usage:
      Max RAM: 44.92 MB (0.04 GB)

   🖥️  CPU Utilization:
      Overall: 90.57%
      CPU0: 99.93%
      CPU1: 99.95%
      CPU10: 80.45%
      CPU11: 80.38%
      ...
```

### CSV Output:
The CSV file contains the following columns:
- `file_name` - Source JSON filename
- `workload` - Workload type (getall, putall, etc.) - extracted from filename
- `num_clients` - Number of concurrent clients - extracted from filename
- `timestamp` - Benchmark timestamp - extracted from filename
- `io_read_speed_bytes_per_sec` - Read speed in bytes/second
- `io_write_speed_bytes_per_sec` - Write speed in bytes/second
- `max_ram_mb` - Maximum RAM in MB
- `max_ram_gb` - Maximum RAM in GB
- `max_minor_faults` - Maximum minor page faults (soft faults)
- `max_major_faults` - Maximum major page faults (hard faults requiring disk I/O)
- `overall_cpu_percent` - Overall CPU utilization (arithmetic mean)
- `overall_cpu_geomean_percent` - Overall CPU utilization (geometric mean)
- `cpu0_percent` through `cpu11_percent` - Individual CPU core utilization (arithmetic mean)
- `cpu0_geomean_percent` through `cpu11_geomean_percent` - Individual CPU core utilization (geometric mean)

Example CSV row (36 columns total):
```csv
file_name,workload,num_clients,timestamp,io_read_speed_bytes_per_sec,io_write_speed_bytes_per_sec,...
metrics-putall_1000_20251123_093323.json,putall,1000,20251123,285.62,9934438.4,44.92,0.04,90.57,...
```

The CSV now includes 36 columns: basic metrics (10) + overall CPU arithmetic & geometric means (2) + 12 CPU cores × 2 metrics each (24).

The filename is parsed using the pattern: `metrics-{WORKLOAD}_{NUM_CLIENTS}_{TIMESTAMP}.json`

## CSV Analysis Examples

With the parsed `workload`, `num_clients`, and `timestamp` columns, you can easily analyze the data:

### Compare workloads at specific client count:
```bash
# Compare getall vs putall at 100 clients
awk -F',' '$3==100 {printf "%-10s | Write: %8.2f MB/s | CPU: %5.2f%%\n", $2, $6/1024/1024, $9}' benchmark_results.csv
```

### Memory scaling with client count:
```bash
# Show how RAM usage scales for getall workload
awk -F',' '$2=="getall" {printf "Clients: %4d | RAM: %6.2f MB\n", $3, $7}' benchmark_results.csv | sort -t: -k2 -n
```

### Find performance bottlenecks:
```bash
# Find benchmarks with highest CPU usage
tail -n +2 benchmark_results.csv | sort -t',' -k9 -rn | head -5 | cut -d',' -f1-4,9
```

### Import into spreadsheet tools:
The CSV can be directly imported into:
- Excel
- Google Sheets
- Python pandas: `df = pd.read_csv('benchmark_results.csv')`
- R: `data <- read.csv('benchmark_results.csv')`

## Dependencies

- `serde` - JSON deserialization
- `serde_json` - JSON parsing
- `csv` - CSV output generation

## Project Structure

```
metrics_analyzer/
├── Cargo.toml          # Project dependencies
└── src/
    └── main.rs         # Main analyzer implementation
```

## Key Insights from Your Benchmarks

From the analysis results:

- **GET operations**: Low write activity (0 B/s), RAM usage scales from 16-41 MB
- **PUT operations**: High write activity (5-11 MB/s), slightly higher RAM usage
- **CPU utilization**: Generally high (85-92%) for concurrent operations
- **Performance cores** (CPU0-5): Consistently high usage (~97-100%)
- **Efficiency cores** (CPU6-11): Lower usage (~74-84%)

This indicates your KVStore is CPU-bound during high concurrency operations!
