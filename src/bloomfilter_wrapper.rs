use bloomfilter::Bloom;

pub struct ThirdPartyBloom {
    inner: Bloom<u32>,
    expected_items: usize,
    target_fp_rate: f64,
}

impl ThirdPartyBloom {
    pub fn new(expected_items: usize, target_fp_rate: f64) -> Self {
        let bloom = Bloom::new_for_fp_rate(expected_items, target_fp_rate).expect(" ❌ Critical Error: Invalid parameters for Bloom filter initialization (check fp_rate or capacity bounds) ❌ ");
        Self { inner: bloom, expected_items, target_fp_rate, }
    }

    pub fn debug_info(&self) {
        let bytes = Bloom::<[u8]>::compute_bitmap_size(self.expected_items, self.target_fp_rate);
        let m_bits = (bytes as u64) * 8;
        let mib = bytes as f64 / 1024.0 / 1024.0;
        eprintln!("  🤖 DEBUG: Bloom uses {} bits ({:.2} MiB) for n={}, p={} 💾 ", m_bits, mib, self.expected_items, self.target_fp_rate);
    }

    #[inline(always)]
    pub fn batch_processing(&self, ips: [u32; 16]) -> [u32; 16] {
        let mut results = [0u32; 16];
        for i in 0..16 {
            results[i] = if self.inner.check(&ips[i]) { 0 } else { 1 };
        }
        results
    }

    pub fn inject_ban(&mut self, ip: u32) {
        self.inner.set(&ip);
    }
}
