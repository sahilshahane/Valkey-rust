#!/bin/bash

# Configuration
LOGS_DIR="benchmark_logs"
KVSTORE_BINARY="./target/release/kvstore"

if [ ! -d "$LOGS_DIR" ]; then
    echo "Error: $LOGS_DIR directory not found."
    exit 1
fi

if [ ! -f "$KVSTORE_BINARY" ]; then
    echo "Error: $KVSTORE_BINARY not found. Please build the server first."
    exit 1
fi

echo "======================================"
echo "Converting Perf Data to BOLT Format"
echo "======================================"

# Request sudo upfront
sudo -v

# Find all BOLT-formatted (.bolt.data) files in the logs directory
BOLT_DATA_FILES=$(find "$LOGS_DIR" -name "*.bolt.data")

if [ -z "$BOLT_DATA_FILES" ]; then
    echo "No .bolt.data files found in $LOGS_DIR"
    exit 0
fi

for BOLT_FILE in $BOLT_DATA_FILES; do
    echo "Processing profile data: $BOLT_FILE"
    
    # Optimize binary using BOLT
    # Extract filename and remove .bolt.data suffix to create unique binary name
    FILE_NAME=$(basename "${BOLT_FILE%.bolt.data}")
    BOLT_BINARY="${KVSTORE_BINARY}_${FILE_NAME}.bolt"
    
    echo "Optimizing $KVSTORE_BINARY using $BOLT_FILE..."
    
    llvm-bolt "$KVSTORE_BINARY" \
        -data "$BOLT_FILE" \
        -o "$BOLT_BINARY" \
        -reorder-blocks=ext-tsp \
        -reorder-functions=hfsort \
        -split-functions \
        -split-all-cold \
        -dyno-stats
        
    if [ $? -eq 0 ]; then
        echo "Successfully optimized: $BOLT_BINARY"
    else
        echo "Error: BOLT optimization failed for $BOLT_FILE"
    fi
    echo "--------------------------------------"
done

echo "Optimization complete!"
