# Rust-Specific Issues and Patterns

## Table of Contents
1. [Ownership and Borrowing](#ownership-and-borrowing)
2. [Lifetimes](#lifetimes)
3. [Panic vs Result](#panic-vs-result)
4. [Unsafe Code](#unsafe-code)
5. [Traits and Generics](#traits-and-generics)

---

## Ownership and Borrowing

### 1. Unnecessary Ownership Transfer

**Problem:**
```rust
// Takes ownership without reason
fn process_data(data: Vec<String>) -> usize {
    data.len()  // Could just look at length!
}

fn main() {
    let my_data = vec!["a".to_string(), "b".to_string()];
    let len = process_data(my_data);
    // my_data no longer available!
    // println!("{:?}", my_data); // Compile error
}
```

**Correct:**
```rust
// Takes reference
fn process_data(data: &[String]) -> usize {
    data.len()
}

fn main() {
    let my_data = vec!["a".to_string(), "b".to_string()];
    let len = process_data(&my_data);
    println!("{:?}", my_data); // Still available
}
```

**How to find:**
```bash
rg "fn.*\(.*Vec<|fn.*\(.*String[^&]" --type rust
# Find functions taking Vec or String without &
```

---

### 2. Borrowing in Loop

**Problem:**
```rust
// Does not compile
fn update_items(items: &mut Vec<Item>) {
    for item in items {  // item - mutable reference to element
        if item.should_remove() {
            items.retain(|i| i.id != item.id);  // Trying to take another mutable reference!
        }
    }
}
```

**Correct:**
```rust
// Use retain with predicate
fn update_items(items: &mut Vec<Item>) {
    items.retain(|item| !item.should_remove());
}

// Or collect indices separately
fn update_items_alt(items: &mut Vec<Item>) {
    let to_remove: Vec<usize> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            if item.should_remove() {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    for &i in to_remove.iter().rev() {  // From end so indices don't shift
        items.remove(i);
    }
}
```

---

### 3. Clone Instead of Copy

**Problem:**
```rust
#[derive(Debug, Clone)]  // Clone for simple struct
struct Point {
    x: i32,
    y: i32,
}

fn distance(p1: &Point, p2: &Point) -> f64 {
    let dx = (p2.x - p1.x) as f64;
    let dy = (p2.y - p1.y) as f64;
    (dx * dx + dy * dy).sqrt()
}

fn main() {
    let p1 = Point { x: 0, y: 0 };
    let p2 = p1.clone();  // Unnecessary explicit clone
}
```

**Correct:**
```rust
#[derive(Debug, Clone, Copy)]  // Copy for simple types
struct Point {
    x: i32,
    y: i32,
}

fn main() {
    let p1 = Point { x: 0, y: 0 };
    let p2 = p1;  // Automatically copied
    println!("{:?} {:?}", p1, p2);  // Both available
}
```

**Rule:** If all struct fields are Copy, the struct should be Copy too.

---

## Lifetimes

### 1. Redundant Lifetime Annotations

**Problem:**
```rust
// Redundant - compiler will infer
fn first_word<'a>(s: &'a str) -> &'a str {
    s.split_whitespace().next().unwrap_or("")
}
```

**Correct:**
```rust
// Lifetime elision works automatically
fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}
```

**When explicit lifetimes are needed:**
```rust
// Here it's necessary - two input references
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

---

### 2. 'static Everywhere

**Problem:**
```rust
// Unnecessary 'static requirement
fn store_callback(callback: Box<dyn Fn() + 'static>) {
    // ...
}

// Can't pass closure capturing local variables!
```

**Correct:**
```rust
// Accept any lifetime
fn store_callback<F: Fn() + 'a>(callback: F) {
    // ...
}

// Or if 'static is really needed, justify in comment:
/// Callback must be 'static because it's stored in global state
fn store_global_callback(callback: Box<dyn Fn() + 'static>) {
    // ...
}
```

---

## Panic vs Result

### 1. unwrap() and expect() in Production

**Problem:**
```rust
// CRITICAL - can crash in production
async fn handle_request(req: Request) -> Response {
    let user_id = req.headers()
        .get("user-id")
        .unwrap()  // Panic if no header!
        .to_str()
        .unwrap(); // Another panic!

    let user = db.get_user(user_id).await.unwrap();  // And another!

    Response::ok(user)
}
```

**How to find:**
```bash
# Find all unwrap/expect
rg "\.unwrap\(\)|\.expect\(" --type rust --glob '!tests'

# Excluding tests
```

**Correct:**
```rust
// Proper error handling
async fn handle_request(req: Request) -> Result<Response, Error> {
    let user_id = req.headers()
        .get("user-id")
        .ok_or(Error::MissingHeader("user-id"))?
        .to_str()
        .map_err(|_| Error::InvalidHeader("user-id"))?;

    let user = db.get_user(user_id).await
        .map_err(Error::Database)?;

    Ok(Response::ok(user))
}
```

---

### 2. When unwrap() Is Acceptable

**In tests:**
```rust
#[test]
fn test_parsing() {
    let data = parse("valid_data").unwrap();  // OK in tests
    assert_eq!(data.field, "value");
}
```

**After check:**
```rust
fn process(opt: Option<String>) -> String {
    if opt.is_some() {
        opt.unwrap()  // OK - we just checked
    } else {
        String::new()
    }
}

// But better:
fn process_better(opt: Option<String>) -> String {
    opt.unwrap_or_default()  // Idiomatic
}
```

**In constants:**
```rust
const DEFAULT_PORT: u16 = "8080".parse().unwrap();  // OK - checked at compile-time
```

---

## Unsafe Code

### 1. Unjustified Unsafe

**Problem:**
```rust
// Unsafe WITHOUT necessity
fn get_first<T>(slice: &[T]) -> Option<&T> {
    if slice.is_empty() {
        None
    } else {
        unsafe {
            Some(slice.get_unchecked(0))  // Why unsafe?
        }
    }
}
```

**Correct:**
```rust
// Safe version
fn get_first<T>(slice: &[T]) -> Option<&T> {
    slice.first()  // Already in standard library!
}
```

---

### 2. Unsafe Without Documentation

**Problem:**
```rust
// Unsafe without explanations
unsafe fn do_something(ptr: *const u8, len: usize) {
    // What's guaranteed? Why is it safe?
}
```

**Correct:**
```rust
// With detailed explanation
/// # Safety
///
/// Caller must ensure that:
/// - `ptr` is valid and properly aligned
/// - `ptr` points to at least `len` initialized bytes
/// - `ptr` remains valid for the duration of this call
/// - No other threads access the memory during this call
unsafe fn read_bytes(ptr: *const u8, len: usize) -> Vec<u8> {
    // SAFETY: Caller guarantees ptr validity and len correctness
    std::slice::from_raw_parts(ptr, len).to_vec()
}
```

---

### 3. Unsafe Code Checklist

Before using `unsafe`:

- [ ] Can it be done without unsafe? (95% of cases - YES)
- [ ] Are invariants documented in `# Safety`?
- [ ] Are there tests (including miri)?
- [ ] Are all edge cases checked?
- [ ] Is there UB (undefined behavior)?

**Test with Miri:**
```bash
cargo +nightly miri test
```

---

## Traits and Generics

### 1. Excessive Trait Bounds

**Problem:**
```rust
// Too many requirements
fn print_items<T: Debug + Display + Clone + Send + Sync>(items: &[T]) {
    for item in items {
        println!("{:?}", item);  // Only using Debug!
    }
}
```

**Correct:**
```rust
// Minimal requirements
fn print_items<T: Debug>(items: &[T]) {
    for item in items {
        println!("{:?}", item);
    }
}
```

---

### 2. impl Trait vs dyn Trait

**When to use:**

```rust
// impl Trait - return concrete type (known at compile-time)
fn get_iterator(data: Vec<i32>) -> impl Iterator<Item = i32> {
    data.into_iter().filter(|&x| x > 0)
}

// Pros: zero-cost, monomorphization
// Cons: can't return different types

// dyn Trait - return trait object (chosen at runtime)
fn get_shape(shape_type: &str) -> Box<dyn Shape> {
    match shape_type {
        "circle" => Box::new(Circle { radius: 10 }),
        "square" => Box::new(Square { side: 5 }),
        _ => Box::new(Circle { radius: 1 }),
    }
}

// Pros: flexibility, can return different types
// Cons: dynamic dispatch (slower), requires allocation
```

---

### 3. AsRef and Borrow for Generalization

**Good pattern:**
```rust
use std::path::Path;

// Accepts &str, String, &Path, and PathBuf
fn read_file<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::read_to_string(path.as_ref())
}

fn main() {
    read_file("file.txt")?;               // &str
    read_file(String::from("file.txt"))?; // String
    read_file(Path::new("file.txt"))?;    // &Path
    read_file(PathBuf::from("file.txt"))?;// PathBuf
}
```

---

## Rust-Specific Patterns

### 1. NewType Pattern

**For type safety:**
```rust
// Easy to confuse
fn transfer_money(from: i64, to: i64, amount: i64) -> Result<()> {
    // transfer_money(amount, from, to) - error is invisible!
}

// NewType makes confusion impossible
struct UserId(i64);
struct Amount(i64);

fn transfer_money(from: UserId, to: UserId, amount: Amount) -> Result<()> {
    // transfer_money(amount, from, to) - compile error!
}
```

---

### 2. Builder Pattern

```rust
// For structs with many optional fields
#[derive(Default)]
struct ServerConfig {
    host: String,
    port: u16,
    timeout: Duration,
    tls: bool,
}

impl ServerConfig {
    fn builder() -> ServerConfigBuilder {
        ServerConfigBuilder::default()
    }
}

#[derive(Default)]
struct ServerConfigBuilder {
    host: Option<String>,
    port: Option<u16>,
    timeout: Option<Duration>,
    tls: bool,
}

impl ServerConfigBuilder {
    fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    fn build(self) -> Result<ServerConfig> {
        Ok(ServerConfig {
            host: self.host.ok_or("host is required")?,
            port: self.port.unwrap_or(8080),
            timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
            tls: self.tls,
        })
    }
}

// Usage:
let config = ServerConfig::builder()
    .host("localhost")
    .port(3000)
    .build()?;
```

---

### 3. Phantom Types

**For compile-time guarantees:**
```rust
use std::marker::PhantomData;

struct Sealed;
struct Open;

// File with state in type
struct File<State> {
    inner: std::fs::File,
    _state: PhantomData<State>,
}

impl File<Open> {
    fn new(path: &str) -> Result<Self> {
        Ok(File {
            inner: std::fs::File::open(path)?,
            _state: PhantomData,
        })
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        // Can only write to open file
        Ok(())
    }

    fn seal(self) -> File<Sealed> {
        File {
            inner: self.inner,
            _state: PhantomData,
        }
    }
}

impl File<Sealed> {
    // Cannot modify sealed file
    // No write() method!
}

// file.write() // OK
// file.seal().write() // Compile error!
```

---

## Rust Code Review Checklist

### Ownership
- [ ] No unnecessary .clone()?
- [ ] Functions take references where possible?
- [ ] Copy types marked as Copy?
- [ ] No fighting borrow checker with hacks?

### Error Handling
- [ ] Result used instead of panic?
- [ ] No unwrap/expect in production code?
- [ ] Errors handled correctly?
- [ ] ? operator used?

### Unsafe
- [ ] Unsafe really necessary?
- [ ] # Safety documentation present?
- [ ] Tested with miri?

### Generics & Traits
- [ ] Minimal trait bounds?
- [ ] Correct choice of impl/dyn Trait?
- [ ] AsRef/Borrow for flexibility?

### Patterns
- [ ] NewType for important IDs/values?
- [ ] Builder for complex constructors?
