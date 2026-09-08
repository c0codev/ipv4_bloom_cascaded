use cuckoofilter::CuckooFilter;
use std::collections::hash_map::DefaultHasher;
use std::time::Instant;

pub struct ThirdPartyCuckoo {
    inner: CuckooFilter<DefaultHasher>,
    pub insert_failures: u64,
    pub insert_latencies_ns: Vec<u64>,
}

impl ThirdPartyCuckoo {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: CuckooFilter::with_capacity(capacity),
            insert_failures: 0,
            insert_latencies_ns: Vec::new(),
        }
    }

//START DEBUG
const BUCKET_SIZE: usize = 4;
fn calculate_actual_capacity(requested_capacity: usize) -> usize {
    if requested_capacity == 0 {
        return 0;
    }
    let min_buckets = (requested_capacity + Self::BUCKET_SIZE - 1) / Self::BUCKET_SIZE;
    let num_buckets = min_buckets.next_power_of_two();
    
    num_buckets * Self::BUCKET_SIZE
}

pub fn debug_capacity_info(&self, requested_capacity: usize) {
    let inserted_items = self.inner.len();
    let max_capacity = Self::calculate_actual_capacity(requested_capacity);

    let load_factor = if max_capacity > 0 {
        (inserted_items as f64 / max_capacity as f64) * 100.0
    } else {
        0.0
    };

    eprintln!(
        "DEBUG: Inserted={}/{} (Requested={}), Load Factor={:.2}%", 
        inserted_items, max_capacity, requested_capacity, load_factor
    );
}
//END DEBUG

    #[inline(always)]
    pub fn batch_processing(&mut self, ips: [u32; 16]) -> [u32; 16] {
        let mut results = [0u32; 16];
        for i in 0..16 {
            results[i] = if self.inner.contains(&ips[i]) { 0 } else { 1 };
        }
        results
    }

    pub fn inject_ban(&mut self, ip: u32) {
        let start = Instant::now();
        let result = self.inner.add(&ip);
        let elapsed_ns = start.elapsed().as_nanos() as u64;

        self.insert_latencies_ns.push(elapsed_ns);

        if result.is_err() {
            self.insert_failures += 1;
        }
    }

    pub fn insertion_latency_summary(&self) -> (f64, u64, u64) {
        let mut latencies = self.insert_latencies_ns.clone();
        if latencies.is_empty() {
            return (0.0, 0, 0);
        }
        latencies.sort_unstable();
        let mean = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
        let p99_idx = ((latencies.len() as f64) * 0.99) as usize;
        let p99 = latencies[p99_idx.min(latencies.len() - 1)];
        let max = *latencies.last().unwrap();
        (mean, p99, max)
    }
}
