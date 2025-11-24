

### Running the server
```cli
cargo build --release --bin kvstore
```
## Architecture

![Check image in assets folder](/assets/architecture.svg "Architecture")


## Other Tools
- [Profiler](/profiler "Profiler") - to record system resource usage
- [Metric Analyzer](/metrics_analyzer/ "Metric Analyzer") - Generates a csv from the analysis generated from profiler


## Benchmarks

### Machine Info
```
CPU : Intel i5 11400H
Base Clock : 2.7Ghz
Boosted Clock : 4.5Ghz 
Physical Cores : 6
Logical Cores : 12
RAM : 8GB
Storage : (512gb NVMe) WDC PC SN530 SDBPNPZ-512G-1114
```


#### PUT-ALL: Create/Delete only (disk-bound)
| Metric | Value |
|--------|-------|
| Duration | 30.21s |
| Successful requests | 6,774,936 |
| Failed requests | 0 |
| Throughput | 224,252.92 req/sec |
| Average latency | 0.03ms |
| Success rate | 100.00% |

#### GET-ALL: Read unique keys (disk-bound)
| Metric | Value |
|--------|-------|
| Duration | 30.23s |
| Successful requests | 7,983,004 |
| Failed requests | 0 |
| Throughput | 264,040.82 req/sec |
| Average latency | 0.00ms |
| Success rate | 100.00% |

#### GET-POPULAR: Read hot keys (cache-bound)
| Metric | Value |
|--------|-------|
| Duration | 30.24s |
| Successful requests | 8,137,244 |
| Failed requests | 0 |
| Throughput | 269,109.89 req/sec |
| Average latency | 0.00ms |
| Success rate | 100.00% |

#### GET+PUT: Mixed workload
| Metric | Value |
|--------|-------|
| Duration | 30.24s |
| Successful requests | 7,859,483 |
| Failed requests | 0 |
| Throughput | 259,925.04 req/sec |
| Average latency | 0.00ms |
| Success rate | 100.00% |

#### STRESS: Maximum throughput test (no delays)
| Metric | Value |
|--------|-------|
| Duration | 30.24s |
| Successful requests | 7,594,777 |
| Failed requests | 0 |
| Throughput | 251,152.72 req/sec |
| Average latency | 0.00ms |
| Success rate | 100.00% |