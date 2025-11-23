use std::fs;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs::File;

#[derive(Debug, Serialize, Deserialize)]
struct MetricEntry {
    ts_ms: u64,
    pid: u32,
    io_read_bytes_total: u64,
    io_write_bytes_total: u64,
    rss_kb_total: u64,
    voluntary_ctx_switches_total: u64,
    nonvoluntary_ctx_switches_total: u64,
    minor_faults_total: u64,
    major_faults_total: u64,
    cycles_total: Option<u64>,
    instructions_total: Option<u64>,
    cache_misses_total: u64,
    per_cpu_jiffies: HashMap<String, Vec<u64>>,
    ctxt_total: u64,
}

#[derive(Debug, Default)]
struct BenchmarkResults {
    duration_sec: f64,
    successful_requests: u64,
    failed_requests: u64,
    total_requests: u64,
    throughput_req_per_sec: f64,
    average_latency_us: f64,
    success_rate_percent: f64,
}

#[derive(Debug)]
struct SystemMetrics {
    file_name: String,
    total_io_read_speed: f64,  // bytes per second
    total_io_write_speed: f64, // bytes per second
    max_ram_used: f64,         // MB
    max_minor_faults: u64,
    max_major_faults: u64,
    avg_cpu_utilization: HashMap<String, f64>, // per CPU percentage (arithmetic mean)
    geomean_cpu_utilization: HashMap<String, f64>, // per CPU percentage (geometric mean)
    benchmark_results: BenchmarkResults,
}

#[derive(Debug, Serialize)]
struct CsvRecord {
    file_name: String,
    workload: String,
    num_clients: u32,
    timestamp: String,
    duration_sec: f64,
    successful_requests: u64,
    failed_requests: u64,
    total_requests: u64,
    throughput_req_per_sec: f64,
    average_latency_us: f64,
    success_rate_percent: f64,
    io_read_speed_bytes_per_sec: f64,
    io_write_speed_bytes_per_sec: f64,
    max_ram_mb: f64,
    max_ram_gb: f64,
    max_minor_faults: u64,
    max_major_faults: u64,
    overall_cpu_percent: f64,
    overall_cpu_geomean_percent: f64,
    cpu0_percent: f64,
    cpu0_geomean_percent: f64,
    cpu1_percent: f64,
    cpu1_geomean_percent: f64,
    cpu2_percent: f64,
    cpu2_geomean_percent: f64,
    cpu3_percent: f64,
    cpu3_geomean_percent: f64,
    cpu4_percent: f64,
    cpu4_geomean_percent: f64,
    cpu5_percent: f64,
    cpu5_geomean_percent: f64,
    cpu6_percent: f64,
    cpu6_geomean_percent: f64,
    cpu7_percent: f64,
    cpu7_geomean_percent: f64,
    cpu8_percent: f64,
    cpu8_geomean_percent: f64,
    cpu9_percent: f64,
    cpu9_geomean_percent: f64,
    cpu10_percent: f64,
    cpu10_geomean_percent: f64,
    cpu11_percent: f64,
    cpu11_geomean_percent: f64,
}

fn parse_filename(filename: &str) -> (String, u32, String) {
    // Pattern: metrics-{WORKLOAD}_{NO. OF CLIENTS}_{TIMESTAMP}.json
    // Example: metrics-putall_1_20251123_041843.json
    let name_without_ext = filename.trim_end_matches(".json");
    
    if let Some(stripped) = name_without_ext.strip_prefix("metrics-") {
        let parts: Vec<&str> = stripped.split('_').collect();
        
        if parts.len() >= 3 {
            let workload = parts[0].to_string();
            let num_clients = parts[1].parse::<u32>().unwrap_or(0);
            // Timestamp can have multiple parts (e.g., 20251123_041843), so join from index 2 onwards
            let timestamp = parts[2..].join("_");
            
            return (workload, num_clients, timestamp);
        }
    }
    
    // Fallback if pattern doesn't match
    ("unknown".to_string(), 0, "unknown".to_string())
}

