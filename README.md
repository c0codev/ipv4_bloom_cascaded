# ⚡ Cascaded Filter.
## A simple yet lethal bloom filter specialized in Ipv4 filtering *(or rather just u32 type data).*

### **Benchmark Throughput Results**

| 🔎 Filter Variant | 📉 Lower Bound | 📊 Mean | 📈 Upper Bound |
| :--- | :---: | :---: | :---: |
| 📦 **`Xorf`** | 216.50 Melem/s | **216.92 Melem/s** | 217.34 Melem/s |
| 🚀 **`Cascaded`** | 101.15 Melem/s | **102.35 Melem/s** | 103.65 Melem/s |
| 🚗 **`Flat`** | 86.746 Melem/s | **87.181 Melem/s** | 87.683 Melem/s |
| 🐦 **`Cuckoo`** | 60.526 Melem/s | **61.605 Melem/s** | 62.245 Melem/s |
| ✈️ **`Fastbloom`** | 60.537 Melem/s | **61.544 Melem/s** | 62.445 Melem/s |
| 🗃️ **`Bloom`** | 32.932 Melem/s | **33.093 Melem/s** | 33.243 Melem/s |

**You can check the full benchmark and the script I used to get the results at `/log`**.

*(Keep in mind results may vary depending on the CPU)*

---

# 🤔 What is Cascaded Filter?
Cascaded Filter is a **hierarchical bloom filter** specialized on raw speed. 
It all came down to when I was one day staring at the ceiling of my room and said **"How can I filter IPs as fast as possible and using the least space possible?"** 

I inicially deduced that the best possible way to do it with 0% of errors is a **raw bitmap** of the whole IPv4 address space, *(a.k.a a bitmap **2³² bits long**, or rather **512MiB** on human readable terms.)*
But then I stubbornly tried to challenge that initial claim thinking on weird implementations that made little to no sense using existing concepts such as **radix tree structures** or **hashes**... until I got close to the concept of **bloom filters** and started delving into that area.

So I said, **"Okay, this seems like the best way to do it"** *(I later discovered that there are alternatives like **xor** and **cuckoo** filters)* but back then... **bloom** was all I knew and I became determined to do it as best as I could.

In order to achieve that I engineered a **__zero-allocation, lock-free, SIMD friendly design__** and created the `flat_filter.rs` file *(you can check all the source code at `/src`)* but as I got close to finishing it... I thought that maybe, instead of creating a **single plain bitmap** creating **3 different layouts** tailored for **L1d/L2/L3 cache** *(of high end consumer cpus / server cpus)* would make the performance go even higher... And it proved to be right until on the very last runs prior to publishing this, as my original flat filter took a **massive boost** of performance that got me intrigued. 

In the end, after a thorough analysis using **__perf__** and analyzing the **__assembly output__** and **__source code__**... I discovered that the reason was due to an **aggressive vectorization of LLVM** over the code, so I changed my **cascaded filter pipeline** to make it SIMD friendly as well achieving a massive boost of performance that **totally outperformed** my old flat implementation *(which are shown on the initial benchmarks results I posted)*.

---

# ⚙️ How is it designed?

- **🔄 SIMD friendly code**: Loops with a group of items which doesn't rely on anything prior and can **rely on parallelization**.

- **⏩ Optimal pipeline**: It doesn't have branches and it's loops are heavily optimized **minimizing cache and branch misses**.

- **💻 Small computational cost**: It relies almost exclusively on **bit-wise operations, additions and subtractions** and uses a **fast hashing implementation** deriving a random seed combined with a **Murmurhash3 style finalizer** instead of using common, heavy cryptographic hashes like Sip-Hash or SHA-256. *(Note that this implementation **is not by ANY means** cryptographic and it doesn't pretend to be like that)*.

- **🤏 It's oddly specific**: Instead of supporting any kind of data, this filter **only allows u32 data** which strips away tons of scaffolding code that would be necessary if it were to be generic.

- **🧠 Cache-Conscious Design**: Its bitmap sizes are partitioned and tailored for L1d/L2/L3 caches allowing to reduce cache misses and latency but on the counterpart, increases FPR.

  *(Default preset is tailored for high end  consumer CPU's and server CPU's with enough cache space).*

> ⚠️ **Disclaimer**

It's important to note that despite its speed **it is NOT by ANY means an attempt to replace any of the crates I've put to test** as those are way better handling all sorts of data *(fastbloom, bloom, cuckoo)*, have the ability to erase items *(cuckoo)* and can be way faster if being dynamic isn't a requirement *(xor)* but I do think that it poses itself as a good implementation if your target is to **avoid massive DDoS attacks on real time** given its extreme speed and decent FPR.

---

# ✏️ How to replicate the benchmark.

If you want to run the code and benchmark it by yourself I created a script *(the same I used personally)* which already clones the repository and does all the tests.

**Be warned** that you **require** to have a **Linux OS** as well as **rust's toolchain, dmidecode, git and perf installed** on your pc or else it won't run properly.

Anyway if you meet all the prior requirements all you should do is:

1. **📂 Go to the `/log` folder and download the `"bench_script.sh"` file**.

2. **⌨️ Perform `"chmod +x bench_script.sh"` once its downloaded**.

3. **🎬 Execute the script as root**

   *(Root is necessary for **perf and dmidecode** tools. You can check the script source code if you want to know what you're executing exactly).*

**✅ And done**, with that you should be able to get the results. 

If you **see any mistakes** in the source code or anything **please don't be afraid to ask** and I'll be glad to answer as long as it is **kind and respectful**.

---

## 💡 Other places you might be interested to check.
- **🔒 SECURITY.md** *(Explains basic security policies in case of vulnerability)*.
- **🫶 ACCESSIBILITY.md** *(Explains accessibility statement)*.
- **💜 CONTRIBUTING.md** *(Explains rules regarding to contributing to this project)*.
- **⚖️ LICENSE.md** *(MPL 2.0 license text)*.
