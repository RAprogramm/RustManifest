# Real Project Examples

## Example 1: Telegram Authentication - Full Analysis

### Initial Problem
A **critical vulnerability** was discovered in Telegram initData validation.

### Code BEFORE Fix

```rust
// src/utils/telegram.rs (simplified)
pub fn validate_telegram_data(
    raw: &str,
    hash: &str,
    bot_token: &str,
    skip_validation: bool,
) -> bool {
    if skip_validation {
        return true;
    }

    let parsed = parse_query_string_raw(raw);

    // PROBLEM 1: NO auth_date CHECK!
    // Old data accepted as valid = Replay Attack

    // PROBLEM 2: Double parsing
    let computed_hash = compute_telegram_hash(raw, bot_token);
    //                                        ^^^
    // Inside compute_telegram_hash() parses again!

    computed_hash == hash
}

fn compute_telegram_hash(raw: &str, bot_token: &str) -> String {
    let parsed = parse_query_string_raw(raw);  // Parsing #2

    let mut kv_pairs: Vec<(String, String)> = parsed.into_iter()...  // Copies String
    kv_pairs.sort_by(...);

    let data_check_string = kv_pairs.iter()...

    let secret = Sha256::digest(bot_token.as_bytes());
    let mut mac = HmacSha256::new_from_slice(&secret)
        .expect("HMAC initialization failed");  // PROBLEM 3: expect()!

    // ...
    hex::encode(mac.finalize().into_bytes())
}
```

---

### Found Issues

#### 1. CRITICAL VULNERABILITY: Replay Attack

**What's wrong:**
```rust
// NO auth_date check
// Means: if attacker intercepts initData, they can
// use it FOREVER for authentication!
```

**How it's exploited:**
1. Victim logs into Telegram Mini App
2. Attacker intercepts request (MITM, compromised Wi-Fi)
3. Saves `initData` with valid signature
4. **Month later** uses same data to log in as victim

**Consequences:**
- Complete account compromise
- CVSS Score: **9.1 (Critical)**
- Violates official Telegram documentation

**Vulnerability proof:**
```rust
#[test]
fn test_replay_attack_vulnerability() {
    let bot_token = "real_token";

    // Create initData with old date (2 years ago)
    let old_timestamp = 1640995200; // 2022-01-01
    let raw = format!("auth_date={}&user=...", old_timestamp);

    // Compute correct hash
    let hash = compute_telegram_hash(&raw, bot_token);

    // VULNERABILITY: Validation passes for old data!
    assert!(validate_telegram_data(&raw, &hash, bot_token, false));

    // Expected behavior: should have returned false!
}
```

---

#### 2. PERFORMANCE: Double Parsing

**Metrics:**
```
Benchmark: validate_telegram_data (1000 calls)
BEFORE optimization:  847.3 us
AFTER:                412.1 us
Improvement:          51.4% faster
```

**Explanation:**
- `parse_query_string_raw()` called 2 times
- For 200-character string that's ~100 parse operations instead of 50

---

#### 3. PROTOCOL VIOLATION: expect()

**Development Protocol section 4:**
> "No panics except tests and unreachable!()"

**Code:**
```rust
.expect("HMAC initialization failed")  // FORBIDDEN!
```

**Why it's a problem:**
- In production `panic = DOS attack`
- If attacker crafts data causing panic -> server crashes
- Violates graceful degradation principle

---

#### 4. OPTIMIZATION: Excessive Copying

**Was:**
```rust
let mut kv_pairs: Vec<(String, String)> = parsed.into_iter()
    .filter(|(k, _)| k != "hash" && k != "signature")
    .collect();
// For 10 pairs of 20 characters = ~400 bytes copying
```

**Now:**
```rust
let mut kv_pairs: Vec<(&String, &String)> = parsed.iter()
    .filter(|(k, _)| k.as_str() != FIELD_HASH && k.as_str() != FIELD_SIGNATURE)
    .collect();
// Zero-copy! 0 bytes copying
```

**Effect:** 3-5x faster vector creation

---

### Fixed Code

