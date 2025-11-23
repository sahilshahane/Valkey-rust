#!/bin/bash

# Script to run KVStore server and benchmark with CPU affinity


# Configuration
SERVER_CORES="6,7,8,9,10,11"
BENCHMARK_CORES="0,1,2,3,4,5"
SERVER_URL=${SERVER_URL:-"http://localhost:4000"}
RUNNING_TIME=${RUNNING_TIME:-600}


# Build the server and load test
echo "Building server, load test and profiler..."
cargo build --release --bin kvstore > /dev/null 2>&1
cargo build --release --bin load_test > /dev/null 2>&1
cargo build --release --bin profiler > /dev/null 2>&1
echo "Build complete!"
echo ""

# Array of N_CLIENTS to test
# N_CLIENTS_ARR=(1 2 5 7 10 15 20 25 30 35 40 45 50 55 60 65 70 75 80 85 90 95 100)
N_CLIENTS_ARR=(800 1000)

# Generate timestamp for output file
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

# Create benchmark_logs directory if it doesn't exist
mkdir -p benchmark_logs
mkdir -p benchmark_logs/server_logs

echo "======================================"
echo "KVStore Benchmark Runner"
echo "======================================"
echo "Server cores: ${SERVER_CORES}"
echo "Benchmark cores: ${BENCHMARK_CORES}"
echo "Running time: ${RUNNING_TIME} seconds"
echo "Client counts: ${N_CLIENTS_ARR[@]}"
echo "======================================"
echo ""

set -e

# Request sudo privileges upfront (for profiler)
echo "Requesting sudo privileges for profiler..."
sudo -v

# Keep sudo alive in background
(while true; do sudo -n true; sleep 50; done) 2>/dev/null &
SUDO_KEEPER_PID=$!

# Ensure sudo keeper is killed on exit
trap "kill ${SUDO_KEEPER_PID} 2>/dev/null || true" EXIT


# Run all workload types individually
# WORKLOADS=("putall" "getall" "getpopular" "getput" "stress")
WORKLOADS=("putall" "getall")

# Loop through each N_CLIENTS value
for N_CLIENTS in "${N_CLIENTS_ARR[@]}"; do
    echo ""
    echo "======================================"
    echo "Testing with ${N_CLIENTS} clients"
    echo "======================================"
    echo ""
    
    SERVER_LOG_FILE="benchmark_logs/server_logs/server_${N_CLIENTS}_${TIMESTAMP}.log"

for WORKLOAD in "${WORKLOADS[@]}"; do

    # Kill any existing kvstore processes
    echo "Cleaning up any existing kvstore processes..."
    pkill -9 kvstore 2>/dev/null  || true
    rm -rf logs
    sleep 2

    # Start the server in background with CPU affinity
    echo "Starting KVStore server on cores ${SERVER_CORES}..."
    taskset -c ${SERVER_CORES} ./target/release/kvstore > ${SERVER_LOG_FILE} 2>&1 &
    SERVER_PID=$!
    echo "Server started with PID: ${SERVER_PID}"

    # Wait for server to be ready
    echo "Waiting for server to be ready..."
    MAX_RETRIES=30
    RETRY_COUNT=0

    while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
        if ! kill -0 ${SERVER_PID} 2>/dev/null; then
            echo "ERROR: Server process died!"
            echo "Check server logs at: ${SERVER_LOG_FILE}"
            exit 1
        fi
        
        if curl -s -f ${SERVER_URL}/health > /dev/null 2>&1; then
            echo "Server is ready!"
            echo ""
            break
        fi
        
        RETRY_COUNT=$((RETRY_COUNT + 1))
        sleep 1
    done

    if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
        echo "ERROR: Server failed to become ready after ${MAX_RETRIES} seconds!"
        echo "Check server logs at: ${SERVER_LOG_FILE}"
        kill ${SERVER_PID} 2>/dev/null || true
        exit 1
    fi


    OUTPUT_FILE="benchmark_logs/benchmark_${WORKLOAD}_${N_CLIENTS}_${TIMESTAMP}.txt"
    METRICS_FILE="benchmark_logs/metrics-${WORKLOAD}_${N_CLIENTS}_${TIMESTAMP}.json"
    
    echo "======================================"
    echo "Running ${WORKLOAD} workload..."
    echo "======================================"
    
    # Start profiler in background
    echo "Starting profiler for PID ${SERVER_PID}..."
    sudo taskset -c 5 ./target/release/profiler --pid ${SERVER_PID} --interval-ms 1000 --out ${METRICS_FILE} > /dev/null 2>&1 &
    PROFILER_PID=$!
    echo "Profiler started with PID: ${PROFILER_PID}"
    
    # Write configuration header and run benchmark
    {
        echo "======================================"
        echo "Benchmark Configuration"
        echo "======================================"
        echo "Workload: ${WORKLOAD}"
        echo "Server cores: ${SERVER_CORES}"
        echo "Benchmark cores: ${BENCHMARK_CORES}"
        echo "Number of clients: ${N_CLIENTS}"
        echo "Running time: ${RUNNING_TIME} seconds"
        echo "Server URL: ${SERVER_URL}"
        echo "Timestamp: ${TIMESTAMP}"
        echo "======================================"
        echo ""
        
        taskset -c ${BENCHMARK_CORES} ./target/release/load_test ${SERVER_URL} ${N_CLIENTS} ${RUNNING_TIME} ${WORKLOAD} | tail -n 16 | head -n 10 2>&1
    } | tee ${OUTPUT_FILE}
    
    # Stop profiler
    echo "Stopping profiler (PID: ${PROFILER_PID})..."
    sudo kill ${PROFILER_PID} 2>/dev/null || true
    
    # Stop server for this workload
    echo "Stopping server (PID: ${SERVER_PID})..."
    kill ${SERVER_PID} 2>/dev/null || true
    sleep 1
    
    echo ""
    echo "Results saved to: ${OUTPUT_FILE}"
    echo "Metrics saved to: ${METRICS_FILE}"
    echo ""
    
    # Small delay between workloads
    sleep 2
done

done  # End of N_CLIENTS loop

# Cleanup
echo ""
echo "======================================"
echo "All Benchmarks Complete!"
echo "======================================"
echo "Ensuring all processes are stopped..."
pkill -9 kvstore 2>/dev/null || true
sleep 1

echo ""
echo "Summary of all tests:"
echo "Client counts tested: ${N_CLIENTS_ARR[@]}"
echo "Workloads tested: ${WORKLOADS[@]}"
echo "All results saved in benchmark_logs/"
echo ""
