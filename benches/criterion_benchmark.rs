use cascaded_filter::{CascadedFilter, genseed};
use bloomfilter_wrapper::ThirdPartyBloom;
use cuckoofilter_wrapper::ThirdPartyCuckoo;
use xorf_wrapper::ThirdPartyXor;
use flat_filter::FlatBloomFilter;
use fastbloom_wrapper::ThirdPartyFastBloom;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::Rng;
use std::collections::HashSet;
use std::sync::atomic::AtomicU32;
use std::hint::black_box;

fn bench_all_filters(c: &mut Criterion) {
    let handle = std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(move || {
            let mut rng = rand::thread_rng();
            const MALICIOUS: usize = 5_000_000;
            
            let mut attacker_set = HashSet::with_capacity(MALICIOUS);
            while attacker_set.len() < MALICIOUS {
                attacker_set.insert(rng.gen::<u32>());
            }
            let attackers: Vec<u32> = attacker_set.iter().copied().collect();

            let traffic: Vec<[u32; 16]> = (0..1_000_000).map(|_| rng.gen()).collect();

            let cascaded = Box::new(CascadedFilter::new(0));
            genseed(&cascaded.master_seed);
            for &ip in &attackers {
                cascaded.inject_ban(ip);
            }

            let mut third_party_bloom = Box::new(ThirdPartyBloom::new(MALICIOUS, 0.000621));
            for &ip in &attackers {
                third_party_bloom.inject_ban(ip);
            }

            let mut third_party_cuckoo = Box::new(ThirdPartyCuckoo::new(MALICIOUS));
            for &ip in &attackers {
                third_party_cuckoo.inject_ban(ip);
            }

            let xor_filter = Box::new(ThirdPartyXor::build(&attackers));

            let flat_bloom = Box::new(FlatBloomFilter::new(AtomicU32::new(0)));
            for &ip in &attackers {
                flat_bloom.inject_ban(ip);
            }

            let fast_bloom = Box::new(ThirdPartyFastBloom::new(MALICIOUS));
            for &ip in &attackers {
                fast_bloom.inject_ban(ip);
            }

            (cascaded, third_party_bloom, third_party_cuckoo, xor_filter, flat_bloom, fast_bloom, traffic)
        })
        .expect("Couldn't create big-stack thread");

    let (cascaded, third_party_bloom, mut third_party_cuckoo, xor_filter, flat_bloom, fast_bloom, traffic) = 
        handle.join().expect("Thread panic");

    let mut group = c.benchmark_group("IPv4_Filter_Comparison");
    group.throughput(Throughput::Elements(16));

    group.bench_function("Cascaded_Filter", |b| {
        let mut idx = 0;
        b.iter(|| {
            let batch = traffic[idx % traffic.len()];
            idx += 1;
            let results = cascaded.batch_processing(black_box(batch));
            black_box(results);
        })
    });

    group.bench_function("Third_Party_Bloom", |b| {
        let mut idx = 0;
        b.iter(|| {
            let batch = traffic[idx % traffic.len()];
            idx += 1;
            let results = third_party_bloom.batch_processing(black_box(batch));
            black_box(results);
        })
    });

    group.bench_function("Third_Party_Cuckoo", |b| {
        let mut idx = 0;
        b.iter(|| {
            let batch = traffic[idx % traffic.len()];
            idx += 1;
            let results = third_party_cuckoo.batch_processing(black_box(batch));
            black_box(results);
        })
    });

    group.bench_function("Third_Party_Xor_Static", |b| {
        let mut idx = 0;
        b.iter(|| {
            let batch = traffic[idx % traffic.len()];
            idx += 1;
            let results = xor_filter.batch_processing(black_box(batch));
            black_box(results);
        })
    });

    group.bench_function("Flat_Bloom_Filter", |b| {
        let mut idx = 0;
        b.iter(|| {
            let batch = traffic[idx % traffic.len()];
            idx += 1;
            let results = flat_bloom.batch_processing(black_box(batch));
            black_box(results);
        })
    });

    group.bench_function("Third_Party_FastBloom", |b| {
        let mut idx = 0;
        b.iter(|| {
            let batch = traffic[idx % traffic.len()];
            idx += 1;
            let results = fast_bloom.batch_processing(black_box(batch));
            black_box(results);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_all_filters);
criterion_main!(benches);