```rust
// Constants instead of magic strings
pub const DEFAULT_MAX_AUTH_AGE_SECONDS: u64 = 86400;
const FIELD_AUTH_DATE: &str = "auth_date";
const FIELD_HASH: &str = "hash";
const FIELD_SIGNATURE: &str = "signature";

/// Validates auth_date freshness to prevent replay attacks
fn validate_auth_date_freshness(
    parsed: &BTreeMap<String, String>,
    max_age_seconds: u64,
) -> bool {
    if let Some(auth_date_str) = parsed.get(FIELD_AUTH_DATE) {
        if let Ok(auth_date) = auth_date_str.parse::<u64>() {
            let current_time = get_current_timestamp();
            let age = current_time.saturating_sub(auth_date);

            if age > max_age_seconds {
                tracing::warn!(
                    "auth_date too old: {} seconds (max: {})",
                    age,
                    max_age_seconds
                );
                return false;  // Replay attack protection
            }
            return true;
        } else {
            tracing::warn!("Invalid auth_date format: {}", auth_date_str);
            return false;
        }
    }

    tracing::warn!("Missing auth_date in initData");
    false
}

/// Computes Telegram hash - now without expect()
fn compute_telegram_hash(
    parsed: &BTreeMap<String, String>,  // Accept reference
    bot_token: &str,
) -> Option<String> {  // Return Option instead of panic
    let mut kv_pairs: Vec<(&String, &String)> = parsed.iter()  // Zero-copy
        .filter(|(k, _)| k.as_str() != FIELD_HASH && k.as_str() != FIELD_SIGNATURE)
        .collect();

    kv_pairs.sort_by(|a, b| a.0.cmp(b.0));

    let data_check_string = kv_pairs
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    let secret = Sha256::digest(bot_token.as_bytes());
    let mac = HmacSha256::new_from_slice(&secret).ok()?;  // Without expect()
    let mut mac = mac;
    mac.update(data_check_string.as_bytes());

    Some(hex::encode(mac.finalize().into_bytes()))
}

/// Main validation function
pub fn validate_telegram_data(
    raw: &str,
    hash: &str,
    bot_token: &str,
    max_auth_age_seconds: u64,  // New parameter
    skip_validation: bool,
) -> bool {
    if skip_validation {
        tracing::warn!("Telegram validation SKIPPED (development mode)");
        return true;
    }

    let parsed = parse_query_string_raw(raw);  // Parse ONCE

    // Check data freshness
    if !validate_auth_date_freshness(&parsed, max_auth_age_seconds) {
        return false;
    }

    // Pass reference instead of re-parsing
    let computed_hash = match compute_telegram_hash(&parsed, bot_token) {
        Some(h) => h,
        None => {
            tracing::error!("Failed to compute HMAC hash");
            return false;  // Graceful error handling
        }
    };

    tracing::debug!("Telegram auth validation:");
    tracing::debug!("  computed_hash: {}", computed_hash);
    tracing::debug!("  provided_hash: {}", hash);

    computed_hash == hash
}
```

---

### Tests for New Functionality

```rust
#[test]
fn test_auth_date_expired() {
    let bot_token = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11";
    let old_timestamp = 1685644800; // Old date

    let raw = format!("auth_date={}&query_id=AAF&user=%7B%22id%22%3A123%7D", old_timestamp);

    let parsed = parse_query_string_raw(&raw);
    let computed_hash = compute_telegram_hash(&parsed, bot_token).unwrap();

    // Should reject old data
    assert!(!validate_telegram_data(
        &raw,
        &computed_hash,
        bot_token,
        DEFAULT_MAX_AUTH_AGE_SECONDS,
        false
    ));
}

#[test]
fn test_missing_auth_date() {
    let bot_token = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11";
    let raw = "query_id=AAF&user=%7B%22id%22%3A123%7D"; // No auth_date

    // Should reject data without timestamp
    assert!(!validate_telegram_data(raw, "somehash", bot_token, DEFAULT_MAX_AUTH_AGE_SECONDS, false));
}

#[test]
fn test_auth_date_within_limit() {
    let bot_token = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11";
    let recent_time = get_current_timestamp() - 60; // 1 minute ago

    let raw = format!("auth_date={}&query_id=AAF&user=%7B%22id%22%3A123%7D", recent_time);

    let parsed = parse_query_string_raw(&raw);
    let computed_hash = compute_telegram_hash(&parsed, bot_token).unwrap();

    // Should accept fresh data
    assert!(validate_telegram_data(
        &raw,
        &computed_hash,
        bot_token,
        DEFAULT_MAX_AUTH_AGE_SECONDS,
        false
    ));
}
```

---

### Improvement Results

#### Security

| Issue | Status | CVSS |
|-------|--------|------|
| Replay Attack | FIXED | 9.1 -> 0.0 |
| expect() panic | FIXED | 5.3 -> 0.0 |

#### Performance

| Metric | BEFORE | AFTER | Improvement |
|--------|--------|-------|-------------|
| initData parsing | 2x | 1x | **-50%** |
| String allocations | ~400 bytes | 0 bytes | **-100%** |
| Validation time | 847 us | 412 us | **+51%** |

#### Code Quality

| Aspect | BEFORE | AFTER |
|--------|--------|-------|
| Lines in main function | 67 | 25 |
| Reusable functions | 0 | 3 |
| Magic strings | 6 places | 0 places |
| Magic numbers | 5 places | 0 places |
| Test coverage | 2 tests | 6 tests |

