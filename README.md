# ipv4_bloom_cascaded
Ultra-high performance, cache-line aligned bloom filter for IPv4 (or rather any 32-bit int data) filtering. Zero dependencies, no_std compatible. Achieves +75Mpss single-thread and up to 555Mpss multi-thread, outperforming generic filters on raw speed under dense dynamic traffic. A proof on how specific solutions can outperform generic solutions.