fn parse_benchmark_txt(workload: &str, num_clients: u32, timestamp: &str, directory: &str) -> BenchmarkResults {
    // Construct the benchmark .txt filename from metrics .json filename
    let txt_filename = format!("benchmark_{}_{}_{}.txt", workload, num_clients, timestamp);
    let txt_path = Path::new(directory).join(&txt_filename);
    
    let mut results = BenchmarkResults::default();
    
    if let Ok(content) = fs::read_to_string(&txt_path) {
        for line in content.lines() {
            let line = line.trim();
            
            // Duration: 600.01s
            if line.starts_with("Duration:") {
                if let Some(value) = line.split(':').nth(1) {
                    let value = value.trim().trim_end_matches('s');
                    results.duration_sec = value.parse().unwrap_or(0.0);
                }
            }
            // Successful requests: 28055188
            else if line.starts_with("Successful requests:") {
                if let Some(value) = line.split(':').nth(1) {
                    results.successful_requests = value.trim().parse().unwrap_or(0);
                }
            }
            // Failed requests: 0
            else if line.starts_with("Failed requests:") {
                if let Some(value) = line.split(':').nth(1) {
                    results.failed_requests = value.trim().parse().unwrap_or(0);
                }
            }
            // Total requests: 28055188
            else if line.starts_with("Total requests:") {
                if let Some(value) = line.split(':').nth(1) {
                    results.total_requests = value.trim().parse().unwrap_or(0);
                }
            }
            // Throughput: 46757.68 req/sec
            else if line.starts_with("Throughput:") {
                if let Some(value) = line.split(':').nth(1) {
                    let value = value.trim().split_whitespace().next().unwrap_or("0");
                    results.throughput_req_per_sec = value.parse().unwrap_or(0.0);
                }
            }
            // Average latency: 20.56µs
            else if line.starts_with("Average latency:") {
                if let Some(value) = line.split(':').nth(1) {
                    let value = value.trim().trim_end_matches("µs").trim_end_matches("ms").trim_end_matches("s");
                    results.average_latency_us = value.parse().unwrap_or(0.0);
                }
            }
            // Success rate: 100.00%
            else if line.starts_with("Success rate:") {
                if let Some(value) = line.split(':').nth(1) {
                    let value = value.trim().trim_end_matches('%');
                    results.success_rate_percent = value.parse().unwrap_or(0.0);
                }
            }
        }
    }
    
    results
}

fn calculate_cpu_diff_usage(prev_jiffies: &[u64], curr_jiffies: &[u64]) -> f64 {
    if prev_jiffies.len() < 4 || curr_jiffies.len() < 4 {
        return 0.0;
    }
    
    let prev_user = prev_jiffies[0];
    let prev_nice = prev_jiffies[1];
    let prev_system = prev_jiffies[2];
    let prev_idle = prev_jiffies[3];
    let prev_iowait = if prev_jiffies.len() > 4 { prev_jiffies[4] } else { 0 };
    let prev_irq = if prev_jiffies.len() > 5 { prev_jiffies[5] } else { 0 };
    let prev_softirq = if prev_jiffies.len() > 6 { prev_jiffies[6] } else { 0 };
    let prev_steal = if prev_jiffies.len() > 7 { prev_jiffies[7] } else { 0 };
    
    let curr_user = curr_jiffies[0];
    let curr_nice = curr_jiffies[1];
    let curr_system = curr_jiffies[2];
    let curr_idle = curr_jiffies[3];
    let curr_iowait = if curr_jiffies.len() > 4 { curr_jiffies[4] } else { 0 };
    let curr_irq = if curr_jiffies.len() > 5 { curr_jiffies[5] } else { 0 };
    let curr_softirq = if curr_jiffies.len() > 6 { curr_jiffies[6] } else { 0 };
    let curr_steal = if curr_jiffies.len() > 7 { curr_jiffies[7] } else { 0 };
    
    let prev_total = prev_user + prev_nice + prev_system + prev_idle + prev_iowait + prev_irq + prev_softirq + prev_steal;
    let curr_total = curr_user + curr_nice + curr_system + curr_idle + curr_iowait + curr_irq + curr_softirq + curr_steal;
    
    let prev_idle_total = prev_idle + prev_iowait;
    let curr_idle_total = curr_idle + curr_iowait;
    
    let total_delta = curr_total.saturating_sub(prev_total);
    let idle_delta = curr_idle_total.saturating_sub(prev_idle_total);
    
    if total_delta == 0 {
        return 0.0;
    }
    
    let usage = 100.0 * (1.0 - (idle_delta as f64 / total_delta as f64));
    usage.max(0.0).min(100.0)
}