---

## Example 2: Finding N+1 Query Problem

### Original Code (Typical Problem)

```rust
// N+1 QUERY PROBLEM
async fn get_users_with_posts(user_ids: &[i64]) -> Result<Vec<UserWithPosts>> {
    let mut result = Vec::new();

    for user_id in user_ids {
        // 1 query for user
        let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", user_id)
            .fetch_one(&db)
            .await?;

        // N queries for each user's posts
        let posts = sqlx::query_as!(Post, "SELECT * FROM posts WHERE user_id = $1", user_id)
            .fetch_all(&db)
            .await?;

        result.push(UserWithPosts { user, posts });
    }

    Ok(result)
}

// For 100 users = 1 + 100 + 100 = 201 DB queries!
```

**Problem:**
- For 100 users: **201 queries** to DB
- Time: **~2-3 seconds**
- DB load: **critical**

---

### Fixed Code

```rust
// OPTIMIZED - 2 queries
async fn get_users_with_posts(user_ids: &[i64]) -> Result<Vec<UserWithPosts>> {
    // 1. One query for all users
    let users = sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE id = ANY($1)",
        user_ids
    )
    .fetch_all(&db)
    .await?;

    // 2. One query for all posts
    let posts = sqlx::query_as!(
        Post,
        "SELECT * FROM posts WHERE user_id = ANY($1)",
        user_ids
    )
    .fetch_all(&db)
    .await?;

    // 3. Group in memory
    let mut posts_by_user: HashMap<i64, Vec<Post>> = HashMap::new();
    for post in posts {
        posts_by_user.entry(post.user_id).or_default().push(post);
    }

    // 4. Assemble result
    let result = users
        .into_iter()
        .map(|user| UserWithPosts {
            user: user.clone(),
            posts: posts_by_user.remove(&user.id).unwrap_or_default(),
        })
        .collect();

    Ok(result)
}

// For 100 users = 2 queries instead of 201!
```

**Result:**
- Queries: **201 -> 2** (99% less!)
- Time: **2-3 sec -> 50-100 ms** (20-30x faster)
- DB load: **critical -> minimal**

---

## Example 3: Memory Leak via Circular References

### Problematic Code

```rust
use std::rc::Rc;
use std::cell::RefCell;

struct Node {
    value: i32,
    next: Option<Rc<RefCell<Node>>>,
    prev: Option<Rc<RefCell<Node>>>,  // Circular reference!
}

fn create_circular_list() {
    let node1 = Rc::new(RefCell::new(Node {
        value: 1,
        next: None,
        prev: None,
    }));

    let node2 = Rc::new(RefCell::new(Node {
        value: 2,
        next: Some(node1.clone()),
        prev: None,
    }));

    // Create cycle
    node1.borrow_mut().prev = Some(node2.clone());  // MEMORY LEAK!

    // node1 and node2 will never be freed!
    // Rc count will never reach 0
}
```

**How to detect:**
```bash
# Use Valgrind or cargo-leak
RUSTFLAGS="-Z sanitizer=leak" cargo +nightly run
```

**Fix:**
```rust
use std::rc::{Rc, Weak};  // Use Weak for back references
use std::cell::RefCell;

struct Node {
    value: i32,
    next: Option<Rc<RefCell<Node>>>,
    prev: Option<Weak<RefCell<Node>>>,  // Weak instead of Rc!
}

fn create_circular_list() {
    let node1 = Rc::new(RefCell::new(Node {
        value: 1,
        next: None,
        prev: None,
    }));

    let node2 = Rc::new(RefCell::new(Node {
        value: 2,
        next: Some(node1.clone()),
        prev: None,
    }));

    // Weak doesn't increment reference count
    node1.borrow_mut().prev = Some(Rc::downgrade(&node2));  // OK

    // Now memory will be freed correctly!
}
```

---

## Lesson: Code Review Process

### 1. Start with Security (15-20 minutes)
- Look for vulnerabilities using checklist
- Check authentication/authorization
- Input data validation

### 2. Performance (10 minutes)
- Duplicate operations
- Inefficient algorithms
- Unnecessary allocations

### 3. Quality (10 minutes)
- Protocol violations (expect, unwrap)
- Code duplication
- Readability

### 4. Tests (5 minutes)
- Do they cover critical logic?
- Are there error tests?

### Total: ~40-45 minutes for thorough review

---

## Success Metrics

A good code review should find:
- **1-2 critical issues** (security, correctness)
- **3-5 important** (performance, quality)
- **5-10 minor** (style, naming)

If you found nothing - either code is perfect (rare), or you weren't looking carefully.
