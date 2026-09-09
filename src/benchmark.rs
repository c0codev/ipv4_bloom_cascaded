use ipv4_bloom_cascaded::cascaded_filter::{CascadedFilter, genseed};
use ipv4_bloom_cascaded::cascaded_filter::FILTER;
use ipv4_bloom_cascaded::bloomfilter_wrapper::ThirdPartyBloom;
use ipv4_bloom_cascaded::cuckoofilter_wrapper::ThirdPartyCuckoo;
use ipv4_bloom_cascaded::xorf_wrapper::ThirdPartyXor;
use ipv4_bloom_cascaded::flat_filter::FlatBloomFilter;
use ipv4_bloom_cascaded::flat_filter::FLAT_FILTER;
use ipv4_bloom_cascaded::fastbloom_wrapper::ThirdPartyFastBloom;

use rand::Rng;
use std::collections::HashSet;
use std::hint::black_box;
use std::time::Instant;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::thread;

fn main() {
    let child = std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(real_main)
        .expect("Couldn't create big-stack thread");

    child.join().expect("Thread panic");
}

fn real_main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    const MALICIOUS: u32 = 5_000_000;
    const TOTAL_PACKETS: u32 = 10_000_000;
    const CLEAN_SAMPLES: u32 = 5_000_000;

    let mut rng = rand::thread_rng();

    println!("\nGenerating shared dataset ({} malicious IPs)...", MALICIOUS);
    let mut attacker_set: HashSet<u32> = HashSet::with_capacity(MALICIOUS as usize);
    while attacker_set.len() < MALICIOUS as usize {
        attacker_set.insert(rng.gen::<u32>());
    }
    let attackers: Vec<u32> = attacker_set.iter().copied().collect();

    println!("Pre-generating traffic stream ({} packets)...", TOTAL_PACKETS);
    let traffic: Vec<[u32; 16]> = (0..TOTAL_PACKETS / 16)
        .map(|_| rng.gen())
        .collect();

    println!("Pre-generating clean samples ({} packets)...", CLEAN_SAMPLES);
    let mut clean_batches: Vec<[u32; 16]> = Vec::with_capacity((CLEAN_SAMPLES / 16) as usize);
    while clean_batches.len() < (CLEAN_SAMPLES / 16) as usize {
        let batch: [u32; 16] = rng.gen();
        if batch.iter().any(|ip| attacker_set.contains(ip)) {
            continue;
        }
        clean_batches.push(batch);
    }

    // --- Cascaded Filter ---
    if mode == "all" || mode == "cascaded" {      
        println!("\n▶ CASCADED FILTER (L1/L2/L3) — shared dataset");
        let cascaded = Box::new(CascadedFilter::new(0));
        genseed(&FILTER.master_seed);
        eprintln!("DEBUG: Filter initialized with seed: {:#X}", FILTER.get_seed());

        for &ip in &attackers {
            cascaded.inject_ban(ip);
        }
        run_case("Cascaded", |batch| cascaded.batch_processing(batch), &traffic, &clean_batches);
    }

    // --- Third-party Bloom ---
    if mode == "all" || mode == "bloom" {
        println!("\n▶ THIRD-PARTY BLOOM (crate `bloomfilter`)");
        let mut third_party = ThirdPartyBloom::new(MALICIOUS as usize, 0.000621);
        for &ip in &attackers {
            third_party.inject_ban(ip);
        }
        third_party.debug_info();
        run_case("ThirdPartyBloom", |batch| third_party.batch_processing(batch), &traffic, &clean_batches);
    }

    // --- Third-party Cuckoo ---
    if mode == "all" || mode == "cuckoo" {
        println!("\n▶ THIRD-PARTY CUCKOO (crate `cuckoofilter`)");
        let mut cuckoo = ThirdPartyCuckoo::new(MALICIOUS as usize);
        for &ip in &attackers {
            cuckoo.inject_ban(ip);
        }
        cuckoo.debug_capacity_info(MALICIOUS as usize);
        let (mean_ns, p99_ns, max_ns) = cuckoo.insertion_latency_summary();
        let failures = cuckoo.insert_failures;
        println!("  Insert latency: mean={:.1}ns, p99={}ns, max={}ns", mean_ns, p99_ns, max_ns);
        println!("  Insert failures: {} / {}", failures, MALICIOUS);
        run_case("Cuckoo", |batch| cuckoo.batch_processing(batch), &traffic, &clean_batches);
    }

    // --- Third-party XORf ---
    if mode == "all" || mode == "xor" {
        println!("\n▶ THIRD-PARTY XOR FILTER (crate `xorf`, static)");
        let build_start = Instant::now();
        let xor_filter = Box::new(ThirdPartyXor::build(&attackers));
        let build_time = build_start.elapsed();
        println!("  Build time (one-shot, {} IPs): {:.3} ms", attackers.len(), build_time.as_secs_f64() * 1000.0);
        run_case("Xor", |batch| xor_filter.batch_processing(batch), &traffic, &clean_batches);
    }

    // --- Flat Bloom Filter (Original Design) ---
    if mode == "all" || mode == "flat" {
        println!("\n▶ FLAT BLOOM FILTER (Cascaded but just using a flat memory map. 7 hashes per IP)");
        let flat = Box::new(FlatBloomFilter::new(AtomicU32::new(0)));
        for &ip in &attackers {
            flat.inject_ban(ip);
        }
        genseed(&FLAT_FILTER.master_seed);
        eprintln!("DEBUG: Filter initialized with seed: {:#X}", FLAT_FILTER.get_seed());
        run_case("Flat", |batch| flat.batch_processing(batch), &traffic, &clean_batches);
    }

    // --- Third-Party FastBloom ---
    if mode == "all" || mode == "fastbloom" {
        println!("\n▶ THIRD-PARTY FASTBLOOM (The fastest crate in community)");
        let fast_bloom = Box::new(ThirdPartyFastBloom::new(MALICIOUS as usize));
        
        for &ip in &attackers {
            fast_bloom.inject_ban(ip);
        }
        
        run_case("FastBloom", |batch| fast_bloom.batch_processing(batch), &traffic, &clean_batches);
    }

    // --- Cascaded Filter (Multi-Thread) ---
    if mode == "all" || mode == "mt" {
        println!("\n==============================================================");
        println!("   MULTI-THREAD SCALING: CASCADED FILTER                     ");
        println!("==============================================================");
        let shared_filter = Arc::new(CascadedFilter::new(0));
        genseed(&shared_filter.master_seed);

        for &ip in &attackers {
            shared_filter.inject_ban(ip);
        } 
        for &n in &[1usize, 2, 4, 8, 16] {
            run_multithread_case("Cascaded", Arc::clone(&shared_filter), &traffic, n);
        }
    }
}

