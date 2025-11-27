# Quick Reference - Code Review Cheat Sheet

Quick reference for daily use.

---

## Critical Issues (Block Merge)

### Security
```bash
# Search for secrets
rg "(?i)(password|secret|api_key|token|private_key).*=.*['\"]" --type rust

# Search for SQL injections
rg "format!.*SELECT|execute.*&format!" --type rust

# Search for command injections
rg "Command::new.*format!" --type rust
```

**Checklist:**
- [ ] No hardcoded credentials
- [ ] SQL queries parameterized
- [ ] All input data validated
- [ ] auth_date checked for replay attack protection
- [ ] No weak algorithms (MD5, SHA1)

---

### Panic in Production
```bash
# Find all unwrap/expect/panic
rg "\.unwrap\(\)|\.expect\(|panic!" --type rust --glob '!tests'
```

**Rule:** No panic in production code (only Result).

---

## Performance Issues

### Quick Checks
```bash
# Nested loops (potential O(n^2))
rg "for.*\{[\s\S]{1,200}for" --type rust

# Vec::new without with_capacity
rg "Vec::new\(\)" --type rust

# contains in loop
rg "\.contains\(" --type rust

# Unnecessary clone
rg "\.clone\(\)" --type rust
```

**Questions:**
- Parsing happens once?
- with_capacity used for Vec?
- No O(n^2) where O(n) is possible?
- Iterators used instead of intermediate Vec?

---

## 5-Minute Checklist

### 1. Security (2 min)
- [ ] No secrets in code
- [ ] No unwrap/expect
- [ ] Input data validated
- [ ] No SQL/Command injections

### 2. Performance (1 min)
- [ ] No obvious O(n^2)
- [ ] No duplicate operations
- [ ] Vec::with_capacity where needed

### 3. Quality (2 min)
- [ ] No code duplication (> 3 times)
- [ ] Functions < 50 lines
- [ ] Sensible variable names
- [ ] Tests for new logic

---

## Common Problem Patterns

### 1. Replay Attack
```rust
// BAD
fn validate(raw: &str, hash: &str) -> bool {
    compute_hash(raw) == hash  // No time check!
}

// GOOD
fn validate(raw: &str, hash: &str, max_age: u64) -> bool {
    check_timestamp(raw, max_age) && compute_hash(raw) == hash
}
```

### 2. Double Parsing
```rust
// BAD - parsing twice
fn validate(raw: &str) -> bool {
    let data = parse(raw);  // 1
    check(data);
    hash(raw)  // parse() called inside again! 2
}

// GOOD
fn validate(raw: &str) -> bool {
    let data = parse(raw);  // 1 time
    check(&data);
    hash(&data)  // pass reference
}
```

### 3. N+1 Query
```rust
// BAD
for id in ids {
    let user = db.get(id).await;  // N queries
}

// GOOD
let users = db.get_many(ids).await;  // 1 query
```

### 4. Unnecessary Clone
```rust
// BAD
fn process(data: Vec<String>) -> usize {
    data.len()  // Takes ownership needlessly
}

// GOOD
fn process(data: &[String]) -> usize {
    data.len()
}
```

---

## Quick Commands

### Problem Search
```bash
# All unwrap/expect
rg "\.unwrap\(\)|\.expect\(" --type rust --glob '!tests'

# Magic numbers
rg "\b[0-9]{3,}\b" --type rust

# TODO/FIXME
rg "TODO|FIXME" --type rust

# Long functions (>50 lines)
# (needs script or manual check)
```

### Metrics
```bash
# Test coverage
cargo tarpaulin --out Xml

# Clippy
cargo clippy -- -D warnings

# Formatting
cargo +nightly fmt --check

# Tests
cargo test

# Benchmarks
cargo bench
```

---

## Prioritization

### MUST FIX (blocks merge)
1. **Security vulnerabilities**
2. **Panic in production**
3. **Logic errors**
4. **Broken tests**

### SHOULD FIX (before merge)
5. **Performance issues (>10% degradation)**
6. **Code duplication (>3 times)**
7. **Missing tests for critical logic**

### NICE TO HAVE (can be separate PR)
8. **Refactoring for readability**
9. **Additional tests**
10. **Documentation improvements**

---

## How to Write Review Comments

### Bad
- "This is bad"
- "Redo this"
- "There's a problem here"

### Good
- "SQL injection possible on line 42. Use parameterized query: `query_as!(...)`"
- "This function is called in a loop, giving O(n^2). Can use HashMap for O(n)"
- "No auth_date check - replay attack vulnerability. Add time validation per Telegram docs"

**Formula:**
1. **What** is wrong
2. **Why** it's a problem
3. **How** to fix (or suggest options)

---

## Focus by Time

### 5 minutes - Critical
- Security
- Panic/unwrap
- Obvious bugs

### 15 minutes - Add Performance
- Duplicate operations
- Inefficient algorithms
- Unnecessary allocations

### 30 minutes - Add Quality
- Code duplication
- Readability
- Tests
- Documentation

### 45 minutes - Thorough Review
- Architecture
- Edge cases in tests
- Long-term maintainability

---

## Comment Template

```markdown
## Security
- [ ] auth_date validation missing (replay attack) - line 42
- [ ] SQL injection in `get_user()` - line 156

## Performance
- [ ] Double parsing of initData - lines 50, 75
- [ ] O(n^2) in `remove_duplicates()` - line 200

## Quality
- [ ] Hash computation duplicated 3 times - suggest extract function
- [ ] Missing tests for error cases

## Suggestions
- Consider using HashMap instead of Vec::find for O(1) lookups
- Add doc comments for public API
```

---

## Quick Start

1. **Open PR** -> view diff
2. **Security** (5 min):
   - `rg` secrets, unwrap, injections
3. **Performance** (5 min):
   - Duplication, O(n^2), allocations
4. **Quality** (5 min):
   - DRY, readability, tests
5. **Write comments** - specific with solutions
6. **Approve or Request Changes**

**Total: 15-20 minutes for typical PR**

---

## Full Documentation

See details in:
- [security-vulnerabilities.md](./security-vulnerabilities.md)
- [performance-issues.md](./performance-issues.md)
- [code-quality.md](./code-quality.md)
- [rust-specific.md](./rust-specific.md)
- [examples.md](./examples.md)