fn analyze_metrics_file(file_path: &str, file_name: &str, directory: &str) -> Result<SystemMetrics, Box<dyn std::error::Error>> {
    println!("Analyzing: {}", file_path);
    
    let content = fs::read_to_string(file_path)?;
    let mut entries: Vec<MetricEntry> = Vec::new();
    
    // Parse each line as a JSON object
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<MetricEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(e) => eprintln!("Warning: Failed to parse line: {}", e),
        }
    }
    
    if entries.is_empty() {
        return Err("No valid entries found in file".into());
    }
    
    println!("  Loaded {} metric entries", entries.len());
    
    // Calculate metrics
    let mut max_ram = 0.0f64;
    let mut max_minor_faults = 0u64;
    let mut max_major_faults = 0u64;
    let mut cpu_usage_sum: HashMap<String, f64> = HashMap::new();
    let mut cpu_sample_count: HashMap<String, u64> = HashMap::new();
    
    // Find max RAM and faults
    for entry in &entries {
        let ram_mb = entry.rss_kb_total as f64 / 1024.0;
        if ram_mb > max_ram {
            max_ram = ram_mb;
        }
        
        if entry.minor_faults_total > max_minor_faults {
            max_minor_faults = entry.minor_faults_total;
        }
        
        if entry.major_faults_total > max_major_faults {
            max_major_faults = entry.major_faults_total;
        }
    }
    
    // Calculate average and geometric mean CPU usage from jiffies differences
    // For geometric mean, we'll accumulate log sum instead of product to avoid overflow
    let mut cpu_log_sum: HashMap<String, f64> = HashMap::new();
    
    for i in 1..entries.len() {
        let prev_entry = &entries[i - 1];
        let curr_entry = &entries[i];
        
        for (cpu_name, curr_jiffies) in &curr_entry.per_cpu_jiffies {
            if let Some(prev_jiffies) = prev_entry.per_cpu_jiffies.get(cpu_name) {
                let usage = calculate_cpu_diff_usage(prev_jiffies, curr_jiffies);
                
                // For arithmetic mean
                *cpu_usage_sum.entry(cpu_name.clone()).or_insert(0.0) += usage;
                
                // For geometric mean: accumulate log(x) to avoid overflow
                // geometric mean = exp(mean(log(x)))
                // Add small epsilon to avoid log(0)
                let log_value = (usage + 0.01).max(0.01).ln();
                *cpu_log_sum.entry(cpu_name.clone()).or_insert(0.0) += log_value;
                
                *cpu_sample_count.entry(cpu_name.clone()).or_insert(0) += 1;
            }
        }
    }
    
    // Calculate arithmetic mean
    let avg_cpu_utilization: HashMap<String, f64> = cpu_usage_sum
        .into_iter()
        .map(|(cpu, sum)| {
            let count = cpu_sample_count.get(&cpu).unwrap_or(&1);
            (cpu, sum / *count as f64)
        })
        .collect();
    
    // Calculate geometric mean: exp(mean(log(x))) - epsilon
    let geomean_cpu_utilization: HashMap<String, f64> = cpu_log_sum
        .into_iter()
        .map(|(cpu, log_sum)| {
            let count = cpu_sample_count.get(&cpu).unwrap_or(&1);
            let n = *count as f64;
            let mean_log = log_sum / n;
            let geomean = mean_log.exp() - 0.01; // Subtract epsilon we added
            (cpu, geomean.max(0.0).min(100.0))
        })
        .collect();
    
    // Calculate IO speeds
    let first_entry = &entries[0];
    let last_entry = &entries[entries.len() - 1];
    
    let time_diff_ms = last_entry.ts_ms.saturating_sub(first_entry.ts_ms);
    let time_diff_sec = time_diff_ms as f64 / 1000.0;
    
    let read_bytes_diff = last_entry.io_read_bytes_total.saturating_sub(first_entry.io_read_bytes_total);
    let write_bytes_diff = last_entry.io_write_bytes_total.saturating_sub(first_entry.io_write_bytes_total);
    
    let total_io_read_speed = if time_diff_sec > 0.0 {
        read_bytes_diff as f64 / time_diff_sec
    } else {
        0.0
    };
    
    let total_io_write_speed = if time_diff_sec > 0.0 {
        write_bytes_diff as f64 / time_diff_sec
    } else {
        0.0
    };
    
    // Parse benchmark results from corresponding .txt file
    let (workload, num_clients, timestamp) = parse_filename(file_name);
    let benchmark_results = parse_benchmark_txt(&workload, num_clients, &timestamp, directory);
    
    Ok(SystemMetrics {
        file_name: file_name.to_string(),
        total_io_read_speed,
        total_io_write_speed,
        max_ram_used: max_ram,
        max_minor_faults,
        max_major_faults,
        avg_cpu_utilization,
        geomean_cpu_utilization,
        benchmark_results,
    })
}

