use xorf::{Filter, Xor8};

pub struct ThirdPartyXor {
    inner: Xor8,
}

impl ThirdPartyXor {
    pub fn build(attacker_ips: &[u32]) -> Self {
        let keys: Vec<u64> = attacker_ips.iter().map(|&ip| ip as u64).collect();
        let inner = Xor8::try_from(keys.as_slice()).expect("xorf build failed (probably due to duplicate keys and internal hash colissions)");
        Self { inner }
    }

    #[inline(always)]
    pub fn batch_processing(&self, ips: [u32; 16]) -> [u32; 16] {
        let mut results = [0u32; 16];
        for i in 0..16 {
            let key = ips[i] as u64;
            results[i] = if self.inner.contains(&key) { 0 } else { 1 };
        }
        results
    }
}
