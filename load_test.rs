use std::time::{Duration, Instant};
use tokio::task::JoinSet;
use rand::Rng;

#[derive(Debug, Clone, Copy)]
enum WorkloadType {
    PutAll,      // Only create/delete requests (disk-bound)
    GetAll,      // Only read requests with unique keys (disk-bound)
    GetPopular,  // Only read requests for popular keys (cache-bound)
    GetPut,      // Mixed workload
    Stress,      // Maximum throughput stress test
}

impl WorkloadType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "putall" | "put-all" | "put_all" => Some(WorkloadType::PutAll),
            "getall" | "get-all" | "get_all" => Some(WorkloadType::GetAll),
            "getpopular" | "get-popular" | "get_popular" => Some(WorkloadType::GetPopular),
            "getput" | "get-put" | "get_put" | "mixed" => Some(WorkloadType::GetPut),
            "stress" => Some(WorkloadType::Stress),
            _ => None,
        }
    }

    fn description(&self) -> &str {
        match self {
            WorkloadType::PutAll => "PUT-ALL: Create/Delete only (disk-bound)",
            WorkloadType::GetAll => "GET-ALL: Read unique keys (disk-bound)",
            WorkloadType::GetPopular => "GET-POPULAR: Read hot keys (cache-bound)",
            WorkloadType::GetPut => "GET+PUT: Mixed workload",
            WorkloadType::Stress => "STRESS: Maximum throughput test (no delays)",
        }
    }
}

#[derive(Debug, Clone)]
struct Stats {
    successful_requests: u64,
    failed_requests: u64,
    total_latency_us: u64,
}

impl Stats {
    fn new() -> Self {
        Stats {
            successful_requests: 0,
            failed_requests: 0,
            total_latency_us: 0,
        }
    }

    fn merge(&mut self, other: &Stats) {
        self.successful_requests += other.successful_requests;
        self.failed_requests += other.failed_requests;
        self.total_latency_us += other.total_latency_us;
    }

    fn avg_latency_us(&self) -> f64 {
        if self.successful_requests == 0 {
            0.0
        } else {
            self.total_latency_us as f64 / self.successful_requests as f64
        }
    }
}

// Workload: PUT-ALL - Only create/delete (disk-bound)
async fn run_worker_putall(
    worker_id: usize,
    base_url: String,
    duration: Duration,
) -> Stats {
    let client = reqwest::Client::new();
    let mut stats = Stats::new();
    let start = Instant::now();

    println!("Worker {} started (PUT-ALL workload)", worker_id);

    let mut counter = 0u64;
    while start.elapsed() < duration {
        let key = format!("key_{}_{}_{}", worker_id, counter, rand::random::<u32>());
        let value = format!("value_{}", rand::random::<u64>());

        // CREATE operation
        let set_start = Instant::now();
        let set_result = client
            .post(format!("{}/key/{}", base_url, key))
            .json(&sonic_rs::json!({ "value": value }))
            .send()
            .await;

        match set_result {
            Ok(response) if response.status().is_success() => {
                stats.successful_requests += 1;
                stats.total_latency_us += set_start.elapsed().as_micros() as u64;
            }
            _ => stats.failed_requests += 1,
        }

        // DELETE operation
        let delete_start = Instant::now();
        let delete_result = client
            .delete(format!("{}/key/{}", base_url, key))
            .send()
            .await;

        match delete_result {
            Ok(response) if response.status().is_success() => {
                stats.successful_requests += 1;
                stats.total_latency_us += delete_start.elapsed().as_micros() as u64;
            }
            _ => stats.failed_requests += 1,
        }

        counter += 1;
    }

    println!("Worker {} finished", worker_id);
    stats
}