fn format_bytes(bytes: f64) -> String {
    const UNITS: &[&str] = &["B/s", "KB/s", "MB/s", "GB/s"];
    let mut value = bytes;
    let mut unit_index = 0;
    
    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }
    
    format!("{:.2} {}", value, UNITS[unit_index])
}

fn analyze_all_metrics(directory: &str, output_csv: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let entries = fs::read_dir(directory)?;
    let mut json_files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "json")
                .unwrap_or(false)
        })
        .collect();
    
    json_files.sort_by_key(|e| e.path());
    
    if json_files.is_empty() {
        println!("No JSON files found in {}", directory);
        return Ok(());
    }
    
    println!("=== Analyzing Benchmark Metrics ===\n");
    println!("Found {} benchmark log files\n", json_files.len());
    
    let mut all_metrics = Vec::new();
    
    for entry in json_files {
        let path = entry.path();
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        
        match analyze_metrics_file(&path.to_string_lossy(), &file_name, directory) {
            Ok(metrics) => {
                println!("\n📄 File: {}", file_name);
                println!("   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("   📊 IO Performance:");
                println!("      Read Speed:  {}", format_bytes(metrics.total_io_read_speed));
                println!("      Write Speed: {}", format_bytes(metrics.total_io_write_speed));
                println!();
                println!("   💾 Memory Usage:");
                println!("      Max RAM: {:.2} MB ({:.2} GB)", 
                         metrics.max_ram_used, 
                         metrics.max_ram_used / 1024.0);
                println!();
                println!("   🖥️  CPU Utilization:");
                
                // Sort CPU names
                let mut cpu_names: Vec<_> = metrics.avg_cpu_utilization.keys().collect();
                cpu_names.sort();
                
                // Print overall CPU first if available
                if let Some(usage) = metrics.avg_cpu_utilization.get("cpu") {
                    println!("      Overall: {:.2}%", usage);
                }
                
                // Print individual CPUs
                for cpu_name in &cpu_names {
                    if *cpu_name != "cpu" {
                        if let Some(usage) = metrics.avg_cpu_utilization.get(*cpu_name) {
                            println!("      {}: {:.2}%", cpu_name.to_uppercase(), usage);
                        }
                    }
                }
                
                all_metrics.push(metrics);
            }
            Err(e) => {
                println!("\n❌ Error analyzing {}: {}", file_name, e);
            }
        }
    }
    
    // Write to CSV if requested
    if let Some(csv_path) = output_csv {
        write_csv(&all_metrics, csv_path)?;
        println!("\n✅ CSV output written to: {}", csv_path);
    }
    
    Ok(())
}

