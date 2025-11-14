tail_n=16
head_n=10
running_time=30
n_clients=100

cargo build --release --bin load_test > /dev/null

echo "Running PUT-ALL test (disk-bound)..."
./target/release/load_test http://localhost:4000 $n_clients $running_time putall | tail -n $tail_n | head -n $head_n && echo "\n"

echo "Running GET-ALL test (disk-bound)..."
./target/release/load_test http://localhost:4000 $n_clients $running_time getall | tail -n $tail_n | head -n $head_n && echo "\n"

echo "Running GET-POPULAR test (cache-bound)..."
./target/release/load_test http://localhost:4000 $n_clients $running_time getpopular | tail -n $tail_n | head -n $head_n && echo "\n"

echo "Running GET+PUT test (mixed workload)..."
./target/release/load_test http://localhost:4000 $n_clients $running_time getput | tail -n $tail_n | head -n $head_n && echo "\n"

echo "Running STRESS test (maximum throughput)..."
./target/release/load_test http://localhost:4000 $n_clients $running_time stress | tail -n $tail_n | head -n $head_n