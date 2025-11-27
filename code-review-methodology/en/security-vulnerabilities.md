# Finding Security Vulnerabilities

<p align="right"><a href="#table-of-contents">Back to top</a></p>

## Table of Contents
1. [Authentication and Authorization](#authentication-and-authorization)
2. [Cryptography](#cryptography)
3. [Injections](#injections)
4. [Database Security](#database-security)
5. [Data Validation](#data-validation)
6. [Information Leaks](#information-leaks)
7. [Availability Attacks](#availability-attacks)

---

## Authentication and Authorization

<p align="right"><a href="#table-of-contents">Back to top</a></p>

### 1. Replay Attack

**What to look for:**
- Missing timestamp validation
- Missing nonce (one-time tokens)
- Infinite token lifetime

**Vulnerability example:**
```rust
// VULNERABLE - no data freshness check
fn validate_telegram_data(raw: &str, hash: &str, bot_token: &str) -> bool {
    let computed_hash = compute_hmac(raw, bot_token);
    computed_hash == hash  // Hash is correct, but data might be old!
}
```

**How it's exploited:**
1. Attacker intercepts a valid request (e.g., via MITM)
2. Saves authentication data
3. Replays it later to gain access

**Correct solution:**
```rust
// SECURE
fn validate_telegram_data(
    raw: &str,
    hash: &str,
    bot_token: &str,
    max_age: u64
) -> bool {
    let parsed = parse_data(raw);

    // 1. Check data freshness
    if let Some(auth_date) = parsed.get("auth_date") {
        let timestamp = auth_date.parse::<u64>()?;
        let now = current_timestamp();

        if now - timestamp > max_age {
            return false; // Data is stale
        }
    } else {
        return false; // No timestamp
    }

    // 2. Verify signature
    let computed_hash = compute_hmac(raw, bot_token);
    computed_hash == hash
}
```

**Where to look:**
- Token-based authentication
- API endpoints with signatures
- OAuth flows
- Webhook handlers

**Search keywords:**
```bash
rg "validate|auth|verify" --type rust
rg "timestamp|auth_date|created_at" --type rust
```

---

### 2. JWT Without Expiration Check

**What to look for:**
```rust
// VULNERABLE
fn decode_jwt(token: &str) -> Result<Claims> {
    jsonwebtoken::decode(
        token,
        &key,
        &Validation::default()  // Doesn't check exp!
    )
}
```

**Correct:**
```rust
// SECURE
fn decode_jwt(token: &str) -> Result<Claims> {
    let mut validation = Validation::default();
    validation.validate_exp = true;  // Check expiration
    validation.leeway = 60;          // 60 seconds tolerance

    jsonwebtoken::decode(token, &key, &validation)
}
```

---

### 3. Missing Rate Limiting

**What to look for:**
- Authentication endpoints without limits
- Possibility of brute-force attacks

**Vulnerability example:**
```rust
// VULNERABLE - can brute-force passwords
async fn login(username: &str, password: &str) -> Result<Token> {
    let user = db.find_user(username).await?;

    if verify_password(password, &user.password_hash) {
        Ok(generate_token(user.id))
    } else {
        Err(Error::InvalidCredentials)
    }
}
```

**Correct:**
```rust
// SECURE
async fn login(username: &str, password: &str, ip: IpAddr) -> Result<Token> {
    // 1. Check rate limit
    if !rate_limiter.check(ip, 5, Duration::from_secs(60)) {
        return Err(Error::TooManyRequests);
    }

    let user = db.find_user(username).await?;

    // 2. Use constant-time comparison
    if verify_password(password, &user.password_hash) {
        Ok(generate_token(user.id))
    } else {
        // 3. Count attempt
        rate_limiter.increment(ip);

        // 4. Add delay before response (against timing attacks)
        tokio::time::sleep(Duration::from_millis(500)).await;

        Err(Error::InvalidCredentials)
    }
}
```

---

## Cryptography

<p align="right"><a href="#table-of-contents">Back to top</a></p>

### 1. Using Weak Algorithms

**What to look for:**
```bash
rg "md5|sha1|DES|RC4" --type rust  # Weak algorithms
```

**Examples:**
```rust
// BAD - MD5 is cryptographically broken
use md5::Md5;
let hash = Md5::digest(password);

// BAD - SHA1 too
use sha1::Sha1;
let hash = Sha1::digest(token);

// GOOD - use modern algorithms
use sha2::Sha256;
let hash = Sha256::digest(data);

// EVEN BETTER - for passwords use specialized functions
use argon2::{Argon2, PasswordHasher};
let hash = Argon2::default().hash_password(password, &salt)?;
```

---

### 2. Hardcoded Keys and Secrets

**What to look for:**
```bash
# Search for potential secrets
rg "password.*=.*\"" --type rust
rg "secret.*=.*\"" --type rust
rg "api_key.*=.*\"" --type rust
rg "token.*=.*\"" --type rust
rg "private_key" --type rust
```

**Vulnerability example:**
```rust
// CRITICAL VULNERABILITY
const API_KEY: &str = "sk_live_51H...";  // Never!
const DB_PASSWORD: &str = "admin123";    // Disaster!

fn encrypt(data: &[u8]) -> Vec<u8> {
    let key = b"mysecretkey12345";  // Hardcoded key
    aes_encrypt(data, key)
}
```

**Correct:**
```rust
// CORRECT - from environment variables
fn get_api_key() -> Result<String> {
    std::env::var("API_KEY")
        .map_err(|_| Error::MissingApiKey)
}

fn encrypt(data: &[u8]) -> Result<Vec<u8>> {
    // Key from secure storage
    let key = get_encryption_key()?;
    aes_encrypt(data, &key)
}
```

---

### 3. Incorrect HMAC Usage

**Problem from our code:**
```rust
// WAS - can cause panic
let mut mac = HmacSha256::new_from_slice(&secret)
    .expect("HMAC initialization failed");
```

**Why it's bad:**
- `expect()` can cause panic
- In production panic = DOS attack

**Correct:**
```rust
// NOW
let mac = HmacSha256::new_from_slice(&secret).ok()?;
```

---

## Injections

<p align="right"><a href="#table-of-contents">Back to top</a></p>

### 1. SQL Injection

**What to look for:**
```bash
rg "format!.*SELECT|query.*format!" --type rust
rg "execute.*&format!" --type rust
```

**Vulnerability example:**
```rust
// SQL INJECTION!
async fn get_user(username: &str) -> Result<User> {
    let query = format!("SELECT * FROM users WHERE username = '{}'", username);
    //                                                                ^^^^^^^^
    // If username = "admin' OR '1'='1", we'll get all users!

    db.execute(&query).await
}
```

**Correct:**
```rust
// SECURE - use parameterized queries
async fn get_user(username: &str) -> Result<User> {
    sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE username = $1",
        username  // Automatically escaped
    )
    .fetch_one(&db)
    .await
}
```

---

### 2. Command Injection

**What to look for:**
```bash
rg "Command::new|std::process::Command" --type rust
```

**Vulnerability example:**
```rust
// COMMAND INJECTION!
fn resize_image(filename: &str) -> Result<()> {
    let cmd = format!("convert {} -resize 100x100 output.jpg", filename);
    //                           ^^^^^^^^
    // filename = "input.jpg; rm -rf /" will delete files!

    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()?;
    Ok(())
}
```

**Correct:**
```rust
// SECURE
fn resize_image(filename: &str) -> Result<()> {
    // 1. Validate filename
    if !is_valid_filename(filename) {
        return Err(Error::InvalidFilename);
    }

    // 2. Don't use shell - pass arguments directly
    std::process::Command::new("convert")
        .arg(filename)        // Safe - not interpreted by shell
        .arg("-resize")
        .arg("100x100")
        .arg("output.jpg")
        .output()?;
    Ok(())
}

fn is_valid_filename(name: &str) -> bool {
    // Only letters, digits, dots, hyphens
    name.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
}
```

---

## Database Security

<p align="right"><a href="#table-of-contents">Back to top</a></p>

> **Automated Analysis:** Use [sql-query-analyzer](https://github.com/RAprogramm/sql-query-analyzer) for static analysis of SQL queries with 18 built-in security and performance rules.

### Static Analysis with sql-query-analyzer

**Installation:**
```bash
cargo install sql-query-analyzer
```

**Local usage:**
```bash
sql-query-analyzer analyze -s schema.sql -q queries.sql --format text
```

**GitHub Actions integration:**
```yaml
- name: SQL Query Analysis
  uses: RAprogramm/sql-query-analyzer@v1
  with:
    schema: db/schema.sql
    queries: db/queries.sql
    fail-on-error: 'true'
    post-comment: 'true'
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

---

### Security Rules

#### SEC001: UPDATE without WHERE

**Severity:** Error

**What to look for:**
```bash
rg "UPDATE.*SET" --type rust
rg "UPDATE.*SET" --type sql
```

**Vulnerability example:**
```sql
-- DANGEROUS: affects ALL rows in the table
UPDATE users SET status = 'inactive';
```

**Correct:**
```sql
-- SAFE: explicit condition
UPDATE users SET status = 'inactive' WHERE last_login < '2024-01-01';
```

---

#### SEC002: DELETE without WHERE

**Severity:** Error

**What to look for:**
```bash
rg "DELETE FROM" --type rust
rg "DELETE FROM" --type sql
```

**Vulnerability example:**
```sql
-- DANGEROUS: deletes ALL rows
DELETE FROM sessions;
```

**Correct:**
```sql
-- SAFE: explicit condition
DELETE FROM sessions WHERE expired_at < NOW();
```

---

### Performance Rules (Security Impact)

#### PERF001: SELECT * without LIMIT

**Severity:** Warning

**Security impact:** Can cause DOS through memory exhaustion

**Vulnerability example:**
```sql
-- CAN CAUSE OOM on large tables
SELECT * FROM logs;
```

**Correct:**
```sql
-- SAFE: bounded result set
SELECT * FROM logs LIMIT 1000;
-- OR: specific columns
SELECT id, message, created_at FROM logs LIMIT 1000;
```

---

#### PERF002: Leading Wildcard in LIKE

**Severity:** Warning

**Security impact:** Prevents index usage, enables DOS through slow queries

**Vulnerability example:**
```sql
-- SLOW: full table scan, no index usage
SELECT * FROM users WHERE email LIKE '%@gmail.com';
```

**Correct:**
```sql
-- BETTER: use suffix column or full-text search
SELECT * FROM users WHERE email_domain = 'gmail.com';
```

---

#### PERF004: Large OFFSET Values

**Severity:** Warning

**Security impact:** Performance degradation, DOS vector

**Vulnerability example:**
```sql
-- SLOW: database must scan and skip 100000 rows
SELECT * FROM products ORDER BY id LIMIT 10 OFFSET 100000;
```

**Correct:**
```sql
-- FAST: keyset pagination
SELECT * FROM products WHERE id > 100000 ORDER BY id LIMIT 10;
```

---

#### PERF005: Cartesian Product

**Severity:** Error

**Security impact:** Exponential result size, memory exhaustion

**Vulnerability example:**
```sql
-- DANGEROUS: returns rows_a * rows_b results
SELECT * FROM users, orders;
```

**Correct:**
```sql
-- SAFE: explicit join condition
SELECT * FROM users
JOIN orders ON users.id = orders.user_id;
```

---

#### PERF007: Scalar Subquery in SELECT

**Severity:** Warning

**Security impact:** N+1 query pattern, database overload

**Vulnerability example:**
```sql
-- SLOW: executes subquery for EACH row
SELECT
    id,
    name,
    (SELECT COUNT(*) FROM orders WHERE user_id = users.id) as order_count
FROM users;
```

**Correct:**
```sql
-- FAST: single query with JOIN
SELECT u.id, u.name, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name;
```

---

#### PERF008: Function on Indexed Column

**Severity:** Warning

**Security impact:** Index bypass, slow queries

**Vulnerability example:**
```sql
-- SLOW: UPPER() prevents index usage
SELECT * FROM users WHERE UPPER(email) = 'TEST@EXAMPLE.COM';
```

**Correct:**
```sql
-- FAST: store normalized data or use functional index
SELECT * FROM users WHERE email_lower = 'test@example.com';
```

---

#### PERF009: NOT IN with Subquery

**Severity:** Warning

**Security impact:** Unexpected NULL behavior, performance issues

**Vulnerability example:**
```sql
-- DANGEROUS: returns empty if subquery contains NULL
SELECT * FROM users WHERE id NOT IN (SELECT user_id FROM banned);
```

**Correct:**
```sql
-- SAFE: handles NULL correctly
SELECT * FROM users u
WHERE NOT EXISTS (SELECT 1 FROM banned b WHERE b.user_id = u.id);
```

---

#### PERF011: SELECT without WHERE

**Severity:** Info

**Security impact:** Full table scan on large tables

**Vulnerability example:**
```sql
-- SLOW: scans entire table
SELECT * FROM audit_logs;
```

**Correct:**
```sql
-- BETTER: add filtering or pagination
SELECT * FROM audit_logs
WHERE created_at > NOW() - INTERVAL '7 days'
LIMIT 1000;
```

---

### Schema-Aware Rules

#### SCHEMA001: Missing Index on Filter Column

**Severity:** Warning

**What to look for:** WHERE/JOIN columns without indexes

**Example:**
```sql
-- If 'status' column has no index, this is slow
SELECT * FROM orders WHERE status = 'pending';
```

**Fix:** Add index in schema:
```sql
CREATE INDEX idx_orders_status ON orders(status);
```

---

#### SCHEMA002: Column Not in Schema

**Severity:** Warning

**What to look for:** References to non-existent columns

**Example:**
```sql
-- If 'user_name' doesn't exist (should be 'username')
SELECT user_name FROM users;
```

---

### Database Security Checklist

#### Query Safety
- [ ] No `UPDATE` without `WHERE` clause?
- [ ] No `DELETE` without `WHERE` clause?
- [ ] All queries use parameterized statements?
- [ ] No string concatenation in SQL?
- [ ] `LIMIT` applied to unbounded queries?

#### Performance & DOS Prevention
- [ ] No leading wildcards in `LIKE` patterns?
- [ ] Keyset pagination instead of large `OFFSET`?
- [ ] No Cartesian products (missing JOIN conditions)?
- [ ] No N+1 queries (scalar subqueries in SELECT)?
- [ ] Indexes exist for filtered columns?

#### Data Integrity
- [ ] Foreign keys defined and enforced?
- [ ] Constraints validate data at database level?
- [ ] Transactions used for multi-step operations?

### Automated Analysis Tools

```bash
# Install sql-query-analyzer
cargo install sql-query-analyzer

# Analyze queries against schema
sql-query-analyzer analyze -s schema.sql -q queries.sql

# Output as SARIF for CI integration
sql-query-analyzer analyze -s schema.sql -q queries.sql --format sarif

# Disable specific rules if needed
sql-query-analyzer analyze -s schema.sql -q queries.sql --disabled-rules PERF003,STYLE001
```

**GitHub Actions with SARIF upload:**
```yaml
- name: SQL Query Analysis
  uses: RAprogramm/sql-query-analyzer@v1
  with:
    schema: db/schema.sql
    queries: db/queries.sql
    format: sarif
    upload-sarif: 'true'
    fail-on-error: 'true'
```

---

## Data Validation

<p align="right"><a href="#table-of-contents">Back to top</a></p>

### 1. Missing Input Validation

**What to look for in API handlers:**
```rust
// NO VALIDATION
#[derive(Deserialize)]
pub struct CreateUser {
    pub email: String,      // Can be "not-an-email"
    pub age: i32,          // Can be -100
    pub username: String,  // Can be empty or 10000 characters
}

async fn create_user(Json(data): Json<CreateUser>) -> Result<Json<User>> {
    // Create directly without checks!
    let user = db.create_user(data).await?;
    Ok(Json(user))
}
```

**Correct:**
```rust
// WITH VALIDATION
use validator::Validate;

#[derive(Deserialize, Validate)]
pub struct CreateUser {
    #[validate(email)]
    pub email: String,

    #[validate(range(min = 0, max = 150))]
    pub age: i32,

    #[validate(length(min = 3, max = 50))]
    #[validate(regex = "USERNAME_REGEX")]
    pub username: String,
}

async fn create_user(Json(data): Json<CreateUser>) -> Result<Json<User>> {
    // 1. Validate
    data.validate()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // 2. Additional business validation
    if db.username_exists(&data.username).await? {
        return Err(Error::UsernameTaken);
    }

    // 3. Create
    let user = db.create_user(data).await?;
    Ok(Json(user))
}
```

---

### 2. Integer Overflow

**Example from real code:**
```rust
// POTENTIAL PROBLEM
fn calculate_total(price: u64, quantity: u32) -> u64 {
    price * quantity as u64  // Can overflow!
    // If price = u64::MAX and quantity = 2, we get wraparound
}
```

**Correct:**
```rust
// SECURE
fn calculate_total(price: u64, quantity: u32) -> Result<u64> {
    price.checked_mul(quantity as u64)
        .ok_or(Error::Overflow)
}

// Or in debug/test builds will panic:
#[cfg(debug_assertions)]
fn calculate_total_debug(price: u64, quantity: u32) -> u64 {
    price * quantity as u64  // In debug - panic on overflow
}
```

---

## Information Leaks

<p align="right"><a href="#table-of-contents">Back to top</a></p>

### 1. Logging Secrets

**What to look for:**
```bash
rg "tracing::.*password|log.*token|debug.*secret" --type rust
```

**Vulnerability example:**
```rust
// SECRET LEAK TO LOGS
async fn login(username: &str, password: &str) -> Result<Token> {
    tracing::info!("Login attempt: {} with password {}", username, password);
    //                                                                ^^^^^^^^
    // Password will end up in logs!

    // ...
}
```

**Correct:**
```rust
// SECURE
async fn login(username: &str, password: &str) -> Result<Token> {
    tracing::info!("Login attempt for user: {}", username);
    // NEVER log password!

    // For debugging you can log hash (but better not)
    tracing::debug!("Password hash: {}", hash_for_debug_only(password));

    // ...
}
```

---

### 2. Exposing Internal Errors

**Vulnerability example:**
```rust
// EXPOSES DB STRUCTURE
async fn get_user(id: Uuid) -> Result<Json<User>, (StatusCode, String)> {
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
        .fetch_one(&db)
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e)  // DB details to client!
        ))?;

    Ok(Json(user))
}
```

**Correct:**
```rust
// SECURE
async fn get_user(id: Uuid) -> Result<Json<User>, StatusCode> {
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
        .fetch_one(&db)
        .await
        .map_err(|e| {
            // Log details for developers
            tracing::error!("Failed to fetch user {}: {}", id, e);

            // Only generic message to client
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(user))
}
```

---

## Security Checklist

<p align="right"><a href="#table-of-contents">Back to top</a></p>

### Authentication
- [ ] Token/timestamp freshness checked?
- [ ] Rate limiting used?
- [ ] JWT tokens have expiration?
- [ ] Signature/HMAC verified?
- [ ] No hardcoded credentials?

### Cryptography
- [ ] Modern algorithms used (SHA-256+, AES-256)?
- [ ] No hardcoded keys and secrets?
- [ ] Proper error handling (no panic)?
- [ ] Argon2/bcrypt used for passwords?

### Injections
- [ ] SQL queries parameterized?
- [ ] No format! in SQL?
- [ ] Command execution is safe?
- [ ] Filenames/paths validated?

### Validation
- [ ] All input data validated?
- [ ] Number ranges checked?
- [ ] Email/URL validated?
- [ ] String length limited?

### Information Leaks
- [ ] Secrets not logged?
- [ ] Error details not exposed to client?
- [ ] Debug info only in development?

## Automatic Check Tools

```bash
# Clippy with security
cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery

# Dependency check
cargo audit

# Secret search
rg "(?i)(password|secret|api_key|token).*=.*['\"]" --type rust
```