fn write_csv(metrics_list: &[SystemMetrics], output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(output_path)?;
    let mut wtr = csv::Writer::from_writer(file);
    
    for metrics in metrics_list {
        let (workload, num_clients, timestamp) = parse_filename(&metrics.file_name);
        
        let record = CsvRecord {
            file_name: metrics.file_name.clone(),
            workload,
            num_clients,
            timestamp,
            duration_sec: metrics.benchmark_results.duration_sec,
            successful_requests: metrics.benchmark_results.successful_requests,
            failed_requests: metrics.benchmark_results.failed_requests,
            total_requests: metrics.benchmark_results.total_requests,
            throughput_req_per_sec: metrics.benchmark_results.throughput_req_per_sec,
            average_latency_us: metrics.benchmark_results.average_latency_us,
            success_rate_percent: metrics.benchmark_results.success_rate_percent,
            io_read_speed_bytes_per_sec: metrics.total_io_read_speed,
            io_write_speed_bytes_per_sec: metrics.total_io_write_speed,
            max_ram_mb: metrics.max_ram_used,
            max_ram_gb: metrics.max_ram_used / 1024.0,
            max_minor_faults: metrics.max_minor_faults,
            max_major_faults: metrics.max_major_faults,
            overall_cpu_percent: *metrics.avg_cpu_utilization.get("cpu").unwrap_or(&0.0),
            overall_cpu_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu").unwrap_or(&0.0),
            cpu0_percent: *metrics.avg_cpu_utilization.get("cpu0").unwrap_or(&0.0),
            cpu0_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu0").unwrap_or(&0.0),
            cpu1_percent: *metrics.avg_cpu_utilization.get("cpu1").unwrap_or(&0.0),
            cpu1_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu1").unwrap_or(&0.0),
            cpu2_percent: *metrics.avg_cpu_utilization.get("cpu2").unwrap_or(&0.0),
            cpu2_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu2").unwrap_or(&0.0),
            cpu3_percent: *metrics.avg_cpu_utilization.get("cpu3").unwrap_or(&0.0),
            cpu3_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu3").unwrap_or(&0.0),
            cpu4_percent: *metrics.avg_cpu_utilization.get("cpu4").unwrap_or(&0.0),
            cpu4_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu4").unwrap_or(&0.0),
            cpu5_percent: *metrics.avg_cpu_utilization.get("cpu5").unwrap_or(&0.0),
            cpu5_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu5").unwrap_or(&0.0),
            cpu6_percent: *metrics.avg_cpu_utilization.get("cpu6").unwrap_or(&0.0),
            cpu6_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu6").unwrap_or(&0.0),
            cpu7_percent: *metrics.avg_cpu_utilization.get("cpu7").unwrap_or(&0.0),
            cpu7_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu7").unwrap_or(&0.0),
            cpu8_percent: *metrics.avg_cpu_utilization.get("cpu8").unwrap_or(&0.0),
            cpu8_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu8").unwrap_or(&0.0),
            cpu9_percent: *metrics.avg_cpu_utilization.get("cpu9").unwrap_or(&0.0),
            cpu9_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu9").unwrap_or(&0.0),
            cpu10_percent: *metrics.avg_cpu_utilization.get("cpu10").unwrap_or(&0.0),
            cpu10_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu10").unwrap_or(&0.0),
            cpu11_percent: *metrics.avg_cpu_utilization.get("cpu11").unwrap_or(&0.0),
            cpu11_geomean_percent: *metrics.geomean_cpu_utilization.get("cpu11").unwrap_or(&0.0),
        };
        
        wtr.serialize(record)?;
    }
    
    wtr.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 {
        let path = &args[1];
        let csv_output = if args.len() > 2 {
            Some(args[2].as_str())
        } else {
            None
        };
        
        if Path::new(path).is_dir() {
            // Analyze all JSON files in directory
            let default_csv = "benchmark_metrics.csv";
            let csv_path = csv_output.or(Some(default_csv));
            analyze_all_metrics(path, csv_path)?;
        } else {
            // Analyze single file
            let file_name = Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let directory = Path::new(path)
                .parent()
                .unwrap_or(Path::new("."))
                .to_string_lossy()
                .to_string();
            let metrics = analyze_metrics_file(path, &file_name, &directory)?;
            
            println!("\n=== System Performance Metrics ===\n");
            println!("📊 IO Performance:");
            println!("  Average Read Speed:  {}", format_bytes(metrics.total_io_read_speed));
            println!("  Average Write Speed: {}", format_bytes(metrics.total_io_write_speed));
            println!();
            
            println!("💾 Memory Usage:");
            println!("  Maximum RAM Used: {:.2} MB ({:.2} GB)", 
                     metrics.max_ram_used, 
                     metrics.max_ram_used / 1024.0);
            println!();
            
            println!("🖥️  CPU Utilization:");
            let mut cpu_names: Vec<_> = metrics.avg_cpu_utilization.keys().collect();
            cpu_names.sort();
            
            for cpu_name in cpu_names {
                if let Some(usage) = metrics.avg_cpu_utilization.get(cpu_name) {
                    if cpu_name == "cpu" {
                        println!("  Overall CPU: {:.2}%", usage);
                    } else {
                        println!("  {}: {:.2}%", cpu_name.to_uppercase(), usage);
                    }
                }
            }
            
            // Write single file to CSV if requested
            if let Some(csv_path) = csv_output {
                write_csv(&[metrics], csv_path)?;
                println!("\n✅ CSV output written to: {}", csv_path);
            }
        }
    } else {
        println!("Usage:");
        println!("  {} <json_file> [output.csv]     - Analyze a single metrics file", args[0]);
        println!("  {} <directory> [output.csv]     - Analyze all JSON files in directory", args[0]);
        println!("\nExamples:");
        println!("  {} benchmark_logs/", args[0]);
        println!("  {} benchmark_logs/ results.csv", args[0]);
        println!("\nNote: CSV output is generated by default as 'benchmark_metrics.csv' when analyzing a directory");
    }
    
    Ok(())
}
