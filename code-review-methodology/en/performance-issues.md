# Finding Performance Issues

## Table of Contents
1. [Inefficient Allocations](#inefficient-allocations)
2. [Duplicate Operations](#duplicate-operations)
3. [Algorithmic Complexity](#algorithmic-complexity)
4. [Excessive Copying](#excessive-copying)
5. [Blocking Operations](#blocking-operations)

---

## Inefficient Allocations

### 1. Repeated Allocations in Loop

**Problem:**
```rust
// SLOW - millions of allocations
fn process_items(items: &[Item]) -> Vec<String> {
    let mut result = Vec::new();  // Initial capacity = 0

    for item in items {
        result.push(item.to_string());  // May reallocate each time!
    }

    result
}
```

**How to find:**
```bash
rg "Vec::new\(\)" --type rust  # Find Vec::new() without with_capacity
```

**Correct:**
```rust
// FAST - single allocation
fn process_items(items: &[Item]) -> Vec<String> {
    let mut result = Vec::with_capacity(items.len());  // Allocate memory upfront

    for item in items {
        result.push(item.to_string());  // Never reallocates
    }

    result
}

// EVEN FASTER - use iterators
fn process_items_fast(items: &[Item]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
    // collect() is smart - allocates correct size automatically
}
```

**Measurable effect:**
- For 10,000 elements: **2-3x faster**
- For 100,000 elements: **5-10x faster**

---

### 2. Unnecessary String Allocations

**Problem:**
```rust
// SLOW - creates new Strings
fn build_url(host: &str, path: &str, query: &str) -> String {
    let mut url = String::new();
    url.push_str("https://");
    url.push_str(host);
    url.push('/');
    url.push_str(path);
    url.push('?');
    url.push_str(query);
    url
}
```

**Correct:**
```rust
// FAST - single allocation of correct size
fn build_url(host: &str, path: &str, query: &str) -> String {
    let capacity = 8 + host.len() + 1 + path.len() + 1 + query.len();
    let mut url = String::with_capacity(capacity);

    url.push_str("https://");
    url.push_str(host);
    url.push('/');
    url.push_str(path);
    url.push('?');
    url.push_str(query);

    url
}

// OR use format! for short strings
fn build_url_simple(host: &str, path: &str, query: &str) -> String {
    format!("https://{}/{}?{}", host, path, query)
}
```

---

### 3. Creating Temporary Vec Instead of Iterators

**Problem:**
```rust
// SLOW - creates intermediate Vec
fn sum_even_squares(nums: &[i32]) -> i32 {
    let evens: Vec<_> = nums.iter().filter(|&n| n % 2 == 0).collect();  // Allocation!
    let squares: Vec<_> = evens.iter().map(|&n| n * n).collect();       // Another one!
    squares.iter().sum()
}
```

**Correct:**
```rust
// FAST - zero allocations
fn sum_even_squares(nums: &[i32]) -> i32 {
    nums.iter()
        .filter(|&n| n % 2 == 0)
        .map(|&n| n * n)
        .sum()
    // No intermediate Vec - all in single pass!
}
```

**Effect:**
- For 1,000,000 elements: **10-20x faster**
- **0 allocations** vs 2 large allocations

---

## Duplicate Operations

### Example from Our Project

**Problem:**
```rust
// SLOW - parsing initData TWICE
pub fn validate_telegram_data(raw: &str, hash: &str, bot_token: &str) -> bool {
    let parsed = parse_query_string_raw(raw);  // 1st time

    validate_auth_date(&parsed);

    let computed_hash = compute_hash(raw, bot_token);  // Parses again inside!
    //                                ^^^
}

fn compute_hash(raw: &str, bot_token: &str) -> String {
    let parsed = parse_query_string_raw(raw);  // 2nd time
    // ...
}
```

**Solution:**
```rust
// FAST - parse once
pub fn validate_telegram_data(raw: &str, hash: &str, bot_token: &str) -> bool {
    let parsed = parse_query_string_raw(raw);  // Once!

    validate_auth_date(&parsed);

    let computed_hash = compute_hash(&parsed, bot_token);  // Pass reference
    //                                 ^^^^^^
}

fn compute_hash(parsed: &BTreeMap<String, String>, bot_token: &str) -> String {
    // Use already parsed data
}
```

**Effect:**
- **-50% time** on parsing
- Especially important for **hot paths** (called frequently)

---

### How to Find Duplication

**1. Search for identical function calls:**
```bash
# Find functions called > 1 time in same function
rg "(\w+)\(.*\).*\1\(" --type rust
```

**2. Visual search:**
- Look for **identical computations** inside loops
- Check if **expensive functions** are called multiple times with same arguments

**Example:**
```rust
// BAD - computes len() on each iteration
for i in 0..items.len() {
    if i < items.len() / 2 {  // len() called again!
        // ...
    }
}

// GOOD
let len = items.len();
let half = len / 2;
for i in 0..len {
    if i < half {
        // ...
    }
}
```

---

## Algorithmic Complexity

### 1. O(n^2) Instead of O(n)

**Problem:**
```rust
// O(n^2) - for each element traverse entire Vec
fn remove_duplicates(items: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();

    for item in items {
        if !result.contains(&item) {  // O(n) for each element!
            result.push(item);
        }
    }

    result
}
```

**Correct:**
```rust
use std::collections::HashSet;

// O(n) - use HashSet
fn remove_duplicates(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for item in items {
        if seen.insert(item.clone()) {  // O(1) on average
            result.push(item);
        }
    }

    result
}

// EVEN SIMPLER
fn remove_duplicates_simple(items: Vec<String>) -> Vec<String> {
    items.into_iter().collect::<HashSet<_>>().into_iter().collect()
}
```

**Effect:**
- For 1,000 elements: **100x faster**
- For 10,000 elements: **1000x faster**

---

### 2. Linear Search Instead of HashMap

**Problem:**
```rust
// O(n) for each search
fn find_user_by_id(users: &[User], id: i64) -> Option<&User> {
    users.iter().find(|u| u.id == id)  // Traverses entire array!
}

// In a loop - disaster:
for id in user_ids {
    let user = find_user_by_id(&all_users, id);  // O(n) each time!
}
// Total: O(m * n) where m = user_ids.len(), n = all_users.len()
```

**Correct:**
```rust
use std::collections::HashMap;

// O(1) for each search
fn build_user_map(users: Vec<User>) -> HashMap<i64, User> {
    users.into_iter().map(|u| (u.id, u)).collect()
}

let user_map = build_user_map(all_users);  // O(n) once

for id in user_ids {
    let user = user_map.get(&id);  // O(1) each time!
}
// Total: O(n + m) instead of O(m * n)
```

**Effect for 10,000 searches in array of 10,000 elements:**
- Was: **~50,000,000** operations (O(m*n))
- Now: **~20,000** operations (O(n+m))
- **2500x faster!**

---

### How to Find Complexity Issues

**1. Nested loops:**
```bash
rg "for.*\{[\s\S]{1,200}for" --type rust  # Find nested loops
```

**2. Vec::contains in loop:**
```bash
rg "\.contains\(" --type rust
```

**3. Linear search patterns:**
```bash
rg "\.iter\(\)\.find|\.iter\(\)\.position" --type rust
```

**Questions to ask yourself:**
- How many times does the innermost operation execute?
- If data grows 10x, how much slower will it be?
- Can I use HashMap/HashSet/BTreeMap?

---

## Excessive Copying

### 1. Clone() Without Necessity

**Problem:**
```rust
// SLOW - copies entire Vec
fn process_items(items: Vec<String>) -> usize {
    let copy = items.clone();  // Why?
    copy.len()
}
```

**How to find:**
```bash
rg "\.clone\(\)" --type rust  # Check each clone!
```

**Correct:**
```rust
// FAST - use reference
fn process_items(items: &[String]) -> usize {
    items.len()
}
```

---

### 2. to_string() in Hot Path

**Problem:**
```rust
// SLOW - millions of allocations
fn log_items(items: &[Item]) {
    for item in items {
        tracing::debug!("Item: {}", item.id.to_string());  // Unnecessary to_string()
    }
}
```

**Correct:**
```rust
// FAST - Display formats itself
fn log_items(items: &[Item]) {
    for item in items {
        tracing::debug!("Item: {}", item.id);  // Without to_string()
    }
}
```

---

### 3. Owned Values Instead of References

**Example from our project:**
```rust
// WAS - copies Strings
let mut kv_pairs: Vec<(String, String)> = parsed.into_iter()...

// NOW - zero-copy
let mut kv_pairs: Vec<(&String, &String)> = parsed.iter()...
```

**Effect:**
- For 100 key-value pairs of 20 characters each: **~4KB** vs **~0 bytes** allocations
- **3-5x faster**

---

## Blocking Operations

### 1. Blocking I/O in Async Function

**Problem:**
```rust
// BLOCKS entire runtime!
async fn handle_request() -> Result<String> {
    let data = std::fs::read_to_string("file.txt")?;  // Sync I/O!
    Ok(data)
}
```

**Correct:**
```rust
// Async read
async fn handle_request() -> Result<String> {
    let data = tokio::fs::read_to_string("file.txt").await?;
    Ok(data)
}

// Or move to blocking pool
async fn handle_request_alt() -> Result<String> {
    tokio::task::spawn_blocking(|| {
        std::fs::read_to_string("file.txt")
    }).await?
}
```

---

### 2. Long Computations in Async

**Problem:**
```rust
// BLOCKS other tasks
async fn expensive_computation(n: u64) -> u64 {
    (0..n).sum()  // Can run milliseconds/seconds
}
```

**Correct:**
```rust
// In separate thread
async fn expensive_computation(n: u64) -> u64 {
    tokio::task::spawn_blocking(move || {
        (0..n).sum()
    }).await.unwrap()
}
```

---

## Profiling Tools

### 1. Cargo Flamegraph
```bash
cargo install flamegraph
cargo flamegraph --bin your-app

# Opens flamegraph.svg - visualization of time spent
```

### 2. Cargo Bench
```bash
# Add to Cargo.toml:
# [dev-dependencies]
# criterion = "0.5"

# benches/my_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_parse(c: &mut Criterion) {
    let data = "auth_date=123&user=test&hash=abc";

    c.bench_function("parse_query_string", |b| {
        b.iter(|| parse_query_string_raw(black_box(data)))
    });
}

criterion_group!(benches, benchmark_parse);
criterion_main!(benches);
```

```bash
cargo bench
```

### 3. Cargo Asm - View Generated Code
```bash
cargo install cargo-asm
cargo asm your_crate::function_name
```

---

## Performance Checklist

### Allocations
- [ ] Vec::new() replaced with Vec::with_capacity() where size is known?
- [ ] No unnecessary .clone() and .to_string()?
- [ ] Iterators used instead of intermediate Vec?
- [ ] String::with_capacity() for concatenation?

### Algorithms
- [ ] No O(n^2) where O(n) is possible?
- [ ] HashMap instead of linear search?
- [ ] No nested loops over large data?
- [ ] Correct data structures chosen?

### Copying
- [ ] References (&T) used instead of owned (T)?
- [ ] No unnecessary .clone()?
- [ ] .to_owned()/.to_string() avoided in hot paths?

### Async
- [ ] No blocking operations in async?
- [ ] Long computations in spawn_blocking?
- [ ] Async I/O used?

### General
- [ ] No duplicate computations?
- [ ] Profiling showed problems?
- [ ] Benchmarks pass within norms?

---

## Optimization Prioritization

### ONLY optimize if:
1. **Profiling showed a problem** (don't optimize blindly!)
2. **It's a hot path** (called frequently)
3. **Measurable effect** (minimum 10% improvement)

### DON'T optimize:
- Cold paths (startup, rare operations)
- Code that's already fast enough
- If it worsens readability without real gain

**Rule:** "Premature optimization is the root of all evil" - Donald Knuth

But: "Premature pessimization is also evil" - choose correct data structures from the start.
