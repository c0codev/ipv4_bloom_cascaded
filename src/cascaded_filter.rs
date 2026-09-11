#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), no_main)]

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
    let hash = xor ^ fibonacci;
    let mut h = hash.wrapping_mul(0xcc9e2d51);
    h = h.rotate_left(15);
    h = h.wrapping_mul(0x1b873593);
    h ^ (h >> 13)
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

    let h_l2_0 = hash1;
    let h_l2_1 = hash1.wrapping_add(hash2);
    let h_l2_2 = h_l2_1.wrapping_add(hash2);

    let l2_idx = [
        (h_l2_0 & 0x3FFF) as usize,
        (h_l2_1 & 0x3FFF) as usize,
        (h_l2_2 & 0x3FFF) as usize,
    ];
    let l2_u32_idx = [
        ((h_l2_0 >> 14) & 0xF) as usize,
        ((h_l2_1 >> 14) & 0xF) as usize,
        ((h_l2_2 >> 14) & 0xF) as usize,
    ];
    let l2_bit_pos = [
        (h_l2_0 >> 18) & 0x1F,
        (h_l2_1 >> 18) & 0x1F,
        (h_l2_2 >> 18) & 0x1F,
    ];

    let h_l3_0 = hash2;
    let h_l3_1 = hash2.wrapping_add(hash3);
    let h_l3_2 = h_l3_1.wrapping_add(hash3);

    let l3_idx = [
        (h_l3_0 & 0x1FFFF) as usize,
        (h_l3_1 & 0x1FFFF) as usize,
        (h_l3_2 & 0x1FFFF) as usize,
    ];
    let l3_u32_idx = [
        ((h_l3_0 >> 17) & 0xF) as usize,
        ((h_l3_1 >> 17) & 0xF) as usize,
        ((h_l3_2 >> 17) & 0xF) as usize,
    ];
    let l3_bit_pos = [
        (h_l3_0 >> 21) & 0x1F,
        (h_l3_1 >> 21) & 0x1F,
        (h_l3_2 >> 21) & 0x1F,
    ];

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


    #[forbid(unsafe_code)]
    #[inline(always)]
    pub fn batch_processing(&self, ips: [u32; 16]) -> [u32; 16] {
        let mut results = [0u32; 16];
        let seed = self.get_seed();

        let seed_xor2 = seed ^ 0x27d4_eb2f;
        let seed_xor3 = seed ^ 0x9e37_79b9;

        let mut h1 = [0u32; 16];
        let mut h2 = [0u32; 16];
        let mut h3 = [0u32; 16];

        let mut l1_results = [0u32; 16];


        for i in 0..16 { h1[i] = mix(ips[i], seed); }
        for i in 0..16 { h2[i] = mix(ips[i], seed_xor2); }
        for i in 0..16 { h3[i] = mix(ips[i], seed_xor3); }

        for i in 0..16 {
            let hash1 = h1[i];
            let l1_cacheline_idx = (hash1 & 0xFF) as usize;
            let l1_u32_idx = ((hash1 >> 8) & 0xF) as usize;
            let l1_bit_pos = (hash1 >> 12) & 0x1F;

            let reg_l1 = self.l1_mem_map[l1_cacheline_idx].bits[l1_u32_idx].load(Ordering::Relaxed);
            l1_results[i] = (reg_l1 >> l1_bit_pos) & 1;
        }

        for i in 0..16 {
            let l1_res = l1_results[i];

            // --- ✅ If L1 = 0, force L2/L3 to also output 0 on the last operation with mask_u32 🔨 ---
            let mask = (0u32.wrapping_sub(l1_res)) as usize;
            let mask_u32 = mask as u32;

            let hash1 = h1[i];
            let hash2 = h2[i];
            let hash3 = h3[i];

            // --- 🥇 L2_0 Index 🥇 ---
            let h_l2_0 = hash1;
            let l2_idx_0 = ((h_l2_0 & 0x3FFF) as usize) & mask;
            let r_l2_0 = (self.l2_mem_map[l2_idx_0].bits[((h_l2_0 >> 14) & 0xF) as usize].load(Ordering::Relaxed) >> ((h_l2_0 >> 18) & 0x1F)) & 1;

            // --- 🥇 L3_0 Index 🥇 ---
            let h_l3_0 = hash2;
            let l3_idx_0 = ((h_l3_0 & 0x1FFFF) as usize) & mask;
            let r_l3_0 = (self.l3_mem_map[l3_idx_0].bits[((h_l3_0 >> 17) & 0xF) as usize].load(Ordering::Relaxed) >> ((h_l3_0 >> 21) & 0x1F)) & 1;

            // --- 🥈 L2_1 Index 🥈 ---
            let h_l2_1 = hash1.wrapping_add(hash2);
            let l2_idx_1 = ((h_l2_1 & 0x3FFF) as usize) & mask;
            let r_l2_1 = (self.l2_mem_map[l2_idx_1].bits[((h_l2_1 >> 14) & 0xF) as usize].load(Ordering::Relaxed) >> ((h_l2_1 >> 18) & 0x1F)) & 1;

            // --- 🥈 L3_1 Index 🥈 ---
            let h_l3_1 = hash2.wrapping_add(hash3);
            let l3_idx_1 = ((h_l3_1 & 0x1FFFF) as usize) & mask;
            let r_l3_1 = (self.l3_mem_map[l3_idx_1].bits[((h_l3_1 >> 17) & 0xF) as usize].load(Ordering::Relaxed) >> ((h_l3_1 >> 21) & 0x1F)) & 1;

            // --- 🥉 L2_2 Index 🥉 ---
            let h_l2_2 = hash1.wrapping_add(hash2.wrapping_mul(2));
            let l2_idx_2 = ((h_l2_2 & 0x3FFF) as usize) & mask;
            let r_l2_2 = (self.l2_mem_map[l2_idx_2].bits[((h_l2_2 >> 14) & 0xF) as usize].load(Ordering::Relaxed) >> ((h_l2_2 >> 18) & 0x1F)) & 1;

            // --- 🥉 L3_2 Index 🥉 ---
            let h_l3_2 = hash2.wrapping_add(hash3.wrapping_mul(2));
            let l3_idx_2 = ((h_l3_2 & 0x1FFFF) as usize) & mask;
            let r_l3_2 = (self.l3_mem_map[l3_idx_2].bits[((h_l3_2 >> 17) & 0xF) as usize].load(Ordering::Relaxed) >> ((h_l3_2 >> 21) & 0x1F)) & 1;

            let l2_final = r_l2_0 & r_l2_1 & r_l2_2;
            let l3_final = r_l3_0 & r_l3_1 & r_l3_2;

            results[i] = ((l2_final & l3_final) & mask_u32) ^ 1;
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
