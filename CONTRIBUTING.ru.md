# Участие в разработке <PROJECT_NAME>

[English version](CONTRIBUTING.md)

Спасибо за интерес к участию в проекте!

## Стиль кода и стандарты

Этот проект следует стандартам кодирования [RustManifest](https://github.com/RAprogramm/RustManifest). Пожалуйста, ознакомьтесь с ними перед началом работы.

Ключевые моменты:
- Используйте `cargo +nightly fmt` для форматирования
- Никаких `unwrap()` или `expect()` в продакшен коде
- Документация только через Rustdoc (без инлайн комментариев)
- Описательные имена переменных и функций

## Настройка окружения

### Требования

- Rust nightly toolchain
- cargo-make (опционально, для автоматизации задач)
- cargo-nextest (для запуска тестов)

### Установка

```bash
git clone https://github.com/RAprogramm/<PROJECT_NAME>
cd <PROJECT_NAME>

# Установка nightly toolchain
rustup toolchain install nightly
rustup component add rustfmt --toolchain nightly
rustup component add clippy

# Установка test runner (опционально, но рекомендуется)
cargo install cargo-nextest
```

### Pre-commit проверки

Перед коммитом убедитесь, что все проверки проходят:

```bash
# Проверка форматирования
cargo +nightly fmt --all -- --check

# Линтинг
cargo clippy --all-targets --all-features -- -D warnings

# Тесты
cargo test --all-features

# Или с nextest
cargo nextest run --all-features
```

## Git Workflow

### Именование веток

Используйте номер issue как имя ветки:
```
123
```

### Сообщения коммитов

Формат: `#<номер_issue> <тип>: <описание>`

```
#123 feat: add new output format
#123 fix: correct line counting in parser
#45 docs: update API examples
#78 test: add property tests for extractor
#90 refactor: simplify config loading
```

Типы:
- `feat` - новая функциональность
- `fix` - исправление бага
- `docs` - документация
- `test` - тесты
- `refactor` - рефакторинг кода
- `chore` - служебные задачи

### Pull Requests

1. Создайте ветку от `main`
2. Внесите изменения
3. Убедитесь, что все CI проверки проходят
4. Создайте PR с описательным заголовком
5. Включите `Closes #<issue>` в описание

### Держите изменения продакшен кода маленькими

**Качество код-ревью падает с размером.** Важен именно *продакшен код*, а не общее количество строк.

| Продакшен код | Качество ревью | Риск |
|---------------|----------------|------|
| < 100 строк | Тщательное | Низкий |
| 100-300 строк | Умеренное | Средний |
| 300+ строк | Поверхностное | Высокий |

**Тесты и бенчмарки не считаются так же:**

- PR с 50 строками в `src/` и 1000 строками тестов — **легко ревьюить**
- PR с 300 строками в `src/` и 0 тестов — **сложно ревьюить и рискованно**

Используйте [rust-prod-diff-checker](https://github.com/RAprogramm/rust-prod-diff-checker) GitHub Action для автоматического анализа размера PR и разделения продакшен изменений от тестов/бенчмарков.

**Правило:** Если ревьюер не может понять ваши изменения продакшен кода за 15 минут — PR слишком большой.

## Тестирование

### Написание тестов

- Покрывайте все публичные API функции
- Тестируйте пути ошибок, не только happy path
- Используйте property-based тестирование для парсеров
- Никаких `unwrap()` в тестах — используйте `?` с правильными типами ошибок

```rust
#[test]
fn test_example() -> Result<(), Box<dyn std::error::Error>> {
    let result = some_function()?;
    assert_eq!(result, expected);
    Ok(())
}
```

### Запуск тестов

```bash
# Все тесты
cargo test --all-features

# С покрытием
cargo llvm-cov nextest --all-features

# Конкретный тест
cargo test test_name
```

## CI/CD Pipeline

Каждый PR запускает:

| Задача | Описание |
|--------|----------|
| Format | `cargo +nightly fmt --check` |
| Clippy | `cargo clippy -D warnings` |
| Test | `cargo test --all-features` |
| Doc | `cargo doc --no-deps` |
| Coverage | Загрузка в Codecov |

## Документация

Все публичные элементы должны иметь Rustdoc:

```rust
/// Краткое описание того, что делает функция.
///
/// # Errors
///
/// Возвращает `Error` если что-то пошло не так.
///
/// # Examples
///
/// ```
/// use my_crate::my_function;
///
/// let result = my_function()?;
/// # Ok::<(), my_crate::Error>(())
/// ```
pub fn my_function() -> Result<(), Error> {
    // ...
}
```

## Получение помощи

- Открывайте issue для багов или запросов функциональности
- Проверяйте существующие issues перед созданием новых
- Предоставляйте минимальное воспроизведение для багов

## Лицензия

Участвуя в проекте, вы соглашаетесь, что ваши contributions будут лицензированы под MIT License.
