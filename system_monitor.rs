use std::fs;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use std::path::Path;

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

#[derive(Debug)]
struct SystemMetrics {
    total_io_read_speed: f64,  // bytes per second
    total_io_write_speed: f64, // bytes per second
    max_ram_used: f64,         // MB
    avg_cpu_utilization: HashMap<String, f64>, // per CPU percentage
}

fn calculate_cpu_usage(jiffies: &[u64]) -> f64 {
    // jiffies format: [user, nice, system, idle, iowait, irq, softirq, steal, guest, guest_nice]
    if jiffies.len() < 4 {
        return 0.0;
    }
    
    let user = jiffies[0];
    let nice = jiffies[1];
    let system = jiffies[2];
    let idle = jiffies[3];
    let iowait = if jiffies.len() > 4 { jiffies[4] } else { 0 };
    let irq = if jiffies.len() > 5 { jiffies[5] } else { 0 };
    let softirq = if jiffies.len() > 6 { jiffies[6] } else { 0 };
    let steal = if jiffies.len() > 7 { jiffies[7] } else { 0 };
    
    let total = user + nice + system + idle + iowait + irq + softirq + steal;
    let idle_total = idle + iowait;
    
    if total == 0 {
        return 0.0;
    }
    
    let usage = 100.0 * (1.0 - (idle_total as f64 / total as f64));
    usage.max(0.0).min(100.0)
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

fn analyze_metrics_file(file_path: &str) -> Result<SystemMetrics, Box<dyn std::error::Error>> {
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
    let mut cpu_usage_sum: HashMap<String, f64> = HashMap::new();
    let mut cpu_sample_count: HashMap<String, u64> = HashMap::new();
    
    // Find max RAM
    for entry in &entries {
        let ram_mb = entry.rss_kb_total as f64 / 1024.0;
        if ram_mb > max_ram {
            max_ram = ram_mb;
        }
    }
    
    // Calculate average CPU usage from jiffies differences
    for i in 1..entries.len() {
        let prev_entry = &entries[i - 1];
        let curr_entry = &entries[i];
        
        for (cpu_name, curr_jiffies) in &curr_entry.per_cpu_jiffies {
            if let Some(prev_jiffies) = prev_entry.per_cpu_jiffies.get(cpu_name) {
                let usage = calculate_cpu_diff_usage(prev_jiffies, curr_jiffies);
                *cpu_usage_sum.entry(cpu_name.clone()).or_insert(0.0) += usage;
                *cpu_sample_count.entry(cpu_name.clone()).or_insert(0) += 1;
            }
        }
    }
    
    let avg_cpu_utilization: HashMap<String, f64> = cpu_usage_sum
        .into_iter()
        .map(|(cpu, sum)| {
            let count = cpu_sample_count.get(&cpu).unwrap_or(&1);
            (cpu, sum / *count as f64)
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
    
    Ok(SystemMetrics {
        total_io_read_speed,
        total_io_write_speed,
        max_ram_used: max_ram,
        avg_cpu_utilization,
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

fn analyze_all_metrics(directory: &str) -> Result<(), Box<dyn std::error::Error>> {
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
    
    for entry in json_files {
        let path = entry.path();
        let file_name = path.file_name().unwrap().to_string_lossy();
        
        match analyze_metrics_file(&path.to_string_lossy()) {
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
            }
            Err(e) => {
                println!("\n❌ Error analyzing {}: {}", file_name, e);
            }
        }
    }
    
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() > 1 {
        let path = &args[1];
        
        if Path::new(path).is_dir() {
            // Analyze all JSON files in directory
            analyze_all_metrics(path)?;
        } else {
            // Analyze single file
            let metrics = analyze_metrics_file(path)?;
            
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
        }
    } else {
        println!("Usage:");
        println!("  {} <json_file>     - Analyze a single metrics file", args[0]);
        println!("  {} <directory>     - Analyze all JSON files in directory", args[0]);
        println!("\nExample:");
        println!("  {} benchmark_logs/", args[0]);
    }
    
    Ok(())
}
