# Finding Security Vulnerabilities

## Table of Contents
1. [Authentication and Authorization](#authentication-and-authorization)
2. [Cryptography](#cryptography)
3. [Injections](#injections)
4. [Data Validation](#data-validation)
5. [Information Leaks](#information-leaks)
6. [Availability Attacks](#availability-attacks)

---

## Authentication and Authorization

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

## Data Validation

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
