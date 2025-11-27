# Code Quality and Maintainability

## Table of Contents
1. [Code Duplication (DRY)](#code-duplication-dry)
2. [Readability](#readability)
3. [Documentation](#documentation)
4. [Testing](#testing)
5. [Architectural Principles](#architectural-principles)

---

## Code Duplication (DRY)

### Don't Repeat Yourself - Rule of Three

**Rule:** If code repeats **3+ times** -> extract to function.

### Project Example (Before Refactoring)

```rust
// DUPLICATION - same code 3 times
#[test]
fn test_case_1() {
    let parsed = parse_query_string_raw(&raw);
    let mut kv_pairs: Vec<(String, String)> = parsed
        .into_iter()
        .filter(|(k, _)| k != "hash" && k != "signature")
        .collect();
    kv_pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let data_check_string = kv_pairs.iter()...
    let secret = Sha256::digest(bot_token.as_bytes());
    let mut mac = HmacSha256::new_from_slice(&secret).unwrap();
    mac.update(data_check_string.as_bytes());
    let computed_hash = hex::encode(mac.finalize().into_bytes());
}

#[test]
fn test_case_2() {
    // ... EXACT SAME CODE ...
}

#[test]
fn test_case_3() {
    // ... AND AGAIN ...
}
```

**After refactoring:**
```rust
// Function used everywhere
fn compute_telegram_hash(parsed: &BTreeMap<String, String>, bot_token: &str) -> String {
    // Code in one place
}

#[test]
fn test_case_1() {
    let hash = compute_telegram_hash(&parsed, bot_token);  // One line!
}

#[test]
fn test_case_2() {
    let hash = compute_telegram_hash(&parsed, bot_token);
}
```

**Benefits:**
- Code **3 times shorter**
- Changes in **one place**
- Less chance of errors

---

### How to Find Duplication

**1. Visually:**
- Scroll through file, look for similar blocks
- Watch for copy-paste

**2. Automatically:**
```bash
# cargo-geiger for duplicates
cargo install tokei
tokei --files

# Or cpd (copy-paste detector)
```

**3. By patterns:**
```bash
# Same call chains
rg "\.iter\(\)\.filter.*\.map.*\.collect" --type rust
```

---

### Exceptions to DRY Rule

**Don't extract to function if:**
1. **Code repeats < 3 times**
2. **Context is different** (similar syntax, different semantics)
3. **Makes code less readable**

Example:
```rust
// OK - though similar, semantics are different
user.validate_email()?;
admin.validate_email()?;
// Don't need validate_any_email(user_or_admin)
```

---

## Readability

### 1. Naming

**Bad names:**
```rust
// Unclear what this is
fn proc(d: &str) -> i32 { ... }
fn handle(x: Vec<u8>) { ... }
fn do_it(s: &State) { ... }

// Too generic
fn manager() { ... }
fn process_data() { ... }
fn helper() { ... }
```

**Good names:**
```rust
// Clear what it does
fn parse_user_id(raw_id: &str) -> Result<i32> { ... }
fn validate_telegram_data(init_data: &str) -> bool { ... }
fn compute_hmac_signature(message: &[u8]) -> String { ... }
```

**Naming rules:**
- Functions: **verb** + what it does (`validate_data`, `compute_hash`)
- Variables: **noun** (`user_id`, `config`, `timestamp`)
- Bool variables: **is_/has_/should_** (`is_valid`, `has_permission`)
- Constants: **SCREAMING_SNAKE_CASE** (`MAX_RETRIES`, `DEFAULT_TIMEOUT`)

---

### 2. Magic Numbers and Magic Strings

**Problem:**
```rust
// What does 86400 mean? What's "hash"?
fn validate(raw: &str) -> bool {
    let age = get_age(raw);
    if age > 86400 { return false; }  // What's this number?

    let data = parse(raw);
    data.get("hash").is_some()  // Why "hash"?
}
```

**Solution:**
```rust
// Constants with names
const MAX_AUTH_AGE_SECONDS: u64 = 86400;  // 24 hours
const FIELD_HASH: &str = "hash";

fn validate(raw: &str) -> bool {
    let age = get_age(raw);
    if age > MAX_AUTH_AGE_SECONDS {
        return false;
    }

    let data = parse(raw);
    data.get(FIELD_HASH).is_some()
}
```

**How to find:**
```bash
# Numbers in code
rg "\b[0-9]{4,}\b" --type rust  # Numbers with 4+ digits

# String literals
rg '"\w+"' --type rust
```

---

### 3. Function Length

**Rule:** Function should fit on screen (~40-50 lines)

**Problem:**
```rust
// 200 lines - impossible to understand
fn handle_request(req: Request) -> Result<Response> {
    // 200 lines of code...
    // Parses something...
    // Validates something...
    // Saves something...
    // Sends something...
    // ...
}
```

**Solution:**
```rust
// Split into understandable parts
fn handle_request(req: Request) -> Result<Response> {
    let data = parse_request(&req)?;
    validate_data(&data)?;
    let result = process_data(data)?;
    save_result(&result)?;
    Ok(build_response(result))
}
```

**Single Responsibility Principle**: One function = one task.

---

### 4. Comments

**When needed:**
```rust
// Explains WHY, not WHAT
fn validate_auth_date(timestamp: u64) -> bool {
    let age = current_time() - timestamp;

    // We check freshness to prevent replay attacks.
    // According to Telegram docs: https://...
    if age > MAX_AUTH_AGE {
        return false;
    }

    true
}
```

**When NOT needed:**
```rust
// Comment duplicates code
// Increment counter by 1
counter += 1;

// Obvious things
// Get user from database
let user = db.get_user(id);

// BETTER rename for clarity
let authenticated_user = db.get_user(id);
```

**By Development Protocol:**
- Regular comments FORBIDDEN
- Only `///` doc-comments for public API

---

## Documentation

### 1. Doc-Comments for Public API

**Must document:**
```rust
/// Validates Telegram Mini App initData
///
/// # Arguments
///
/// * `raw` - Raw initData string from Telegram Mini App
/// * `hash` - Expected hash value extracted from initData
/// * `bot_token` - Telegram Bot API token
/// * `max_auth_age` - Maximum age of auth_date in seconds
///
/// # Returns
///
/// `true` if data is valid and fresh, `false` otherwise
///
/// # Example
///
/// ```
/// use my_crate::validate_telegram_data;
///
/// let raw = "auth_date=123&user=...";
/// let hash = "abc123...";
/// let is_valid = validate_telegram_data(raw, hash, "BOT_TOKEN", 86400, false);
/// ```
///
/// # Security
///
/// This function validates auth_date freshness to prevent replay attacks.
/// See: <https://core.telegram.org/bots/webapps#validating-data-received-via-the-mini-app>
pub fn validate_telegram_data(...) -> bool {
    // ...
}
```

**What to include:**
- What the function does
- Arguments (and their format, if not obvious)
- Return value
- Usage example (automatically tested!)
- Special cases (# Panics, # Errors, # Safety)

---

### 2. Examples as Tests (doctests)

```rust
/// Computes SHA-256 hash of data
///
/// # Example
///
/// ```
/// use my_crate::compute_hash;
///
/// let hash = compute_hash(b"hello");
/// assert_eq!(hash.len(), 64); // SHA-256 = 64 hex chars
/// ```
pub fn compute_hash(data: &[u8]) -> String {
    // ...
}
```

**Check:**
```bash
cargo test --doc
```

If example doesn't compile or assertion fails -> test fails!

---

### 3. README.md for Modules

**For large modules:**
```rust
//! Telegram authentication module
//!
//! This module handles validation of Telegram Mini App initData.
//!
//! # Security
//!
//! - Uses HMAC-SHA256 for signature validation
//! - Validates auth_date freshness to prevent replay attacks
//!
//! # Example
//!
//! ```rust
//! use crate::telegram::validate_telegram_data;
//!
//! let is_valid = validate_telegram_data(...);
//! ```

pub mod validation;
pub mod types;
```

---

## Testing

### 1. Test Structure

**AAA Principle: Arrange, Act, Assert**

```rust
#[test]
fn test_auth_date_expired() {
    // Arrange - prepare data
    let bot_token = "test_token";
    let old_timestamp = 1640995200;
    let raw = format!("auth_date={}&user=...", old_timestamp);

    // Act - perform action
    let result = validate_telegram_data(&raw, "hash", bot_token, 86400, false);

    // Assert - check result
    assert!(!result, "Should reject expired auth_date");
}
```

---

### 2. Edge Case Coverage

**Test:**
- Normal case (happy path)
- Boundary values (0, MAX, -1)
- Invalid data
- Empty values
- All if/match branches

**Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_case() {
        // Normal case
    }

    #[test]
    fn test_empty_input() {
        // Empty string
    }

    #[test]
    fn test_missing_required_field() {
        // Missing required field
    }

    #[test]
    fn test_invalid_format() {
        // Wrong format
    }

    #[test]
    fn test_boundary_values() {
        // 0, MAX_VALUE, MIN_VALUE
    }
}
```

---

### 3. Property-Based Testing

**For generating many test cases:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_parse_never_panics(s in "\\PC*") {
        // For ANY string parser should not panic
        let _ = parse_data(&s);
    }

    #[test]
    fn test_hash_is_deterministic(data in prop::collection::vec(any::<u8>(), 0..1000)) {
        // Hash of same data is always same
        let hash1 = compute_hash(&data);
        let hash2 = compute_hash(&data);
        prop_assert_eq!(hash1, hash2);
    }
}
```

---

### 4. Integration Tests

**Structure:**
```
project/
|-- src/
|   +-- lib.rs
+-- tests/              # Integration tests
    |-- auth_tests.rs
    |-- api_tests.rs
    +-- common/         # Common helpers
        +-- mod.rs
```

**Example:**
```rust
// tests/auth_tests.rs
use my_crate::*;

#[test]
fn test_full_auth_flow() {
    // Test full flow from start to finish
    let app = setup_test_app();

    let response = app.request()
        .post("/auth/telegram")
        .json(&auth_request)
        .send();

    assert_eq!(response.status(), 200);
    // ...
}
```

---

## Architectural Principles

### 1. Single Responsibility Principle (SRP)

**Each module/function does ONE thing:**

```rust
// Too many responsibilities
fn handle_user_action(user_id: i64, action: &str, data: &str) -> Result<()> {
    // 1. Parses data
    // 2. Validates
    // 3. Updates DB
    // 4. Sends email
    // 5. Logs
    // 6. Updates cache
    // ...
}

// Separated by responsibility
mod parser { /* Parsing */ }
mod validator { /* Validation */ }
mod repository { /* DB */ }
mod notifier { /* Email */ }
```

---

### 2. Dependency Injection

**Testability and flexibility:**

```rust
// Tightly coupled to specific DB
fn get_user(id: i64) -> Result<User> {
    let conn = PostgresConnection::new()?;  // Can't substitute in tests
    conn.query("SELECT * FROM users WHERE id = $1", &[&id])
}

// Accept abstraction
trait UserRepository {
    fn get_user(&self, id: i64) -> Result<User>;
}

fn get_user<R: UserRepository>(repo: &R, id: i64) -> Result<User> {
    repo.get_user(id)  // Can substitute mock in tests
}
```

---

### 3. Modularity

**Project structure:**
```
src/
|-- handlers/       # HTTP handlers (one file = one endpoint)
|   |-- auth.rs
|   |-- users.rs
|   +-- profiles.rs
|-- models/         # Data structures
|   +-- user.rs
|-- repository/     # DB operations
|   +-- users.rs
|-- services/       # Business logic
|   +-- auth_service.rs
|-- utils/          # Utilities
|   +-- telegram.rs
+-- main.rs         # Entry point
```

**Rule:** Each module solves ONE task.

---

## Code Quality Checklist

### Readability
- [ ] Function and variable names clear?
- [ ] No magic numbers and strings?
- [ ] Functions < 50 lines?
- [ ] No deep nesting (< 3-4 levels)?

### DRY
- [ ] No code duplication?
- [ ] Common logic extracted to functions?
- [ ] Constants used?

### Documentation
- [ ] Public API documented?
- [ ] Usage examples present?
- [ ] Complex parts explained?

### Tests
- [ ] Happy path covered?
- [ ] Edge cases covered?
- [ ] Error tests present?
- [ ] Integration tests for critical logic?

### Architecture
- [ ] Modules separated by responsibility?
- [ ] Dependencies explicit (not global)?
- [ ] Easy to test?
- [ ] Easy to extend?

---

## Quality Metrics

### Goals for production code:
- **Test coverage**: >= 95% (Protocol requires 100%)
- **Cyclomatic complexity**: <= 10 per function
- **Function length**: <= 50 lines
- **Nesting**: <= 4 levels
- **Clippy warnings**: 0

### Tools:
```bash
# Coverage
cargo tarpaulin --out Xml

# Complexity
cargo install cargo-geiger
cargo geiger

# Metrics
cargo install tokei
tokei

# Quality
cargo clippy -- -D warnings
```
