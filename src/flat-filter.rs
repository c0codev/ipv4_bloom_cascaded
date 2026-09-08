//Cascaded Filter Flat Mem Map Design
#[deny(unsafe_code)]
use core::sync::atomic::{AtomicU32, Ordering};

#[repr(align(64))]
pub struct CacheLineBlock {
    pub bits: [AtomicU32; 16],
}

impl CacheLineBlock {
    const fn new() -> Self {
        Self {
            bits: [const { AtomicU32::new(0) }; 16],
        }
    }
}

#[inline(always)]
pub fn genseed(master_seed: &mut u32) {
    let mut seed: u32 = 0;
    let mut success: u8 = 0;

    #[allow(unsafe_code)]
    unsafe {
        for _ in 0..100 {
            core::arch::asm!(
                "rdseed {0:e}",
                "setc {1}",
                out(reg) seed,
                out(reg_byte) success,
                options(nostack, nomem)
            );
            if success != 0 { break; }
        }
        let tsc_jitter = core::arch::x86_64::_rdtsc() as u32;
        *master_seed = mix(seed ^ tsc_jitter, 0x9E3779B9);
    }
}

#[forbid(unsafe_code)]
const TOTAL_BLOCKS: usize = 147_776 //256 + 16_384 + 131_072 blocks

pub struct FlatBloomFilter {
    pub master_seed: u32,
    pub mem_map: [CacheLineBlock; TOTAL_BLOCKS],
}

#[inline(always)]
fn mix(x: u32, seed: u32) -> u32 {
    let mut h = x ^ seed;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    h
}

struct FlatIndexes {
    idx: [usize; 7],
    u32_idx: [usize; 7],
    bit_pos: [u32; 7],
}

#[inline(always)]
fn derive_flat(ip: u32, master_seed: u32) -> FlatIndexes {
    let mut idx = [0usize; 7];
    let mut u32_idx = [0usize; 7];
    let mut bit_pos = [0u32; 7];

    for i in 0..7 {
        let seed_i = master_seed ^ (0x9e3779b9u32.wrapping_mul(i as u32 + 1));
        let h = mix(ip, seed_i);

        idx[i] = ((h as u64 * TOTAL_BLOCKS as u64) >> 32) as usize;

        let h_pos = h.wrapping_mul(0x2545F491).rotate_left(11);
        u32_idx[i] = (h_pos & 0xF) as usize;
        bit_pos[i] = (h_pos >> 4) & 0x1F;
    }

    FlatIndexes { idx, u32_idx, bit_pos }
}

impl FlatBloomFilter {
    pub const fn new(master_seed: u32) -> Self {
        Self {
            master_seed,
            mem_map: [const { CacheLineBlock::new() }; TOTAL_BLOCKS],
        }
    }

    #[inline(always)]
    pub fn batch_processing(&self, ips: [u32; 8]) -> [u32; 8] {
        let mut results = [0u32; 8];

        for i in 0..8 {
            let idx = derive_flat(ips[i], self.master_seed);
            let mut all_set = 1u32;
            for hash in 0..7 {
                let reg = self.mem_map[idx.idx[hash]].bits[idx.u32_idx[hash]].load(Ordering::Relaxed);
                all_set &= (reg >> idx.bit_pos[hash]) & 1;
            }
            results[i] = all_set ^ 1;
        }

        results
    }

    pub fn inject_ban(&self, ip: u32) {
        let idx = derive_flat(ip, self.master_seed);
        for ban_flag in 0..7 {
            self.mem_map[idx.idx[ban_flag]].bits[idx.u32_idx[ban_flag]]
                .fetch_or(1 << idx.bit_pos[ban_flag], Ordering::Relaxed);
        }
    }
}
