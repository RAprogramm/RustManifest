# Качество кода и поддерживаемость

## Оглавление
1. [Дублирование кода (DRY)](#дублирование-кода-dry)
2. [Читаемость](#читаемость)
3. [Документация](#документация)
4. [Тестирование](#тестирование)
5. [Архитектурные принципы](#архитектурные-принципы)

---

## Дублирование кода (DRY)

### Don't Repeat Yourself - правило трех

**Правило:** Если код повторяется **3+ раза** → выноси в функцию.

### Пример из проекта (до рефакторинга)

```rust
// ❌ ДУБЛИРОВАНИЕ - один и тот же код 3 раза
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
    // ... ТОЧНО ТАКОЙ ЖЕ КОД ...
}

#[test]
fn test_case_3() {
    // ... И ЕЩЕ РАЗ ...
}
```

**После рефакторинга:**
```rust
// ✅ Функция используется везде
fn compute_telegram_hash(parsed: &BTreeMap<String, String>, bot_token: &str) -> String {
    // Код в одном месте
}

#[test]
fn test_case_1() {
    let hash = compute_telegram_hash(&parsed, bot_token);  // Одна строка!
}

#[test]
fn test_case_2() {
    let hash = compute_telegram_hash(&parsed, bot_token);
}
```

**Выигрыш:**
- Код **3 раза короче**
- Изменения в **одном месте**
- Меньше вероятность ошибок

---

### Как найти дублирование

**1. Визуально:**
- Прокрути файл, ищи похожие блоки
- Обрати внимание на копипасту

**2. Автоматически:**
```bash
# cargo-geiger для поиска дублей
cargo install tokei
tokei --files

# Или cpd (copy-paste detector)
```

**3. По паттернам:**
```bash
# Одинаковые цепочки вызовов
rg "\.iter\(\)\.filter.*\.map.*\.collect" --type rust
```

---

### Исключения из правила DRY

**Не выноси в функцию если:**
1. **Код повторяется < 3 раз**
2. **Контекст разный** (похожий синтаксис, разная семантика)
3. **Делает код менее читаемым**

Пример:
```rust
// ✅ OK - хоть и похоже, но семантика разная
user.validate_email()?;
admin.validate_email()?;
// Не надо делать validate_any_email(user_or_admin)
```

---

## Читаемость

### 1. Именование

**Плохие имена:**
```rust
// ❌ Неясно что это
fn proc(d: &str) -> i32 { ... }
fn handle(x: Vec<u8>) { ... }
fn do_it(s: &State) { ... }

// ❌ Слишком общие
fn manager() { ... }
fn process_data() { ... }
fn helper() { ... }
```

**Хорошие имена:**
```rust
// ✅ Понятно что делает
fn parse_user_id(raw_id: &str) -> Result<i32> { ... }
fn validate_telegram_data(init_data: &str) -> bool { ... }
fn compute_hmac_signature(message: &[u8]) -> String { ... }
```

**Правила именования:**
- Функции: **глагол** + что делает (`validate_data`, `compute_hash`)
- Переменные: **существительное** (`user_id`, `config`, `timestamp`)
- Bool переменные: **is_/has_/should_** (`is_valid`, `has_permission`)
- Константы: **SCREAMING_SNAKE_CASE** (`MAX_RETRIES`, `DEFAULT_TIMEOUT`)

---

### 2. Magic numbers и magic strings

**Проблема:**
```rust
// ❌ Что означает 86400? Что такое "hash"?
fn validate(raw: &str) -> bool {
    let age = get_age(raw);
    if age > 86400 { return false; }  // ❌ Что это за число?

    let data = parse(raw);
    data.get("hash").is_some()  // ❌ Почему "hash"?
}
```

**Решение:**
```rust
// ✅ Константы с именами
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

**Как найти:**
```bash
# Числа в коде
rg "\b[0-9]{4,}\b" --type rust  # Числа из 4+ цифр

# Строковые литералы
rg '"\w+"' --type rust
```

---

### 3. Длина функций

**Правило:** Функция должна помещаться на экран (~40-50 строк)

**Проблема:**
```rust
// ❌ 200 строк - невозможно понять
fn handle_request(req: Request) -> Result<Response> {
    // 200 строк кода...
    // Что-то парсит...
    // Что-то валидирует...
    // Что-то сохраняет...
    // Что-то отправляет...
    // ...
}
```

**Решение:**
```rust
// ✅ Разбито на понятные части
fn handle_request(req: Request) -> Result<Response> {
    let data = parse_request(&req)?;
    validate_data(&data)?;
    let result = process_data(data)?;
    save_result(&result)?;
    Ok(build_response(result))
}
```

**Single Responsibility Principle**: Одна функция = одна задача.

---

### 4. Комментарии

**Когда нужны:**
```rust
// ✅ Объясняет ПОЧЕМУ, не ЧТО
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

**Когда НЕ нужны:**
```rust
// ❌ Комментарий дублирует код
// Increment counter by 1
counter += 1;

// ❌ Очевидные вещи
// Get user from database
let user = db.get_user(id);

// ✅ ЛУЧШЕ переименовать для ясности
let authenticated_user = db.get_user(id);
```

**По AI Development Protocol:**
- ❌ Обычные комментарии ЗАПРЕЩЕНЫ
- ✅ Только `///` doc-комментарии для публичного API

---

## Документация

### 1. Doc-комментарии для публичного API

**Обязательно документируй:**
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

**Что включить:**
- Что делает функция
- Аргументы (и их формат, если не очевидно)
- Возвращаемое значение
- Пример использования (автоматически тестируется!)
- Особые случаи (# Panics, # Errors, # Safety)

---

### 2. Примеры как тесты (doctests)

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

**Проверка:**
```bash
cargo test --doc
```

Если пример не компилируется или assertion fails → тест упадет!

---

### 3. README.md для модулей

**Для крупных модулей:**
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

## Тестирование

### 1. Структура тестов

**Принцип AAA: Arrange, Act, Assert**

```rust
#[test]
fn test_auth_date_expired() {
    // Arrange - подготовка данных
    let bot_token = "test_token";
    let old_timestamp = 1640995200;
    let raw = format!("auth_date={}&user=...", old_timestamp);

    // Act - выполнение действия
    let result = validate_telegram_data(&raw, "hash", bot_token, 86400, false);

    // Assert - проверка результата
    assert!(!result, "Should reject expired auth_date");
}
```

---

### 2. Покрытие edge cases

**Тестируй:**
- ✅ Нормальный случай (happy path)
- ✅ Пограничные значения (0, MAX, -1)
- ✅ Невалидные данные
- ✅ Пустые значения
- ✅ Все ветки if/match

**Пример:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_case() {
        // Обычный случай
    }

    #[test]
    fn test_empty_input() {
        // Пустая строка
    }

    #[test]
    fn test_missing_required_field() {
        // Нет обязательного поля
    }

    #[test]
    fn test_invalid_format() {
        // Неправильный формат
    }

    #[test]
    fn test_boundary_values() {
        // 0, MAX_VALUE, MIN_VALUE
    }
}
```

---

### 3. Property-based testing

**Для генерации множества тест-кейсов:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_parse_never_panics(s in "\\PC*") {
        // Для ЛЮБОЙ строки парсер не должен паниковать
        let _ = parse_data(&s);
    }

    #[test]
    fn test_hash_is_deterministic(data in prop::collection::vec(any::<u8>(), 0..1000)) {
        // Хэш одинаковых данных всегда одинаковый
        let hash1 = compute_hash(&data);
        let hash2 = compute_hash(&data);
        prop_assert_eq!(hash1, hash2);
    }
}
```

---

### 4. Integration tests

**Структура:**
```
project/
├── src/
│   └── lib.rs
└── tests/              # Integration tests
    ├── auth_tests.rs
    ├── api_tests.rs
    └── common/         # Общие helpers
        └── mod.rs
```

**Пример:**
```rust
// tests/auth_tests.rs
use my_crate::*;

#[test]
fn test_full_auth_flow() {
    // Тестируем полный flow от начала до конца
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

## Архитектурные принципы

### 1. Single Responsibility Principle (SRP)

**Каждый модуль/функция делает ОДНУ вещь:**

```rust
// ❌ Слишком много ответственности
fn handle_user_action(user_id: i64, action: &str, data: &str) -> Result<()> {
    // 1. Парсит данные
    // 2. Валидирует
    // 3. Обновляет БД
    // 4. Отправляет email
    // 5. Логирует
    // 6. Обновляет кеш
    // ...
}

// ✅ Разделено по ответственности
mod parser { /* Парсинг */ }
mod validator { /* Валидация */ }
mod repository { /* БД */ }
mod notifier { /* Email */ }
```

---

### 2. Dependency Injection

**Тестируемость и гибкость:**

```rust
// ❌ Жестко привязано к конкретной БД
fn get_user(id: i64) -> Result<User> {
    let conn = PostgresConnection::new()?;  // Не можем подменить в тестах
    conn.query("SELECT * FROM users WHERE id = $1", &[&id])
}

// ✅ Принимаем абстракцию
trait UserRepository {
    fn get_user(&self, id: i64) -> Result<User>;
}

fn get_user<R: UserRepository>(repo: &R, id: i64) -> Result<User> {
    repo.get_user(id)  // Можем подменить mock в тестах
}
```

---

### 3. Модульность

**Структура проекта:**
```
src/
├── handlers/       # HTTP handlers (один файл = один endpoint)
│   ├── auth.rs
│   ├── users.rs
│   └── profiles.rs
├── models/         # Структуры данных
│   └── user.rs
├── repository/     # Работа с БД
│   └── users.rs
├── services/       # Бизнес-логика
│   └── auth_service.rs
├── utils/          # Утилиты
│   └── telegram.rs
└── main.rs         # Entry point
```

**Правило:** Каждый модуль решает ОДНУ задачу.

---

## Чеклист качества кода

### Читаемость
- [ ] Имена функций и переменных понятны?
- [ ] Нет magic numbers и strings?
- [ ] Функции < 50 строк?
- [ ] Нет глубокой вложенности (< 3-4 уровня)?

### DRY
- [ ] Нет дублирования кода?
- [ ] Общая логика вынесена в функции?
- [ ] Используются константы?

### Документация
- [ ] Публичный API задокументирован?
- [ ] Есть примеры использования?
- [ ] Объяснены сложные моменты?

### Тесты
- [ ] Покрыт happy path?
- [ ] Покрыты edge cases?
- [ ] Тесты на ошибки?
- [ ] Integration tests для критичной логики?

### Архитектура
- [ ] Модули разделены по ответственности?
- [ ] Зависимости явные (не глобальные)?
- [ ] Легко тестируется?
- [ ] Легко расширять?

---

## Метрики качества

### Цели для production кода:
- **Покрытие тестами**: ≥ 95% (AI Protocol требует 100%)
- **Cyclomatic complexity**: ≤ 10 на функцию
- **Длина функции**: ≤ 50 строк
- **Вложенность**: ≤ 4 уровня
- **Clippy warnings**: 0

### Инструменты:
```bash
# Покрытие
cargo tarpaulin --out Xml

# Сложность
cargo install cargo-geiger
cargo geiger

# Метрики
cargo install tokei
tokei

# Качество
cargo clippy -- -D warnings
```