// Workload: GET-ALL - Only read unique keys (disk-bound, cache misses)
async fn run_worker_getall(
    worker_id: usize,
    base_url: String,
    duration: Duration,
) -> Stats {
    let client = reqwest::Client::new();
    let mut stats = Stats::new();
    let start = Instant::now();

    println!("Worker {} started (GET-ALL workload)", worker_id);

    let mut counter = 0u64;
    while start.elapsed() < duration {
        // Generate unique key for each request (ensures cache miss)
        let key = format!("unique_key_{}_{}_{}", worker_id, counter, rand::random::<u64>());

        let get_start = Instant::now();
        let get_result = client
            .get(format!("{}/key/{}", base_url, key))
            .send()
            .await;

        match get_result {
            Ok(_) => {
                stats.successful_requests += 1;
                stats.total_latency_us += get_start.elapsed().as_micros() as u64;
            }
            _ => stats.failed_requests += 1,
        }

        counter += 1;
    }

    println!("Worker {} finished", worker_id);
    stats
}

// Workload: GET-POPULAR - Only read popular keys (cache-bound, cache hits)
async fn run_worker_getpopular(
    worker_id: usize,
    base_url: String,
    duration: Duration,
) -> Stats {
    let client = reqwest::Client::new();
    let mut stats = Stats::new();
    let start = Instant::now();

    println!("Worker {} started (GET-POPULAR workload)", worker_id);

    // Small set of popular keys (only 10 keys shared across all workers)
    let popular_keys = vec![
        "popular_key_1",
        "popular_key_2",
        "popular_key_3",
        "popular_key_4",
        "popular_key_5",
        "popular_key_6",
        "popular_key_7",
        "popular_key_8",
        "popular_key_9",
        "popular_key_10",
    ];

    // Pre-populate these keys (only worker 0 does this)
    if worker_id == 0 {
        for key in &popular_keys {
            let _ = client
                .post(format!("{}/key/{}", base_url, key))
                .json(&sonic_rs::json!({ "value": format!("popular_value_{}", key) }))
                .send()
                .await;
        }
        println!("Worker 0: Pre-populated popular keys");
    }

    // Wait a bit for worker 0 to populate
    tokio::time::sleep(Duration::from_millis(100)).await;

    while start.elapsed() < duration {
        // Randomly select from popular keys
        let idx = rand::rng().random_range(0..popular_keys.len());
        let key = popular_keys[idx];

        let get_start = Instant::now();
        let get_result = client
            .get(format!("{}/key/{}", base_url, key))
            .send()
            .await;

        match get_result {
            Ok(response) if response.status().is_success() => {
                stats.successful_requests += 1;
                stats.total_latency_us += get_start.elapsed().as_micros() as u64;
            }
            _ => stats.failed_requests += 1,
        }
    }

    println!("Worker {} finished", worker_id);
    stats
}

// Workload: GET+PUT - Mixed workload
async fn run_worker_getput(
    worker_id: usize,
    base_url: String,
    duration: Duration,
) -> Stats {
    let client = reqwest::Client::new();
    let mut stats = Stats::new();
    let start = Instant::now();

    println!("Worker {} started (GET+PUT workload)", worker_id);

    let mut counter = 0u64;
    while start.elapsed() < duration {
        let random = rand::random::<u32>() % 100;

        if random < 70 {
            // 70% GET requests
            let key = if random < 35 {
                // 50% of GETs hit popular keys (cache hits)
                format!("hot_key_{}", rand::random::<u32>() % 20)
            } else {
                // 50% of GETs hit unique keys (cache misses)
                format!("cold_key_{}_{}", worker_id, counter)
            };

            let get_start = Instant::now();
            let get_result = client
                .get(format!("{}/key/{}", base_url, key))
                .send()
                .await;

            match get_result {
                Ok(_) => {
                    stats.successful_requests += 1;
                    stats.total_latency_us += get_start.elapsed().as_micros() as u64;
                }
                _ => stats.failed_requests += 1,
            }
        } else if random < 90 {
            // 20% PUT requests
            let key = format!("key_{}_{}", worker_id, counter);
            let value = format!("value_{}", rand::random::<u64>());

            let set_start = Instant::now();
            let set_result = client
                .post(format!("{}/key/{}", base_url, key))
                .json(&sonic_rs::json!({ "value": value }))
                .send()
                .await;

            match set_result {
                Ok(response) if response.status().is_success() => {
                    stats.successful_requests += 1;
                    stats.total_latency_us += set_start.elapsed().as_micros() as u64;
                }
                _ => stats.failed_requests += 1,
            }
        } else {
            // 10% DELETE requests
            let key = format!("key_{}_{}", worker_id, rand::random::<u32>() % 100);

            let delete_start = Instant::now();
            let delete_result = client
                .delete(format!("{}/key/{}", base_url, key))
                .send()
                .await;

            match delete_result {
                Ok(_) => {
                    stats.successful_requests += 1;
                    stats.total_latency_us += delete_start.elapsed().as_micros() as u64;
                }
                _ => stats.failed_requests += 1,
            }
        }

        counter += 1;
    }

    println!("Worker {} finished", worker_id);
    stats
}

