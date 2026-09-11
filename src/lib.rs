#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "baremetal", no_main)]

// --- ‼️ Main module (always compile either std or no_std/baremetal) 🛠️ ---
pub mod cascaded_filter;


// --- ❗ Benchmarking modules (std_only) 🔨 ---
#[cfg(feature = "std")]
pub mod bloomfilter_wrapper;
#[cfg(feature = "std")]
pub mod cuckoofilter_wrapper;
#[cfg(feature = "std")]
pub mod xorf_wrapper;
#[cfg(feature = "std")]
pub mod flat_filter;
#[cfg(feature = "std")]
pub mod fastbloom_wrapper;
