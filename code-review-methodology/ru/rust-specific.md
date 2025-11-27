# Rust-специфичные проблемы и особенности

## Оглавление
1. [Ownership и Borrowing](#ownership-и-borrowing)
2. [Lifetimes](#lifetimes)
3. [Panic vs Result](#panic-vs-result)
4. [Unsafe код](#unsafe-код)
5. [Traits и generics](#traits-и-generics)

---

## Ownership и Borrowing

### 1. Unnecessary ownership transfer

**Проблема:**
```rust
// ❌ Забирает ownership без причины
fn process_data(data: Vec<String>) -> usize {
    data.len()  // Можно было просто посмотреть длину!
}

fn main() {
    let my_data = vec!["a".to_string(), "b".to_string()];
    let len = process_data(my_data);
    // my_data больше недоступен! ❌
    // println!("{:?}", my_data); // Ошибка компиляции
}
```

**Правильно:**
```rust
// ✅ Принимает ссылку
fn process_data(data: &[String]) -> usize {
    data.len()
}

fn main() {
    let my_data = vec!["a".to_string(), "b".to_string()];
    let len = process_data(&my_data);
    println!("{:?}", my_data); // ✅ Все еще можем использовать
}
```

**Как найти:**
```bash
rg "fn.*\(.*Vec<|fn.*\(.*String[^&]" --type rust
# Ищем функции, принимающие Vec или String без &
```

---

### 2. Borrowing в цикле

**Проблема:**
```rust
// ❌ Не компилируется
fn update_items(items: &mut Vec<Item>) {
    for item in items {  // item - изменяемая ссылка на элемент
        if item.should_remove() {
            items.retain(|i| i.id != item.id);  // ❌ Пытаемся взять еще одну изменяемую ссылку!
        }
    }
}
```

**Правильно:**
```rust
// ✅ Используем retain с предикатом
fn update_items(items: &mut Vec<Item>) {
    items.retain(|item| !item.should_remove());
}

// Или собираем индексы отдельно
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

    for &i in to_remove.iter().rev() {  // С конца, чтобы индексы не сдвигались
        items.remove(i);
    }
}
```

---

### 3. Clone вместо Copy

**Проблема:**
```rust
#[derive(Debug, Clone)]  // ❌ Clone для простой структуры
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
    let p2 = p1.clone();  // ❌ Лишняя явная клонация
}
```

**Правильно:**
```rust
#[derive(Debug, Clone, Copy)]  // ✅ Copy для simple types
struct Point {
    x: i32,
    y: i32,
}

fn main() {
    let p1 = Point { x: 0, y: 0 };
    let p2 = p1;  // ✅ Автоматически копируется
    println!("{:?} {:?}", p1, p2);  // Оба доступны
}
```

**Правило:** Если все поля структуры - Copy, то и структура должна быть Copy.

---

## Lifetimes

### 1. Избыточные lifetime аннотации

**Проблема:**
```rust
// ❌ Избыточно - компилятор сам выведет
fn first_word<'a>(s: &'a str) -> &'a str {
    s.split_whitespace().next().unwrap_or("")
}
```

**Правильно:**
```rust
// ✅ Lifetime elision работает автоматически
fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}
```

**Когда нужны явные lifetimes:**
```rust
// ✅ Здесь необходимо - две входные ссылки
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

---

### 2. 'static везде

**Проблема:**
```rust
// ❌ Лишнее требование 'static
fn store_callback(callback: Box<dyn Fn() + 'static>) {
    // ...
}

// Не сможем передать замыкание, захватывающее локальные переменные!
```

**Правильно:**
```rust
// ✅ Принимаем любое время жизни
fn store_callback<F: Fn() + 'a>(callback: F) {
    // ...
}

// Или если действительно нужен 'static, обоснуй в комментарии:
/// Callback must be 'static because it's stored in global state
fn store_global_callback(callback: Box<dyn Fn() + 'static>) {
    // ...
}
```

---

## Panic vs Result

### 1. unwrap() и expect() в production

**Проблема:**
```rust
// ❌ КРИТИЧНО - может упасть в production
async fn handle_request(req: Request) -> Response {
    let user_id = req.headers()
        .get("user-id")
        .unwrap()  // ❌ Panic если нет заголовка!
        .to_str()
        .unwrap(); // ❌ Еще один panic!

    let user = db.get_user(user_id).await.unwrap();  // ❌ И еще!

    Response::ok(user)
}
```

**Как найти:**
```bash
# Найди все unwrap/expect
rg "\.unwrap\(\)|\.expect\(" --type rust --glob '!tests'

# Исключая тесты
```

**Правильно:**
```rust
// ✅ Правильная обработка ошибок
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

### 2. Когда unwrap() допустим

**✅ В тестах:**
```rust
#[test]
fn test_parsing() {
    let data = parse("valid_data").unwrap();  // OK в тестах
    assert_eq!(data.field, "value");
}
```

**✅ После проверки:**
```rust
fn process(opt: Option<String>) -> String {
    if opt.is_some() {
        opt.unwrap()  // OK - мы только что проверили
    } else {
        String::new()
    }
}

// Но лучше так:
fn process_better(opt: Option<String>) -> String {
    opt.unwrap_or_default()  // Идиоматично
}
```

**✅ В константах:**
```rust
const DEFAULT_PORT: u16 = "8080".parse().unwrap();  // OK - проверяется на compile-time
```

---

## Unsafe код

### 1. Необоснованный unsafe

**Проблема:**
```rust
// ❌ Unsafe БЕЗ необходимости
fn get_first<T>(slice: &[T]) -> Option<&T> {
    if slice.is_empty() {
        None
    } else {
        unsafe {
            Some(slice.get_unchecked(0))  // ❌ Зачем unsafe?
        }
    }
}
```

**Правильно:**
```rust
// ✅ Безопасная версия
fn get_first<T>(slice: &[T]) -> Option<&T> {
    slice.first()  // Уже есть в стандартной библиотеке!
}
```

---

### 2. Unsafe без документации

**Проблема:**
```rust
// ❌ Unsafe без объяснений
unsafe fn do_something(ptr: *const u8, len: usize) {
    // Что гарантируется? Почему безопасно?
}
```

**Правильно:**
```rust
// ✅ С детальным объяснением
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

### 3. Чеклист для unsafe кода

Перед использованием `unsafe`:

- [ ] Можно ли без unsafe? (95% случаев - ДА)
- [ ] Документированы ли инварианты в `# Safety`?
- [ ] Есть ли тесты (включая miri)?
- [ ] Проверены ли все edge cases?
- [ ] Нет ли UB (undefined behavior)?

**Тест с Miri:**
```bash
cargo +nightly miri test
```

---

## Traits и Generics

### 1. Избыточные trait bounds

**Проблема:**
```rust
// ❌ Лишние требования
fn print_items<T: Debug + Display + Clone + Send + Sync>(items: &[T]) {
    for item in items {
        println!("{:?}", item);  // Используем только Debug!
    }
}
```

**Правильно:**
```rust
// ✅ Минимальные требования
fn print_items<T: Debug>(items: &[T]) {
    for item in items {
        println!("{:?}", item);
    }
}
```

---

### 2. impl Trait vs dyn Trait

**Когда использовать:**

```rust
// ✅ impl Trait - возвращаем конкретный тип (известен в compile-time)
fn get_iterator(data: Vec<i32>) -> impl Iterator<Item = i32> {
    data.into_iter().filter(|&x| x > 0)
}

// Плюсы: zero-cost, monomorphization
// Минусы: не можем вернуть разные типы

// ✅ dyn Trait - возвращаем trait object (выбирается в runtime)
fn get_shape(shape_type: &str) -> Box<dyn Shape> {
    match shape_type {
        "circle" => Box::new(Circle { radius: 10 }),
        "square" => Box::new(Square { side: 5 }),
        _ => Box::new(Circle { radius: 1 }),
    }
}

// Плюсы: гибкость, можем вернуть разные типы
// Минусы: динамическая диспетчеризация (медленнее), требует аллокации
```

---

### 3. AsRef и Borrow для обобщения

**Хороший паттерн:**
```rust
use std::path::Path;

// ✅ Принимает и &str, и String, и &Path, и PathBuf
fn read_file<P: AsRef<Path>>(path: P) -> Result<String> {
    std::fs::read_to_string(path.as_ref())
}

fn main() {
    read_file("file.txt")?;              // &str
    read_file(String::from("file.txt"))?; // String
    read_file(Path::new("file.txt"))?;    // &Path
    read_file(PathBuf::from("file.txt"))?;// PathBuf
}
```

---

## Специфичные для Rust паттерны

### 1. NewType Pattern

**Для типобезопасности:**
```rust
// ❌ Легко перепутать
fn transfer_money(from: i64, to: i64, amount: i64) -> Result<()> {
    // transfer_money(amount, from, to) - ошибка незаметна!
}

// ✅ NewType делает невозможным перепутать
struct UserId(i64);
struct Amount(i64);

fn transfer_money(from: UserId, to: UserId, amount: Amount) -> Result<()> {
    // transfer_money(amount, from, to) - ошибка компиляции!
}
```

---

### 2. Builder Pattern

```rust
// ✅ Для структур с множеством опциональных полей
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

// Использование:
let config = ServerConfig::builder()
    .host("localhost")
    .port(3000)
    .build()?;
```

---

### 3. Phantom Types

**Для compile-time гарантий:**
```rust
use std::marker::PhantomData;

struct Sealed;
struct Open;

// Файл с состоянием в типе
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
        // Можем писать только в открытый файл
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
    // Нельзя изменить запечатанный файл
    // Методы write() нет!
}

// file.write() // OK
// file.seal().write() // Ошибка компиляции!
```

---

## Чеклист для Rust code review

### Ownership
- [ ] Нет лишних .clone()?
- [ ] Функции принимают ссылки где возможно?
- [ ] Copy типы помечены как Copy?
- [ ] Нет борьбы с borrow checker через костыли?

### Error Handling
- [ ] Result используется вместо panic?
- [ ] Нет unwrap/expect в production коде?
- [ ] Ошибки обрабатываются корректно?
- [ ] Используется ? operator?

### Unsafe
- [ ] Unsafe действительно необходим?
- [ ] Есть документация # Safety?
- [ ] Протестировано с miri?

### Generics & Traits
- [ ] Минимальные trait bounds?
- [ ] Правильный выбор impl/dyn Trait?
- [ ] AsRef/Borrow для гибкости?

### Patterns
- [ ] NewType для важных ID/значений?
- [ ] Builder для сложных конструкторов?
