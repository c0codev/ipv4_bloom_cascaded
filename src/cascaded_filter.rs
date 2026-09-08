#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

#[deny(unsafe_code)]
use core::sync::atomic::{AtomicU32, Ordering};

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{_mm_prefetch, _MM_HINT_T1, _MM_HINT_T2};

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

const L1_BLOCKS: usize = 256;
const L2_BLOCKS: usize = 16384;
const L3_BLOCKS: usize = 131_072;

pub struct CascadedFilter {
    pub master_seed: AtomicU32,
    pub l1_mem_map: [CacheLineBlock; L1_BLOCKS],
    pub l2_mem_map: [CacheLineBlock; L2_BLOCKS],
    pub l3_mem_map: [CacheLineBlock; L3_BLOCKS],
}

pub static FILTER: CascadedFilter = CascadedFilter::new(0);

#[inline(always)]
pub fn genseed(master_seed: &AtomicU32) {
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
        let generated = mix(seed ^ tsc_jitter, 0x9E3779B9);
        master_seed.store(generated, Ordering::Relaxed);
    }
}

#[inline(always)]
fn mix(xor: u32, fibonacci: u32) -> u32 {
    let mut hash = xor ^ fibonacci;
    hash = hash.wrapping_mul(0x85ebca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2ae35);
    hash ^= hash >> 16;
    hash
}

#[deny(unsafe_code)]
struct Indexes {
    l1_cacheline_idx: usize,
    l1_u32_idx: usize,
    l1_bit_pos: u32,

    l2_idx: [usize; 3],
    l2_u32_idx: [usize; 3],
    l2_bit_pos: [u32; 3],

    l3_idx: [usize; 3],
    l3_u32_idx: [usize; 3],
    l3_bit_pos: [u32; 3],
}

#[inline(always)]
fn derive(ip: u32, master_seed: u32) -> Indexes {
    let hash1 = mix(ip, master_seed);
    let hash2 = mix(ip, master_seed ^ 0x27d4_eb2f);
    let hash3 = mix(ip, master_seed ^ 0x9e37_79b9);

    let l1_cacheline_idx = (hash1 & 0xFF) as usize;
    let l1_u32_idx = ((hash1 >> 8) & 0xF) as usize;
    let l1_bit_pos = (hash1 >> 12) & 0x1F;

    let hashes = [hash1, hash2, hash3];

    let mut l2_idx = [0usize; 3];
    let mut l2_u32_idx = [0usize; 3];
    let mut l2_bit_pos = [0u32; 3];

    let mut l3_idx = [0usize; 3];
    let mut l3_u32_idx = [0usize; 3];
    let mut l3_bit_pos = [0u32; 3];

    for i in 0..3 {
        let hash = hashes[i];

        l2_idx[i] = (hash & 0x3FFF) as usize;
        l2_u32_idx[i] = ((hash >> 14) & 0xF) as usize;
        l2_bit_pos[i] = (hash >> 18) & 0x1F;

        l3_idx[i] = (hash & 0x1FFFF) as usize;
        l3_u32_idx[i] = ((hash >> 17) & 0xF) as usize;
        l3_bit_pos[i] = (hash >> 21) & 0x1F;
    }

    Indexes {
        l1_cacheline_idx,
        l1_u32_idx,
        l1_bit_pos,
        l2_idx,
        l2_u32_idx,
        l2_bit_pos,
        l3_idx,
        l3_u32_idx,
        l3_bit_pos,
    }
}

impl CascadedFilter {
    pub const fn new(initial_seed: u32) -> Self {
        Self {
            master_seed: AtomicU32::new(initial_seed),
            l1_mem_map: [const { CacheLineBlock::new() }; L1_BLOCKS],
            l2_mem_map: [const { CacheLineBlock::new() }; L2_BLOCKS],
            l3_mem_map: [const { CacheLineBlock::new() }; L3_BLOCKS],
        }
    }

    #[inline(always)]
    pub fn update_seed(&self, seed: u32) {
        self.master_seed.store(seed, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn get_seed(&self) -> u32 {
        self.master_seed.load(Ordering::Relaxed)
    }

    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    fn prefetch(&self, idx: &Indexes) {
        #[allow(unsafe_code)]
        unsafe {
            for i in 0..3 {
                let l2_pointer = &self.l2_mem_map[idx.l2_idx[i]] as *const CacheLineBlock as *const i8;
                _mm_prefetch::<_MM_HINT_T1>(l2_pointer);

                let l3_pointer = &self.l3_mem_map[idx.l3_idx[i]] as *const CacheLineBlock as *const i8;
                _mm_prefetch::<_MM_HINT_T2>(l3_pointer);
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    #[inline(always)]
    fn prefetch(&self, _idx: &Indexes) { }

    #[forbid(unsafe_code)]
    #[inline(always)]
    pub fn batch_processing(&self, ips: [u32; 16]) -> [u32; 16] {
        let mut results = [0u32; 16];
        let seed = self.get_seed();

        let derived: [Indexes; 16] = [
            derive(ips[0], seed),
            derive(ips[1], seed),
            derive(ips[2], seed),
            derive(ips[3], seed),
            derive(ips[4], seed),
            derive(ips[5], seed),
            derive(ips[6], seed),
            derive(ips[7], seed),
            derive(ips[8], seed),
            derive(ips[9], seed),
            derive(ips[10], seed),
            derive(ips[11], seed),
            derive(ips[12], seed),
            derive(ips[13], seed),
            derive(ips[14], seed),
            derive(ips[15], seed),
        ];

        for derived_ip in &derived {
            self.prefetch(derived_ip);
        }

        for i in 0..16 {
            let idx = &derived[i];

            let reg_l1 = self.l1_mem_map[idx.l1_cacheline_idx].bits[idx.l1_u32_idx].load(Ordering::Relaxed);
            let l1_result = (reg_l1 >> idx.l1_bit_pos) & 1;

            let mut l2_result = 1u32;
            let mut l3_result = 1u32;

            for k in 0..3 {
                let reg_l2 = self.l2_mem_map[idx.l2_idx[k]].bits[idx.l2_u32_idx[k]].load(Ordering::Relaxed);
                l2_result &= (reg_l2 >> idx.l2_bit_pos[k]) & 1;

                let reg_l3 = self.l3_mem_map[idx.l3_idx[k]].bits[idx.l3_u32_idx[k]].load(Ordering::Relaxed);
                l3_result &= (reg_l3 >> idx.l3_bit_pos[k]) & 1;
            }

            results[i] = (l1_result & l2_result & l3_result) ^ 1;
        }

        results
    }

    pub fn inject_ban(&self, ip: u32) {
        let idx = derive(ip, self.get_seed());

        self.l1_mem_map[idx.l1_cacheline_idx].bits[idx.l1_u32_idx].fetch_or(1 << idx.l1_bit_pos, Ordering::Relaxed);

        for ban_flag in 0..3 {
            self.l2_mem_map[idx.l2_idx[ban_flag]].bits[idx.l2_u32_idx[ban_flag]].fetch_or(1 << idx.l2_bit_pos[ban_flag], Ordering::Relaxed);
            self.l3_mem_map[idx.l3_idx[ban_flag]].bits[idx.l3_u32_idx[ban_flag]].fetch_or(1 << idx.l3_bit_pos[ban_flag], Ordering::Relaxed);
        }
    }
}

// Controladores de entrada para no_std / std
#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::x86_64::_mm_pause(); }
    }
}

#[cfg(not(feature = "std"))]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    genseed(&FILTER.master_seed);
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::x86_64::_mm_pause(); }
    }
}
