# Поиск уязвимостей безопасности

## Оглавление
1. [Аутентификация и авторизация](#аутентификация-и-авторизация)
2. [Криптография](#криптография)
3. [Инъекции](#инъекции)
4. [Валидация данных](#валидация-данных)
5. [Утечки информации](#утечки-информации)
6. [Атаки на доступность](#атаки-на-доступность)

---

## Аутентификация и авторизация

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

## Валидация данных

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
