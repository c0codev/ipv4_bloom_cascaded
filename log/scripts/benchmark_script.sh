#!/bin/bash

# =====================================================================
# BASIC CONFIG
# =====================================================================
set -e # Stop script in case of a failed command

BINARY_NAME="ipv4_bloom_cascaded"
PROJECT_DIR="/$HOME/ipv4_bloom_cascaded"
RESULTS_DIR="$HOME/benchmark_results"

mkdir -p "$RESULTS_DIR"

ORIG_GOV_FILE=$(mktemp)

# Function that will ALWAYS run when the script exits
cleanup_cpu_governor() {
    echo -e "\n🔄 Restoring user CPU governors to original state..."
    if [ -s "$ORIG_GOV_FILE" ]; then
        while IFS= read -r line; do
            # Extract core index and its original governor configuration
            core_path=$(echo "$line" | cut -d':' -f1)
            orig_gov=$(echo "$line" | cut -d':' -f2)
            if [ -f "$core_path" ]; then
                echo "$orig_gov" | sudo tee "$core_path" > /dev/null 2>&1 || true
            fi
        done < "$ORIG_GOV_FILE"
    fi
    rm -f "$ORIG_GOV_FILE"
    echo -e "\n✅ CPU restored successfully to original user state, this script has finished."
}

# Register the cleanup function to fire on EXIT, INT (Ctrl+C), and TERM signals
trap cleanup_cpu_governor EXIT INT TERM

echo -e "\n🛠 Fetching Github Repo..."
git --version 1>/dev/null 2>/dev/null || echo -e "\nGit command failed, do you have git installed?"
git clone "https://github.com/c0codev/ipv4_bloom_cascaded" || true

# =====================================================================
# 1. ENABLING CPU MAX PERFORMANCE
# =====================================================================
echo -e "\n⚙ Backing up original scaling states and performace modes before changing them..."

if [ -f /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor ]; then
    for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
        if [ -f "$gov" ]; then
            echo "$gov:$(cat "$gov")" >> "$ORIG_GOV_FILE"
        fi
    done
fi

echo -e "\n⚙ Eabling CPU max performance..."
if command -v cpupower &> /dev/null; then
    sudo cpupower frequency-set -g performance > /dev/null 2>&1 || true
elif [ -f /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor ]; then
    echo "performance" | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor > /dev/null 2>&1 || true
fi

# =====================================================================
# 2. GATHERING HARDWARE AND SYSTEM INFO
# =====================================================================
echo -e "\n📊 Extracting hardware and system info..."
echo -e "=== System Hardware (dmidecode) ===\n" > "$RESULTS_DIR/system_info.txt"
sudo dmidecode -s system-manufacturer >> "$RESULTS_DIR/system_info.txt" 2>/dev/null || echo "Unknown Manufacturer (Do you have dmidecode installed?)" >> "$RESULTS_DIR/system_info.txt"
sudo dmidecode -s bios-vendor >> "$RESULTS_DIR/system_info.txt" 2>/dev/null || echo "Unknown BIOS (Do you have dmidecode installed?)" >> "$RESULTS_DIR/system_info.txt"

echo -e "\n=== Compiler Version ===\n" >> "$RESULTS_DIR/system_info.txt"
rustc --version >> "$RESULTS_DIR/system_info.txt" 2>/dev/null || echo "Couldn't find rustc, do you have rust's toolchain installed?"

echo -e "\n=== System Info  ===\n"
uname -a >> "$RESULTS_DIR/system_info.txt"

echo -e "=== Timestamp ===\n" > "$RESULTS_DIR/time_info.txt"
date -u >> "$RESULTS_DIR/time_info.txt"

echo -e "=== lscpu information ===\n" > "$RESULTS_DIR/lscpu_output.txt"
lscpu | grep -E "Model name|Socket|Core|Thread|L1d|L1i|L2|L3" >> "$RESULTS_DIR/lscpu_output.txt" || true
echo -e "\n=== Total nproc OS cores ===" >> "$RESULTS_DIR/lscpu_output.txt"
nproc >> "$RESULTS_DIR/lscpu_output.txt"

# =====================================================================
# 3. COMPILING PROFILE (CONSERVATIVE)
# =====================================================================
cd "$PROJECT_DIR" 2>/dev/null || cd ./ipv4_bloom_cascaded

echo -e "\n🔧 Creating basic compiling profile..."
mkdir -p .cargo
cat << 'EOF' > .cargo/config.toml
[build]
rustflags = [
    "-C", "target-cpu=native"
]
EOF

echo -e "\n🏗 Compiling release build..."
cargo build --release

echo -e "=== Version of Code (Commit) ===\n" > "$RESULTS_DIR/commit_hash.txt"
git log -1 --format="%H %ci" >> "$RESULTS_DIR/commit_hash.txt" 2>/dev/null || echo "No git repository found" >> "$RESULTS_DIR/commit_hash.txt"

# =====================================================================
# 4. BENCHMARK EXECUTIONS
# =====================================================================
echo -e "\n▶ Initiating Single-Thread Benchmarks..."
rm -f "$RESULTS_DIR/benchmark_raw.txt"
for filtro in cascaded bloom cuckoo xor flat fastbloom; do 
    echo -e "\n=== $filtro ===" | tee -a "$RESULTS_DIR/benchmark_raw.txt"
    ./target/release/"$BINARY_NAME" $filtro 2>&1 | tee -a "$RESULTS_DIR/benchmark_raw.txt"
done

echo -e "\n🧪 Initiating Criterion Benchmark (This may take a while)..."
rm -f "$RESULTS_DIR/criterion_raw.txt"
cargo bench 2>&1 | tee -a "$RESULTS_DIR/criterion_raw.txt"

echo -e "\n🏎 Initiating Cascaded Filter Multi-Thread Scaling Benchmark..."
rm -f "$RESULTS_DIR/multithread.txt"
echo -e "=== Cascaded Filter Multithread ===\n" > "$RESULTS_DIR/multithread.txt"
./target/release/"$BINARY_NAME" mt 2>&1 | tee -a "$RESULTS_DIR/multithread.txt"

# =====================================================================
# 5. PERF ANALYSIS
# =====================================================================
echo -e "\n🧠 Analyzing hardware using perf tool..."
rm -f "$RESULTS_DIR/perf_raw.txt"
for filtro in cascaded bloom cuckoo xor flat fastbloom; do 
    echo -e "\n=== $filtro ===" | tee -a "$RESULTS_DIR/perf_raw.txt"
    perf stat -r 10 -e cycles,instructions,L1-dcache-loads,L1-dcache-load-misses,L1-icache-load-misses ./target/release/"$BINARY_NAME" $filtro 2>&1 | tee -a "$RESULTS_DIR/perf_raw.txt" || echo "Perf command failed, do you have perf ins>
done

# =====================================================================
# 6. PREPARING RESULTS
# =====================================================================
echo -e "\n📦 Exporting Criterion HTML reports..."
mkdir -p "$RESULTS_DIR/criterion_html_report/"
if [ -d "target/criterion/report/" ]; then
    cp -r target/criterion/* "$RESULTS_DIR/criterion_html_report/"
fi

cd "$HOME"
tar -czf benchmark_results.tar.gz benchmark_results/
rm -rf benchmark_results/ && echo -e "\n🚀 [DONE!] Benchmarking process finished successfully." && echo -e "\n📥 You have all the results available at: ~/benchmark_results.tar.gz"