// Workload: STRESS - Maximum throughput stress test
async fn run_worker_stress(
    worker_id: usize,
    base_url: String,
    duration: Duration,
) -> Stats {
    let client = reqwest::Client::new();
    let mut stats = Stats::new();
    let start = Instant::now();

    println!("Worker {} started (STRESS workload)", worker_id);

    // Pre-populate some hot keys for reads
    let hot_keys: Vec<String> = (0..100)
        .map(|i| format!("stress_hot_key_{}", i))
        .collect();

    if worker_id == 0 {
        for key in &hot_keys {
            let _ = client
                .post(format!("{}/key/{}", base_url, key))
                .json(&sonic_rs::json!({ "value": format!("hot_value_{}", key) }))
                .send()
                .await;
        }
        println!("Worker 0: Pre-populated stress test keys");
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut counter = 0u64;
    while start.elapsed() < duration {
        let op = rand::random::<u32>() % 100;

        if op < 60 {
            // 60% GET requests on hot keys (fast, cache hits)
            let idx = rand::rng().random_range(0..hot_keys.len());
            let key = &hot_keys[idx];
            
            let get_start = Instant::now();
            let get_result = client
                .get(format!("{}/key/{}", base_url, key))
                .send()
                .await;

            match get_result {
                Ok(response) if response.status().is_success() => {
                    stats.successful_requests += 1;
                    stats.total_latency_us += get_start.elapsed().as_micros() as u64;
                }
                _ => stats.failed_requests += 1,
            }
        } else if op < 85 {
            // 25% PUT requests
            let key = format!("stress_key_{}_{}", worker_id, counter);
            let value = format!("val_{}", counter);

            let set_start = Instant::now();
            let set_result = client
                .post(format!("{}/key/{}", base_url, key))
                .json(&sonic_rs::json!({ "value": value }))
                .send()
                .await;

            match set_result {
                Ok(response) if response.status().is_success() => {
                    stats.successful_requests += 1;
                    stats.total_latency_us += set_start.elapsed().as_micros() as u64;
                }
                _ => stats.failed_requests += 1,
            }
        } else {
            // 15% DELETE requests
            let key = format!("stress_key_{}_{}", worker_id, rand::random::<u32>() % 1000);

            let delete_start = Instant::now();
            let delete_result = client
                .delete(format!("{}/key/{}", base_url, key))
                .send()
                .await;

            match delete_result {
                Ok(_) => {
                    stats.successful_requests += 1;
                    stats.total_latency_us += delete_start.elapsed().as_micros() as u64;
                }
                _ => stats.failed_requests += 1,
            }
        }

        counter += 1;

        // NO DELAYS - run at maximum speed!
    }

    println!("Worker {} finished", worker_id);
    stats
}

async fn run_load_test(
    base_url: &str,
    num_workers: usize,
    duration_secs: u64,
    workload_type: WorkloadType,
) {
    println!("Starting closed-loop load test:");
    println!("  URL: {}", base_url);
    println!("  Workload: {}", workload_type.description());
    println!("  Workers (concurrent users): {}", num_workers);
    println!("  Duration: {} seconds", duration_secs);
    println!("---");

    let duration = Duration::from_secs(duration_secs);
    let start = Instant::now();
    let mut tasks = JoinSet::new();

    // Spawn workers based on workload type
    for worker_id in 0..num_workers {
        let url = base_url.to_string();
        
        match workload_type {
            WorkloadType::PutAll => {
                tasks.spawn(async move {
                    run_worker_putall(worker_id, url, duration).await
                });
            }
            WorkloadType::GetAll => {
                tasks.spawn(async move {
                    run_worker_getall(worker_id, url, duration).await
                });
            }
            WorkloadType::GetPopular => {
                tasks.spawn(async move {
                    run_worker_getpopular(worker_id, url, duration).await
                });
            }
            WorkloadType::GetPut => {
                tasks.spawn(async move {
                    run_worker_getput(worker_id, url, duration).await
                });
            }
            WorkloadType::Stress => {
                tasks.spawn(async move {
                    run_worker_stress(worker_id, url, duration).await
                });
            }
        }
    }

    // Collect results
    let mut total_stats = Stats::new();
    while let Some(result) = tasks.join_next().await {
        if let Ok(stats) = result {
            total_stats.merge(&stats);
        }
    }

    let elapsed = start.elapsed().as_secs_f64();

    // Print results
    println!("\n=== Load Test Results ===");
    println!("Workload: {}", workload_type.description());
    println!("Duration: {:.2}s", elapsed);
    println!("Successful requests: {}", total_stats.successful_requests);
    println!("Failed requests: {}", total_stats.failed_requests);
    println!(
        "Total requests: {}",
        total_stats.successful_requests + total_stats.failed_requests
    );
    println!(
        "Throughput: {:.2} req/sec",
        (total_stats.successful_requests + total_stats.failed_requests) as f64 / elapsed
    );
    println!("Average latency: {:.2}µs", total_stats.avg_latency_us());
    println!(
        "Success rate: {:.2}%",
        (total_stats.successful_requests as f64
            / (total_stats.successful_requests + total_stats.failed_requests) as f64)
            * 100.0
    );
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();

    let base_url = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("http://localhost:4000");

    let num_workers = args
        .get(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);

    let duration_secs = args
        .get(3)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(30);

    let workload_type = args
        .get(4)
        .and_then(|s| WorkloadType::from_str(s))
        .unwrap_or(WorkloadType::GetPut);

    // First, check if server is reachable
    println!("Checking server health at {}...", base_url);
    let client = reqwest::Client::new();
    match client.get(format!("{}/health", base_url)).send().await {
        Ok(response) if response.status().is_success() => {
            println!("✓ Server is reachable\n");
        }
        Ok(response) => {
            eprintln!("✗ Server returned status: {}", response.status());
            eprintln!("Make sure your KV server is running!");
            return;
        }
        Err(e) => {
            eprintln!("✗ Failed to connect to server: {}", e);
            eprintln!("Make sure your KV server is running at {}", base_url);
            return;
        }
    }

    run_load_test(base_url, num_workers, duration_secs, workload_type).await;
    
    println!("\n=== Workload Types Available ===");
    println!("putall     - Create/Delete only (disk-bound at database)");
    println!("getall     - Read unique keys only (disk-bound, cache misses)");
    println!("getpopular - Read hot keys only (cache-bound, cache hits)");
    println!("getput     - Mixed workload (default, 70% GET, 20% PUT, 10% DELETE)");
    println!("stress     - Maximum throughput test (60% GET, 25% PUT, 15% DELETE, no delays)");
}
