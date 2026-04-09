#!/bin/bash

# Script to run BOLT-optimized KVStore binaries for their specific workloads

# Configuration
BENCHMARK_CORES="0,1,2,3,4,5,6,7,8,9"
SERVER_CORES="10,11"
SERVER_URL=${SERVER_URL:-"http://localhost:4000"}
RUNNING_TIME=${RUNNING_TIME:-30}
LOGS_DIR="benchmark_logs"
BOLT_BIN_DIR="./target/release"

# Build the load test and profiler (server binaries are already optimized by BOLT)
echo "Building load test and profiler..."
cargo build --release --bin load_test > /dev/null 2>&1
cd profiler && (cargo build --release --bin profiler > /dev/null 2>&1) && cd ..
echo "Build complete!"
echo ""

# Array of N_CLIENTS to test
N_CLIENTS_ARR=(40)

# Workload to BOLT-binary mapping (using the latest generated bolt binaries)
# Note: These names match the pattern: kvstore_perf_<workload>_<params>_<timestamp>.bolt
declare -A WORKLOAD_BINARIES
WORKLOAD_BINARIES["putall"]=$(ls -t ${BOLT_BIN_DIR}/kvstore_perf_putall_*.bolt 2>/dev/null | head -n 1)
WORKLOAD_BINARIES["getall"]=$(ls -t ${BOLT_BIN_DIR}/kvstore_perf_getall_*.bolt 2>/dev/null | head -n 1)
WORKLOAD_BINARIES["getpopular"]=$(ls -t ${BOLT_BIN_DIR}/kvstore_perf_getpopular_*.bolt 2>/dev/null | head -n 1)
WORKLOAD_BINARIES["getput"]=$(ls -t ${BOLT_BIN_DIR}/kvstore_perf_getput_*.bolt 2>/dev/null | head -n 1)

WORKLOADS=("putall" "getall" "getpopular" "getput")

# Generate timestamp for output file
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

# Create directories
mkdir -p ${LOGS_DIR}/bolt_results
mkdir -p ${LOGS_DIR}/server_logs

echo "======================================"
echo "KVStore BOLT-Optimized Benchmark Runner"
echo "======================================"
echo "Server cores: ${SERVER_CORES}"
echo "Benchmark cores: ${BENCHMARK_CORES}"
echo "Running time: ${RUNNING_TIME} seconds"
echo "======================================"

set -e

# Request sudo privileges upfront (for profiler)
echo "Requesting sudo privileges for profiler..."
sudo -v

# Loop through each N_CLIENTS value
for N_CLIENTS in "${N_CLIENTS_ARR[@]}"; do
    echo ""
    echo "======================================"
    echo "Testing with ${N_CLIENTS} clients"
    echo "======================================"
    
    for WORKLOAD in "${WORKLOADS[@]}"; do
        BINARY_PATH=${WORKLOAD_BINARIES[$WORKLOAD]}
        
        if [ -z "$BINARY_PATH" ] || [ ! -f "$BINARY_PATH" ]; then
            echo "SKIPPING: No optimized BOLT binary found for workload: ${WORKLOAD}"
            continue
        fi

        BINARY_NAME=$(basename "$BINARY_PATH")
        SERVER_LOG_FILE="${LOGS_DIR}/server_logs/server_bolt_${WORKLOAD}_${N_CLIENTS}_${TIMESTAMP}.log"
        OUTPUT_FILE="${LOGS_DIR}/bolt_results/benchmark_${WORKLOAD}_${N_CLIENTS}_${TIMESTAMP}.txt"
        METRICS_FILE="${LOGS_DIR}/bolt_results/metrics-${WORKLOAD}_${N_CLIENTS}_${TIMESTAMP}.json"

        # Kill any existing server processes
        echo "Cleaning up processes for ${BINARY_NAME}..."
        pkill -9 "${BINARY_NAME}" 2>/dev/null || true
        pkill -9 "kvstore" 2>/dev/null || true
        rm -rf logs
        sleep 2

        # Start the optimized server
        echo "Starting ${BINARY_NAME} on cores ${SERVER_CORES}..."
        taskset -c ${SERVER_CORES} ${BINARY_PATH} > ${SERVER_LOG_FILE} 2>&1 &
        SERVER_PID=$!

        # Wait for server to be ready
        echo "Waiting for server to be ready..."
        MAX_RETRIES=30
        RETRY_COUNT=0
        while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
            if ! kill -0 ${SERVER_PID} 2>/dev/null; then
                echo "ERROR: Server process died!"
                exit 1
            fi
            if curl -s -f ${SERVER_URL}/health > /dev/null 2>&1; then
                echo "Server is ready!"
                break
            fi
            RETRY_COUNT=$((RETRY_COUNT + 1))
            sleep 1
        done

        if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
            echo "ERROR: Server failed to start!"
            kill ${SERVER_PID} 2>/dev/null || true
            exit 1
        fi

        echo "--------------------------------------"
        echo "Running ${WORKLOAD} workload with BOLT binary..."
        
        # Start profiler
        sudo taskset -c 5 ./profiler/target/release/profiler --pid ${SERVER_PID} --interval-ms 1000 --out ${METRICS_FILE} > /dev/null 2>&1 &
        PROFILER_PID=$!

        # Run benchmark
        {
            echo "======================================"
            echo "BOLT Benchmark Configuration"
            echo "======================================"
            echo "Workload: ${WORKLOAD}"
            echo "BOLT Binary: ${BINARY_NAME}"
            echo "Running time: ${RUNNING_TIME} seconds"
            echo "======================================"
            echo ""
            
            taskset -c ${BENCHMARK_CORES} ./target/release/load_test ${SERVER_URL} ${N_CLIENTS} ${RUNNING_TIME} ${WORKLOAD} | tail -n 16 | head -n 10 2>&1
        } | tee ${OUTPUT_FILE}

        # Stop profiler and server
        sudo kill ${PROFILER_PID} 2>/dev/null || true
        kill ${SERVER_PID} 2>/dev/null || true
        sleep 2
    done
done

echo ""
echo "BOLT Benchmarks complete. Results in ${LOGS_DIR}/bolt_results/"
