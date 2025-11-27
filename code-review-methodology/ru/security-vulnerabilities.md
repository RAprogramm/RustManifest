# Поиск уязвимостей безопасности

<p align="right"><a href="#оглавление">Наверх</a></p>

## Оглавление
1. [Аутентификация и авторизация](#аутентификация-и-авторизация)
2. [Криптография](#криптография)
3. [Инъекции](#инъекции)
4. [Безопасность баз данных](#безопасность-баз-данных)
5. [Валидация данных](#валидация-данных)
6. [Утечки информации](#утечки-информации)
7. [Атаки на доступность](#атаки-на-доступность)

---

## Аутентификация и авторизация

<p align="right"><a href="#оглавление">Наверх</a></p>

### 1. Replay Attack (Атака повторного воспроизведения)

**Что искать:**
- Отсутствие проверки временных меток (timestamp)
- Отсутствие nonce (одноразовых токенов)
- Бесконечное время жизни токенов

**Пример уязвимости:**
```rust
// ❌ УЯЗВИМО - нет проверки свежести данных
fn validate_telegram_data(raw: &str, hash: &str, bot_token: &str) -> bool {
    let computed_hash = compute_hmac(raw, bot_token);
    computed_hash == hash  // Хэш правильный, но данные могут быть старыми!
}
```

**Как эксплуатируется:**
1. Злоумышленник перехватывает валидный запрос (например, через MITM)
2. Сохраняет данные аутентификации
3. Воспроизводит их позже для получения доступа

**Правильное решение:**
```rust
// ✅ БЕЗОПАСНО
fn validate_telegram_data(
    raw: &str,
    hash: &str,
    bot_token: &str,
    max_age: u64
) -> bool {
    let parsed = parse_data(raw);

    // 1. Проверяем свежесть данных
    if let Some(auth_date) = parsed.get("auth_date") {
        let timestamp = auth_date.parse::<u64>()?;
        let now = current_timestamp();

        if now - timestamp > max_age {
            return false; // Данные устарели
        }
    } else {
        return false; // Нет временной метки
    }

    // 2. Проверяем подпись
    let computed_hash = compute_hmac(raw, bot_token);
    computed_hash == hash
}
```

**Где искать:**
- Аутентификация по токенам
- API endpoints с подписями
- OAuth flows
- Webhook handlers

**Ключевые слова для поиска:**
```bash
rg "validate|auth|verify" --type rust
rg "timestamp|auth_date|created_at" --type rust
```

---

### 2. JWT без проверки expiration

**Что искать:**
```rust
// ❌ УЯЗВИМО
fn decode_jwt(token: &str) -> Result<Claims> {
    jsonwebtoken::decode(
        token,
        &key,
        &Validation::default()  // Не проверяет exp!
    )
}
```

**Правильно:**
```rust
// ✅ БЕЗОПАСНО
fn decode_jwt(token: &str) -> Result<Claims> {
    let mut validation = Validation::default();
    validation.validate_exp = true;  // Проверяем expiration
    validation.leeway = 60;          // 60 секунд погрешности

    jsonwebtoken::decode(token, &key, &validation)
}
```

---

### 3. Отсутствие rate limiting

**Что искать:**
- Endpoints аутентификации без ограничений
- Возможность brute-force атак

**Пример уязвимости:**
```rust
// ❌ УЯЗВИМО - можно перебирать пароли
async fn login(username: &str, password: &str) -> Result<Token> {
    let user = db.find_user(username).await?;

    if verify_password(password, &user.password_hash) {
        Ok(generate_token(user.id))
    } else {
        Err(Error::InvalidCredentials)
    }
}
```

**Правильно:**
```rust
// ✅ БЕЗОПАСНО
async fn login(username: &str, password: &str, ip: IpAddr) -> Result<Token> {
    // 1. Проверяем rate limit
    if !rate_limiter.check(ip, 5, Duration::from_secs(60)) {
        return Err(Error::TooManyRequests);
    }

    let user = db.find_user(username).await?;

    // 2. Используем constant-time comparison
    if verify_password(password, &user.password_hash) {
        Ok(generate_token(user.id))
    } else {
        // 3. Учитываем попытку
        rate_limiter.increment(ip);

        // 4. Добавляем задержку перед ответом (против timing attacks)
        tokio::time::sleep(Duration::from_millis(500)).await;

        Err(Error::InvalidCredentials)
    }
}
```

---

## Криптография

<p align="right"><a href="#оглавление">Наверх</a></p>

### 1. Использование слабых алгоритмов

**Что искать:**
```bash
rg "md5|sha1|DES|RC4" --type rust  # Слабые алгоритмы
```

**Примеры:**
```rust
// ❌ ПЛОХО - MD5 криптографически сломан
use md5::Md5;
let hash = Md5::digest(password);

// ❌ ПЛОХО - SHA1 тоже
use sha1::Sha1;
let hash = Sha1::digest(token);

// ✅ ХОРОШО - используй современные алгоритмы
use sha2::Sha256;
let hash = Sha256::digest(data);

// ✅ ЕЩЁ ЛУЧШЕ - для паролей используй специализированные функции
use argon2::{Argon2, PasswordHasher};
let hash = Argon2::default().hash_password(password, &salt)?;
```

---

### 2. Хардкод ключей и секретов

**Что искать:**
```bash
# Поиск потенциальных секретов
rg "password.*=.*\"" --type rust
rg "secret.*=.*\"" --type rust
rg "api_key.*=.*\"" --type rust
rg "token.*=.*\"" --type rust
rg "private_key" --type rust
```

**Пример уязвимости:**
```rust
// ❌ КРИТИЧЕСКАЯ УЯЗВИМОСТЬ
const API_KEY: &str = "sk_live_51H...";  // Никогда!
const DB_PASSWORD: &str = "admin123";    // Катастрофа!

fn encrypt(data: &[u8]) -> Vec<u8> {
    let key = b"mysecretkey12345";  // ❌ Хардкод ключа
    aes_encrypt(data, key)
}
```

**Правильно:**
```rust
// ✅ ПРАВИЛЬНО - из переменных окружения
fn get_api_key() -> Result<String> {
    std::env::var("API_KEY")
        .map_err(|_| Error::MissingApiKey)
}

fn encrypt(data: &[u8]) -> Result<Vec<u8>> {
    // Ключ из безопасного хранилища
    let key = get_encryption_key()?;
    aes_encrypt(data, &key)
}
```

---

### 3. Неправильное использование HMAC

**Проблема из нашего кода:**
```rust
// ❌ БЫЛО - может вызвать panic
let mut mac = HmacSha256::new_from_slice(&secret)
    .expect("HMAC initialization failed");
```

**Почему плохо:**
- `expect()` может вызвать panic
- В production panic = DOS атака

**Правильно:**
```rust
// ✅ СТАЛО
let mac = HmacSha256::new_from_slice(&secret).ok()?;
```

---

## Инъекции

<p align="right"><a href="#оглавление">Наверх</a></p>

### 1. SQL Injection

**Что искать:**
```bash
rg "format!.*SELECT|query.*format!" --type rust
rg "execute.*&format!" --type rust
```

**Пример уязвимости:**
```rust
// ❌ SQL INJECTION!
async fn get_user(username: &str) -> Result<User> {
    let query = format!("SELECT * FROM users WHERE username = '{}'", username);
    //                                                                ^^^^^^^^
    // Если username = "admin' OR '1'='1", то получим всех пользователей!

    db.execute(&query).await
}
```

**Правильно:**
```rust
// ✅ БЕЗОПАСНО - используем параметризованные запросы
async fn get_user(username: &str) -> Result<User> {
    sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE username = $1",
        username  // Автоматически экранируется
    )
    .fetch_one(&db)
    .await
}
```

---

### 2. Command Injection

**Что искать:**
```bash
rg "Command::new|std::process::Command" --type rust
```

**Пример уязвимости:**
```rust
// ❌ COMMAND INJECTION!
fn resize_image(filename: &str) -> Result<()> {
    let cmd = format!("convert {} -resize 100x100 output.jpg", filename);
    //                           ^^^^^^^^
    // filename = "input.jpg; rm -rf /" приведет к удалению файлов!

    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()?;
    Ok(())
}
```

**Правильно:**
```rust
// ✅ БЕЗОПАСНО
fn resize_image(filename: &str) -> Result<()> {
    // 1. Валидируем имя файла
    if !is_valid_filename(filename) {
        return Err(Error::InvalidFilename);
    }

    // 2. Не используем shell - передаем аргументы напрямую
    std::process::Command::new("convert")
        .arg(filename)        // Безопасно - не интерпретируется shell'ом
        .arg("-resize")
        .arg("100x100")
        .arg("output.jpg")
        .output()?;
    Ok(())
}

fn is_valid_filename(name: &str) -> bool {
    // Только буквы, цифры, точки, дефисы
    name.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
}
```

---

## Безопасность баз данных

<p align="right"><a href="#оглавление">Наверх</a></p>

> **Автоматический анализ:** Используйте [sql-query-analyzer](https://github.com/RAprogramm/sql-query-analyzer) для статического анализа SQL-запросов с 18 встроенными правилами безопасности и производительности.

### Статический анализ с sql-query-analyzer

**Установка:**
```bash
cargo install sql-query-analyzer
```

**Локальное использование:**
```bash
sql-query-analyzer analyze -s schema.sql -q queries.sql --format text
```

**Интеграция с GitHub Actions:**
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

### Правила безопасности

#### SEC001: UPDATE без WHERE

**Критичность:** Error

**Что искать:**
```bash
rg "UPDATE.*SET" --type rust
rg "UPDATE.*SET" --type sql
```

**Пример уязвимости:**
```sql
-- ❌ ОПАСНО: затрагивает ВСЕ строки таблицы
UPDATE users SET status = 'inactive';
```

**Правильно:**
```sql
-- ✅ БЕЗОПАСНО: явное условие
UPDATE users SET status = 'inactive' WHERE last_login < '2024-01-01';
```

---

#### SEC002: DELETE без WHERE

**Критичность:** Error

**Что искать:**
```bash
rg "DELETE FROM" --type rust
rg "DELETE FROM" --type sql
```

**Пример уязвимости:**
```sql
-- ❌ ОПАСНО: удаляет ВСЕ строки
DELETE FROM sessions;
```

**Правильно:**
```sql
-- ✅ БЕЗОПАСНО: явное условие
DELETE FROM sessions WHERE expired_at < NOW();
```

---

### Правила производительности (влияние на безопасность)

#### PERF001: SELECT * без LIMIT

**Критичность:** Warning

**Влияние на безопасность:** Может вызвать DOS через исчерпание памяти

**Пример уязвимости:**
```sql
-- ❌ МОЖЕТ ВЫЗВАТЬ OOM на больших таблицах
SELECT * FROM logs;
```

**Правильно:**
```sql
-- ✅ БЕЗОПАСНО: ограниченный набор результатов
SELECT * FROM logs LIMIT 1000;
-- ИЛИ: конкретные колонки
SELECT id, message, created_at FROM logs LIMIT 1000;
```

---

#### PERF002: Ведущий wildcard в LIKE

**Критичность:** Warning

**Влияние на безопасность:** Предотвращает использование индекса, позволяет DOS через медленные запросы

**Пример уязвимости:**
```sql
-- ❌ МЕДЛЕННО: полное сканирование таблицы, индекс не используется
SELECT * FROM users WHERE email LIKE '%@gmail.com';
```

**Правильно:**
```sql
-- ✅ ЛУЧШЕ: используйте колонку суффикса или полнотекстовый поиск
SELECT * FROM users WHERE email_domain = 'gmail.com';
```

---

#### PERF004: Большие значения OFFSET

**Критичность:** Warning

**Влияние на безопасность:** Деградация производительности, вектор DOS

**Пример уязвимости:**
```sql
-- ❌ МЕДЛЕННО: база должна просканировать и пропустить 100000 строк
SELECT * FROM products ORDER BY id LIMIT 10 OFFSET 100000;
```

**Правильно:**
```sql
-- ✅ БЫСТРО: keyset пагинация
SELECT * FROM products WHERE id > 100000 ORDER BY id LIMIT 10;
```

---

#### PERF005: Декартово произведение

**Критичность:** Error

**Влияние на безопасность:** Экспоненциальный размер результата, исчерпание памяти

**Пример уязвимости:**
```sql
-- ❌ ОПАСНО: возвращает rows_a * rows_b результатов
SELECT * FROM users, orders;
```

**Правильно:**
```sql
-- ✅ БЕЗОПАСНО: явное условие соединения
SELECT * FROM users
JOIN orders ON users.id = orders.user_id;
```

---

#### PERF007: Скалярный подзапрос в SELECT

**Критичность:** Warning

**Влияние на безопасность:** Паттерн N+1 запросов, перегрузка базы данных

**Пример уязвимости:**
```sql
-- ❌ МЕДЛЕННО: подзапрос выполняется для КАЖДОЙ строки
SELECT
    id,
    name,
    (SELECT COUNT(*) FROM orders WHERE user_id = users.id) as order_count
FROM users;
```

**Правильно:**
```sql
-- ✅ БЫСТРО: один запрос с JOIN
SELECT u.id, u.name, COUNT(o.id) as order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name;
```

---

#### PERF008: Функция на индексированной колонке

**Критичность:** Warning

**Влияние на безопасность:** Обход индекса, медленные запросы

**Пример уязвимости:**
```sql
-- ❌ МЕДЛЕННО: UPPER() предотвращает использование индекса
SELECT * FROM users WHERE UPPER(email) = 'TEST@EXAMPLE.COM';
```

**Правильно:**
```sql
-- ✅ БЫСТРО: храните нормализованные данные или используйте функциональный индекс
SELECT * FROM users WHERE email_lower = 'test@example.com';
```

---

#### PERF009: NOT IN с подзапросом

**Критичность:** Warning

**Влияние на безопасность:** Неожиданное поведение с NULL, проблемы производительности

**Пример уязвимости:**
```sql
-- ❌ ОПАСНО: возвращает пустой результат если подзапрос содержит NULL
SELECT * FROM users WHERE id NOT IN (SELECT user_id FROM banned);
```

**Правильно:**
```sql
-- ✅ БЕЗОПАСНО: корректно обрабатывает NULL
SELECT * FROM users u
WHERE NOT EXISTS (SELECT 1 FROM banned b WHERE b.user_id = u.id);
```

---

#### PERF011: SELECT без WHERE

**Критичность:** Info

**Влияние на безопасность:** Полное сканирование таблицы

**Пример уязвимости:**
```sql
-- ❌ МЕДЛЕННО: сканирует всю таблицу
SELECT * FROM audit_logs;
```

**Правильно:**
```sql
-- ✅ ЛУЧШЕ: добавьте фильтрацию или пагинацию
SELECT * FROM audit_logs
WHERE created_at > NOW() - INTERVAL '7 days'
LIMIT 1000;
```

---

### Правила с учётом схемы

#### SCHEMA001: Отсутствие индекса на колонке фильтра

**Критичность:** Warning

**Что искать:** Колонки в WHERE/JOIN без индексов

**Пример:**
```sql
-- Если колонка 'status' не имеет индекса, это медленно
SELECT * FROM orders WHERE status = 'pending';
```

**Исправление:** Добавьте индекс в схему:
```sql
CREATE INDEX idx_orders_status ON orders(status);
```

---

#### SCHEMA002: Колонка отсутствует в схеме

**Критичность:** Warning

**Что искать:** Ссылки на несуществующие колонки

**Пример:**
```sql
-- Если 'user_name' не существует (должно быть 'username')
SELECT user_name FROM users;
```

---

### Чеклист безопасности баз данных

#### Безопасность запросов
- [ ] Нет `UPDATE` без условия `WHERE`?
- [ ] Нет `DELETE` без условия `WHERE`?
- [ ] Все запросы используют параметризованные выражения?
- [ ] Нет конкатенации строк в SQL?
- [ ] `LIMIT` применяется к неограниченным запросам?

#### Производительность и защита от DOS
- [ ] Нет ведущих wildcards в паттернах `LIKE`?
- [ ] Keyset пагинация вместо большого `OFFSET`?
- [ ] Нет декартовых произведений (отсутствующие условия JOIN)?
- [ ] Нет N+1 запросов (скалярные подзапросы в SELECT)?
- [ ] Индексы существуют для фильтруемых колонок?

#### Целостность данных
- [ ] Внешние ключи определены и применяются?
- [ ] Ограничения валидируют данные на уровне базы?
- [ ] Транзакции используются для многошаговых операций?

### Инструменты автоматического анализа

```bash
# Установка sql-query-analyzer
cargo install sql-query-analyzer

# Анализ запросов по схеме
sql-query-analyzer analyze -s schema.sql -q queries.sql

# Вывод в SARIF для интеграции с CI
sql-query-analyzer analyze -s schema.sql -q queries.sql --format sarif

# Отключение конкретных правил при необходимости
sql-query-analyzer analyze -s schema.sql -q queries.sql --disabled-rules PERF003,STYLE001
```

**GitHub Actions с загрузкой SARIF:**
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

## Валидация данных

<p align="right"><a href="#оглавление">Наверх</a></p>

### 1. Отсутствие валидации входных данных

**Что искать в API handlers:**
```rust
// ❌ НЕТ ВАЛИДАЦИИ
#[derive(Deserialize)]
pub struct CreateUser {
    pub email: String,      // Может быть "не-email"
    pub age: i32,          // Может быть -100
    pub username: String,  // Может быть пустым или 10000 символов
}

async fn create_user(Json(data): Json<CreateUser>) -> Result<Json<User>> {
    // Прямо создаем без проверок!
    let user = db.create_user(data).await?;
    Ok(Json(user))
}
```

**Правильно:**
```rust
// ✅ С ВАЛИДАЦИЕЙ
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
    // 1. Валидируем
    data.validate()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // 2. Дополнительная бизнес-валидация
    if db.username_exists(&data.username).await? {
        return Err(Error::UsernameTaken);
    }

    // 3. Создаем
    let user = db.create_user(data).await?;
    Ok(Json(user))
}
```

---

### 2. Integer Overflow

**Пример из реального кода:**
```rust
// ❌ ПОТЕНЦИАЛЬНАЯ ПРОБЛЕМА
fn calculate_total(price: u64, quantity: u32) -> u64 {
    price * quantity as u64  // Может переполниться!
    // Если price = u64::MAX и quantity = 2, получим wraparound
}
```

**Правильно:**
```rust
// ✅ БЕЗОПАСНО
fn calculate_total(price: u64, quantity: u32) -> Result<u64> {
    price.checked_mul(quantity as u64)
        .ok_or(Error::Overflow)
}

// Или в debug/test билдах будет panic:
#[cfg(debug_assertions)]
fn calculate_total_debug(price: u64, quantity: u32) -> u64 {
    price * quantity as u64  // В debug - panic при overflow
}
```

---

## Утечки информации

<p align="right"><a href="#оглавление">Наверх</a></p>

### 1. Логирование секретов

**Что искать:**
```bash
rg "tracing::.*password|log.*token|debug.*secret" --type rust
```

**Пример уязвимости:**
```rust
// ❌ УТЕЧКА СЕКРЕТОВ В ЛОГИ
async fn login(username: &str, password: &str) -> Result<Token> {
    tracing::info!("Login attempt: {} with password {}", username, password);
    //                                                                ^^^^^^^^
    // Пароль попадет в логи!

    // ...
}
```

**Правильно:**
```rust
// ✅ БЕЗОПАСНО
async fn login(username: &str, password: &str) -> Result<Token> {
    tracing::info!("Login attempt for user: {}", username);
    // Пароль НЕ логируем никогда!

    // Для дебага можно логировать хэш (но лучше не надо)
    tracing::debug!("Password hash: {}", hash_for_debug_only(password));

    // ...
}
```

---

### 2. Раскрытие внутренних ошибок

**Пример уязвимости:**
```rust
// ❌ РАСКРЫВАЕТ СТРУКТУРУ БД
async fn get_user(id: Uuid) -> Result<Json<User>, (StatusCode, String)> {
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
        .fetch_one(&db)
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e)  // ❌ Детали БД клиенту!
        ))?;

    Ok(Json(user))
}
```

**Правильно:**
```rust
// ✅ БЕЗОПАСНО
async fn get_user(id: Uuid) -> Result<Json<User>, StatusCode> {
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
        .fetch_one(&db)
        .await
        .map_err(|e| {
            // Детально логируем для разработчиков
            tracing::error!("Failed to fetch user {}: {}", id, e);

            // Клиенту - только общее сообщение
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(user))
}
```

---

## Чеклист безопасности

<p align="right"><a href="#оглавление">Наверх</a></p>

### Аутентификация
- [ ] Проверяется свежесть токенов/временных меток?
- [ ] Используется rate limiting?
- [ ] JWT токены имеют expiration?
- [ ] Проверяется подпись/HMAC?
- [ ] Нет hardcoded credentials?

### Криптография
- [ ] Используются современные алгоритмы (SHA-256+, AES-256)?
- [ ] Нет хардкод ключей и секретов?
- [ ] Правильная обработка ошибок (нет panic)?
- [ ] Для паролей используется Argon2/bcrypt?

### Инъекции
- [ ] SQL запросы параметризованы?
- [ ] Нет использования format! в SQL?
- [ ] Command execution безопасен?
- [ ] Валидируются имена файлов/пути?

### Валидация
- [ ] Все входные данные валидируются?
- [ ] Проверяются диапазоны чисел?
- [ ] Email/URL валидируются?
- [ ] Длина строк ограничена?

### Утечки информации
- [ ] Секреты не логируются?
- [ ] Детали ошибок не раскрываются клиенту?
- [ ] Debug info только в development?

## Инструменты для автоматической проверки

```bash
# Clippy с безопасностью
cargo clippy -- -W clippy::all -W clippy::pedantic -W clippy::nursery

# Проверка зависимостей
cargo audit

# Поиск секретов
rg "(?i)(password|secret|api_key|token).*=.*['\"]" --type rust
```
