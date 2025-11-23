use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Metrics {
    ts_ms: u64,
    io_read_bytes_total: u64,
    io_write_bytes_total: u64,
    rss_kb_total: u64,
    per_cpu_jiffies: HashMap<String, Vec<u64>>,
}

#[derive(Debug, Serialize)]
struct Summary {
    operation: String,
    thread_count: u32,
    avg_io_read_speed_bytes_per_sec: f64,
    avg_io_write_speed_bytes_per_sec: f64,
    max_ram_kb: u64,
    avg_cpu_utilization_per_cpu: HashMap<String, f64>,
    total_duration_ms: u64,
}

fn calculate_cpu_utilization(jiffies_start: &[u64], jiffies_end: &[u64]) -> f64 {
    // CPU jiffies: [user, nice, system, idle, iowait, irq, softirq, steal, guest, guest_nice]
    if jiffies_start.len() < 4 || jiffies_end.len() < 4 {
        return 0.0;
    }

    let total_start: u64 = jiffies_start.iter().sum();
    let total_end: u64 = jiffies_end.iter().sum();
    let total_delta = total_end.saturating_sub(total_start);

    if total_delta == 0 {
        return 0.0;
    }

    // Idle + iowait
    let idle_start = jiffies_start[3] + jiffies_start.get(4).unwrap_or(&0);
    let idle_end = jiffies_end[3] + jiffies_end.get(4).unwrap_or(&0);
    let idle_delta = idle_end.saturating_sub(idle_start);

    let active_delta = total_delta.saturating_sub(idle_delta);
    
    (active_delta as f64 / total_delta as f64) * 100.0
}

fn analyze_metrics_file(file_path: &str) -> Result<Summary, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(file_path)?;
    let lines: Vec<&str> = content.lines().collect();
    
    if lines.is_empty() {
        return Err("Empty file".into());
    }

    let mut metrics_records: Vec<Metrics> = Vec::new();
    for line in lines {
        if let Ok(record) = serde_json::from_str::<Metrics>(line) {
            metrics_records.push(record);
        }
    }

    if metrics_records.len() < 2 {
        return Err("Not enough data points".into());
    }

    // Extract operation and thread count from filename
    let filename = Path::new(file_path)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    
    let parts: Vec<&str> = filename.split('_').collect();
    let operation = parts.get(0)
        .and_then(|s| s.strip_prefix("metrics-"))
        .unwrap_or("unknown")
        .to_string();
    let thread_count: u32 = parts.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Calculate metrics
    let first = &metrics_records[0];
    let last = &metrics_records[metrics_records.len() - 1];

    let duration_ms = last.ts_ms.saturating_sub(first.ts_ms);
    let duration_sec = duration_ms as f64 / 1000.0;

    let io_read_bytes = last.io_read_bytes_total.saturating_sub(first.io_read_bytes_total);
    let io_write_bytes = last.io_write_bytes_total.saturating_sub(first.io_write_bytes_total);

    let avg_io_read_speed = if duration_sec > 0.0 {
        io_read_bytes as f64 / duration_sec
    } else {
        0.0
    };

    let avg_io_write_speed = if duration_sec > 0.0 {
        io_write_bytes as f64 / duration_sec
    } else {
        0.0
    };

    let max_ram_kb = metrics_records
        .iter()
        .map(|m| m.rss_kb_total)
        .max()
        .unwrap_or(0);

    // Calculate average CPU utilization per CPU
    let mut avg_cpu_utilization: HashMap<String, f64> = HashMap::new();
    
    // Get all CPU names (excluding the aggregate "cpu")
    let cpu_names: Vec<String> = first.per_cpu_jiffies
        .keys()
        .filter(|k| k.as_str() != "cpu")
        .cloned()
        .collect();

    for cpu_name in cpu_names {
        if let (Some(start_jiffies), Some(end_jiffies)) = (
            first.per_cpu_jiffies.get(&cpu_name),
            last.per_cpu_jiffies.get(&cpu_name),
        ) {
            let utilization = calculate_cpu_utilization(start_jiffies, end_jiffies);
            avg_cpu_utilization.insert(cpu_name, utilization);
        }
    }

    Ok(Summary {
        operation,
        thread_count,
        avg_io_read_speed_bytes_per_sec: avg_io_read_speed,
        avg_io_write_speed_bytes_per_sec: avg_io_write_speed,
        max_ram_kb,
        avg_cpu_utilization_per_cpu: avg_cpu_utilization,
        total_duration_ms: duration_ms,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <metrics-file> [metrics-file2] ...", args[0]);
        eprintln!("Example: {} metrics-getall_1_20251123_041843.json", args[0]);
        std::process::exit(1);
    }

    let mut summaries = Vec::new();
    
    for file_path in &args[1..] {
        match analyze_metrics_file(file_path) {
            Ok(summary) => {
                summaries.push(summary);
            }
            Err(e) => {
                eprintln!("Error processing {}: {}", file_path, e);
            }
        }
    }

    // Print results
    for summary in &summaries {
        println!("\n{'=':.>60}", "");
        println!("Operation: {}", summary.operation);
        println!("Thread Count: {}", summary.thread_count);
        println!("Duration: {} ms ({:.2} sec)", summary.total_duration_ms, summary.total_duration_ms as f64 / 1000.0);
        println!("\nI/O Statistics:");
        println!("  Average Read Speed:  {:.2} MB/s", summary.avg_io_read_speed_bytes_per_sec / (1024.0 * 1024.0));
        println!("  Average Write Speed: {:.2} MB/s", summary.avg_io_write_speed_bytes_per_sec / (1024.0 * 1024.0));
        println!("\nMemory:");
        println!("  Max RAM Used: {:.2} MB", summary.max_ram_kb as f64 / 1024.0);
        println!("\nCPU Utilization (Average per CPU):");
        
        let mut cpu_names: Vec<_> = summary.avg_cpu_utilization_per_cpu.keys().collect();
        cpu_names.sort();
        
        for cpu_name in cpu_names {
            if let Some(utilization) = summary.avg_cpu_utilization_per_cpu.get(cpu_name) {
                println!("  {}: {:.2}%", cpu_name, utilization);
            }
        }
    }

    // Export to JSON
    if !summaries.is_empty() {
        let json_output = serde_json::to_string_pretty(&summaries)?;
        fs::write("analysis_summary.json", json_output)?;
        println!("\n{'=':.>60}", "");
        println!("Summary exported to: analysis_summary.json");
    }

    Ok(())
}
