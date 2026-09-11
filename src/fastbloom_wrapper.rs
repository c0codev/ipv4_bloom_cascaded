use fastbloom::AtomicBloomFilter;

pub struct ThirdPartyFastBloom {
    inner: AtomicBloomFilter,
}

impl ThirdPartyFastBloom {
    pub fn new(_malicious_count: usize) -> Self {
        Self {
            inner: AtomicBloomFilter::with_num_bits(75_628_544).hashes(7),
        }
    }

    pub fn inject_ban(&self, ip: u32) {
        self.inner.insert(&ip);
    }

    #[inline(always)]
    pub fn batch_processing(&self, ips: [u32; 16]) -> [u32; 16] {
        let mut results = [0u32; 16];
        for i in 0..16 {
            // --- ✅ 1 = Pass 0 = Banned ❌ ---
            results[i] = if self.inner.contains(&ips[i]) { 0 } else { 1 };
        }
        results
    }
}