fn run_case(
    label: &str,
    mut lookup: impl FnMut([u32; 16]) -> [u32; 16],
    traffic: &[[u32; 16]],
    clean_batches: &[[u32; 16]],
) {
    let start_time = Instant::now();
    for &batch in traffic {
        let results = lookup(black_box(batch));
        black_box(results);
    }
    let duration = start_time.elapsed();

    let total_packets = traffic.len() as u64 * 16;
    let nanoseconds = duration.as_nanos() as f64;
    let throughput_mpss = (total_packets as f64 / (nanoseconds / 1_000_000_000.0)) / 1_000_000.0;
    let latency_per_packet_ns = nanoseconds / total_packets as f64;

    let mut false_positives: u64 = 0;
    let mut total_clean: u64 = 0;
    for &batch in clean_batches {
        let veredictos = lookup(black_box(batch));
        for v in veredictos {
            total_clean += 1;
            if v == 0 {
                false_positives += 1;
            }
        }
    }
    let fp_rate = (false_positives as f64 / total_clean as f64) * 100.0;

    println!("  [{}]", label);
    println!("    - Performance:         {:.2} Mpps", throughput_mpss);
    println!("    - Average Latency:     {:.3} ns/packet", latency_per_packet_ns);
    println!("    - False Positive Rate: {:.4}% ({}/{})", fp_rate, false_positives, total_clean);
    println!("--------------------------------------------------------------");
}

fn run_multithread_case(
    label: &str,
    filter: Arc<CascadedFilter>,
    traffic: &[[u32; 16]],
    num_threads: usize,
) {
    let chunk_size = (traffic.len() / num_threads).max(1);
    let start_time = Instant::now();

    thread::scope(|s| {
        for chunk in traffic.chunks(chunk_size) {
            let filter_ref = Arc::clone(&filter);
            s.spawn(move || {
                for &batch in chunk {
                    let results = filter_ref.batch_processing(black_box(batch));
                    black_box(results);
                }
            });
        }
    });

    let duration = start_time.elapsed();
    let total_packets = traffic.len() as u64 * 16;
    let nanoseconds = duration.as_nanos() as f64;
    let throughput_mpss = (total_packets as f64 / (nanoseconds / 1_000_000_000.0)) / 1_000_000.0;

    println!("  [{} - {} thread(s)]", label, num_threads);
    println!("    - Performance (aggregate): {:.2} Mpps", throughput_mpss);
    println!("    - Total Time:           {:.4} s", duration.as_secs_f64());
    println!("--------------------------------------------------------------");
}
