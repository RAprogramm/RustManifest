<h1>Rust Code Style Guide</h1>

<p align="right">
  <a href="./README.md">English</a> | <strong>Русский</strong>
</p>

<h2>1. Форматирование</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Форматируйте весь Rust код с помощью <code>nightly rustfmt</code>.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> >
> > <pre><code>cargo +nightly fmt --</code></pre>
> >
> >
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > <p>Единообразное форматирование улучшает читаемость, уменьшает конфликты при слиянии и упрощает код-ревью. Это гарантирует, что код каждого члена команды соответствует единому стандарту.</p>
> > >
> > > <details>
> > > <summary><strong>Примеры и пояснения</strong></summary>
> > >
> > > > <p>Например, хорошо отформатированная кодовая база позволяет новым членам команды быстро понять структуру и логику проекта. Автоматическое форматирование экономит время и минимизирует стилистические споры при код-ревью.</p>
> > > </details>
> >
> > </details>
>
> </details>

<h3>1.1 Конфигурация .rustfmt.toml</h3>

> [!TIP]
>
> <p>
>   <strong>Используйте следующую конфигурацию <code>.rustfmt.toml</code> для обеспечения единообразного форматирования в проекте.</strong>
> </p>
>
> <details>
> <summary><strong>Конфигурация</strong></summary>
>
> ```toml
> # Не добавлять запятую, если только один элемент
> trailing_comma = "Never"
>
> # Держать скобки на той же строке где возможно
> brace_style = "SameLineWhere"
>
> # Выравнивать поля структуры, если их длина ниже порога
> struct_field_align_threshold = 20
>
> # Форматировать комментарии внутри документации
> wrap_comments = true
> format_code_in_doc_comments = true
>
> # Не сворачивать литералы структур в одну строку
> struct_lit_single_line = false
>
> # Максимальная ширина строки
> max_width = 99
>
> # Группировка импортов
> imports_granularity = "Crate"          # Группировать импорты по крейту
> group_imports = "StdExternalCrate"     # Разделять группы: std, внешние крейты, локальные
> reorder_imports = true                 # Сортировать импорты внутри групп
>
> # Включить нестабильные функции (только nightly)
> unstable_features = true
> ```
>
> </details>
>
> <details>
> <summary><strong>Почему это важно?</strong></summary>
>
> <p>
> Эта конфигурация обеспечивает ясность и консистентность. Она уменьшает ненужные различия в pull request'ах, упрощает код-ревью и гарантирует предсказуемость стиля и читаемости для всей команды.
> </p>
>
> <details>
> <summary><strong>Примеры и пояснения</strong></summary>
>
> ### Пример без конфигурации
>
> ```rust
> use std::fmt; use std::io; use serde::Serialize;
>
> struct Person {name:String,age:u32}
>
> impl Person{
>     pub fn new(name:String,age:u32)->Self{
>         Self{name,age}
>     }
> }
> ```
>
> ### Пример с конфигурацией
>
> ```rust
> use std::{fmt, io};
>
> use serde::Serialize;
>
> struct Person {
>     name: String,
>     age:  u32,
> }
>
> impl Person {
>     pub fn new(name: String, age: u32) -> Self {
>         Self {
>             name,
>             age,
>         }
>     }
> }
> ```
>
> <p>
> Обратите внимание: импорты сгруппированы и отсортированы, поля структуры выровнены для читаемости, скобки расставлены консистентно. Это уменьшает шум в диффах и делает кодовую базу понятной как для новичков, так и для опытных разработчиков.
> </p>
> </details>
>
> </details>



<h2>2. Соглашения об именовании</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Используйте ясные, описательные имена, отражающие назначение. Следуйте snake_case для переменных/функций, PascalCase для типов и SCREAMING_SNAKE_CASE для констант.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > - **Описательные имена:**
> >   - <code>create_user_handler</code> – OK
> >   - <code>create_user_service</code> – OK
> >   - <code>create</code> – NO
> >   - <code>create_user</code> – NO
> >
> > - Следуйте Rust <em>snake_case</em> для переменных и функций.
> > - Используйте <em>PascalCase</em> для структур и енамов (например, <code>TransactionStatus</code>).
> > - Константы должны быть в SCREAMING_SNAKE_CASE.
> >
> >
> > <details>
> > <summary><strong>Почему описательные имена?</strong></summary>
> > <p>
> >   Описательные имена уменьшают неоднозначность, облегчают онбординг и улучшают поддерживаемость. Ясные имена делают очевидным, что делает функция или переменная, избегая недоразумений и конфликтов.
> > </p>
> >
> > > <details>
> > > <summary><strong>Примеры и пояснения</strong></summary>
> > > <p>
> > >   Например, <code>create_user_handler</code> указывает, что функция отвечает за обработку создания пользователя в веб-контексте, тогда как общее имя <code>create</code> не дает контекста.
> > > </p>
> > > </details>
> >
> > </details>
>
> </details>


<h2>3. Качество кода</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Пишите чистый, поддерживаемый код. Избегайте ненужной сложности, паник и клонирования. Минимизируйте глобальное состояние и ограничьте использование <code>::</code> операторами импорта. Не используйте файлы <code>mod.rs</code>.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > - Пишите чистый и поддерживаемый код.
> > - Избегайте ненужной сложности.
> > - **Избегайте ненужных <code>unwrap()</code> и <code>clone()</code>.**
> > - Минимизируйте глобальное состояние и побочные эффекты.
> > - Используйте <code>::</code> только в операторах импорта.
> > - **Не используйте файлы <code>mod.rs</code>.**
> >
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> > <p>
> >   Вместо написания <code>some_option.unwrap()</code>, предпочтите:
> > </p>
> > <pre><code>let value = some_option.ok_or("Expected a value, but found None")?;</code></pre>
> > <p>
> >   Это правильно пробрасывает ошибки и избегает падения приложения. Аналогично, организуйте модули в отдельных файлах <code>module_name.rs</code> вместо устаревших файлов <code>mod.rs</code>, что упрощает структуру проекта и улучшает обнаружение модулей.
> > </p>
> > </details>
>
> </details>

<h2>4. Управление ветками</h2>

> [!NOTE]
>
> <p>
>   <strong>Каждая ветка, коммит и PR должны соответствовать номеру GitHub Issue. Это обеспечивает автоматическую связь, чистую историю и полную отслеживаемость.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > - **Создавайте ветку только с номером Issue:**
> >   Имя ветки должно быть точно номером Issue.
> >   Пример:
> >   ```bash
> >   git checkout -b 123
> >   ```
> >
> > - **Используйте авто-связывание в коммитах:**
> >   Чтобы GitHub автоматически связывал коммиты с Issue, всегда начинайте сообщение коммита с `#` и номера Issue с пробелом.
> >   Пример:
> >   ```bash
> >   #123 implement login session restore
> >   #123 fix null pointer in user handler
> >   ```
> >
> > - **Заголовок Pull Request = Имя ветки:**
> >   Заголовок PR должен совпадать с именем ветки (просто номер Issue).
> >   Пример:
> >   ```
> >   123
> >   ```
> >
> > - **Добавьте ссылку для автозакрытия:**
> >   В описании PR всегда включайте:
> >   ```
> >   Closes #123
> >   ```
> >   Это автоматически закрывает Issue при слиянии PR.
> >
> > - **Очистка после слияния:**
> >   Включите "Delete branch on merge" в настройках репозитория, чтобы слитые ветки автоматически удалялись.
> >   Цепочка Issue -> Branch -> Commits -> PR -> Merge остается полностью связанной.
> >
> > - **Держите репозиторий чистым:**
> >   Каждая ветка должна соответствовать активному Issue.
> >   Никаких осиротевших или экспериментальных веток после слияния.
> >
> >
> > <details>
> > <summary><strong>Реальный пример и пояснение</strong></summary>
> > <p>
> >   Предположим, вам назначен Issue <code>#123</code> для исправления бага сессии логина.
> >   Вы создаете ветку <code>123</code> и начинаете коммитить с сообщениями:
> > </p>
> >
> >   <pre>
> >   #123 implement login session restore
> >   #123 add retry logic for session token refresh
> >   </pre>
> >
> >   Затем открываете PR с заголовком <code>123</code> и описанием:
> >
> >   <pre>
> >   Closes #123
> >   </pre>
> >
> >   Когда PR слит, GitHub автоматически закрывает Issue #123, удаляет ветку и показывает все связанные коммиты в таймлайне Issue.
> >   Это создает идеально отслеживаемый и автоматизированный рабочий процесс с минимумом ручных шагов.
> > </details>
>
> </details>


<h2>5. Лучшие практики</h2>

> [!TIP]
>
> <p>
>   <strong>Следуйте лучшим практикам для поддержания высокого качества кода.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > - Используйте <code>cargo clippy</code> для линтинга.
> > - Обрабатывайте ошибки грациозно с помощью <code>Result</code> и <code>Option</code>.
> > - **Избегайте ненужных паник.**
> >
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> > <p>
> >   Вместо написания:
> > </p>
> > <pre><code>let value = some_option.unwrap();</code></pre>
> > <p>
> >   используйте:
> > </p>
> > <pre><code>let value = some_option.ok_or("Expected a value, but found None")?;</code></pre>
> > <p>
> >   Этот паттерн гарантирует, что ошибки пробрасываются и обрабатываются должным образом, увеличивая надежность вашего приложения.
> > </p>
> > </details>
>
> </details>


<h2>6. Паники в Rust</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Избегайте паник в продакшене; используйте правильную обработку ошибок с <code>Result</code> и оператором <code>?</code>.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > - **Избегайте паник в продакшен коде.**
> > - **Не рекомендуется:** Избегайте <code>unwrap()</code> и <code>expect()</code>, если не уверены абсолютно, что ошибка не может произойти.
> > - **Рекомендуется:** Используйте правильную обработку ошибок с <code>Result</code> и оператором <code>?</code>.
> >
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> > <p>
> >   Например, вместо:
> > </p>
> > <pre><code>let config = Config::from_file("config.toml").unwrap();</code></pre>
> > <p>
> >   используйте:
> > </p>
> > <pre><code>let config = Config::from_file("config.toml")
> >   .map_err(|e| format!("Failed to load config: {}", e))?;
> > </code></pre>
> > <p>
> >   Этот подход логирует детальные сообщения об ошибках и грациозно пробрасывает ошибки вверх по стеку вызовов, приводя к более надежной и поддерживаемой системе.
> > </p>
> > </details>
> >
> > <details>
> > <summary><strong>Реальный инцидент: падение Cloudflare (ноябрь 2025)</strong></summary>
> > <p>
> >   18 ноября 2025 года один вызов <code>.unwrap()</code> в Rust коде вызвал массовый сбой в 330+ дата-центрах Cloudflare. Сервисы ChatGPT, X, Canva и многие другие были недоступны около 3 часов.
> > </p>
> > <p>
> >   Причина: изменение конфигурации привело к тому, что файл фич содержал больше записей, чем ожидалось. Rust код проверял лимит, но использовал <code>unwrap()</code> на пути ошибки вместо грациозной обработки. При превышении лимита код запаниковал с сообщением: <code>"thread fl2_worker_thread panicked: called Result::unwrap() on an Err value"</code>
> > </p>
> > <p>
> >   <strong>Урок:</strong> Этот <code>.unwrap()</code> был в кодовой базе долгое время, но никогда не срабатывал, пока неожиданные входные данные не достигли этого пути в коде. Вот почему продакшен код должен явно обрабатывать все случаи ошибок.
> > </p>
> > <p>
> >   <a href="https://blog.cloudflare.com/18-november-2025-outage/">Читать официальный post-mortem Cloudflare</a>
> > </p>
> > </details>
>
> </details>

<h2>7. Тестирование и CI</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Все коммиты должны проходить pre-commit проверки. Тесты, форматирование, линтинг и сканирование безопасности применяются как локально (через pre-commit хуки), так и удаленно (через CI).</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > - **Pre-commit хуки**
> >   - Устанавливаются через:
> >     <pre><code>cargo make install-hooks</code></pre>
> >   - Автоматически запускаются перед каждым коммитом:
> >     <pre><code>cargo +nightly fmt -- </code></pre>
> >     <pre><code> cargo clippy -D warnings  </code></pre>
> >      <pre><code> cargo test --all</code></pre>
> >   - Предотвращают коммит неотформатированного кода, предупреждений или падающих тестов.
> >
> > - **Unit тесты**
> >   - Покрывают публичные функции и случаи ошибок.
> >   - Тесты не должны полагаться на <code>unwrap()</code> или <code>expect()</code>.
> >
> > - **Интеграционные тесты**
> >   - Покрывают публичный API, размещаются в директории <code>tests/</code>.
> >
> > - **Doctests**
> >   - Все примеры <code>///</code> должны компилироваться и проходить с <code>cargo test --doc</code>.
> >
> > - **Coverage (cargo-llvm-cov + Codecov)**
> >   - Установка: <pre><code>cargo install cargo-llvm-cov</code></pre>
> >   - Локальный запуск: <pre><code>cargo llvm-cov --all-features --workspace --html</code></pre>
> >   - Конфигурация CI:
> >     ```yaml
> >     - name: Install cargo-llvm-cov
> >       uses: taiki-e/install-action@cargo-llvm-cov
> >     - name: Generate code coverage
> >       run: cargo llvm-cov --all-features --workspace --codecov --output-path codecov.json
> >     - name: Upload coverage to Codecov
> >       uses: codecov/codecov-action@v5
> >       with:
> >         token: ${{ secrets.CODECOV_TOKEN }}
> >         files: codecov.json
> >         fail_ci_if_error: true
> >     ```
> >
> > <details>
> > <summary><strong>Почему cargo-llvm-cov + Codecov?</strong></summary>
> >
> > > - **Точность**: LLVM-инструментирование обеспечивает точное покрытие строк и веток, надежнее source-based инструментов
> > > - **Скорость**: Значительно быстрее tarpaulin, особенно на больших кодовых базах с множеством зависимостей
> > > - **Нативный формат**: Прямой вывод в codecov.json без промежуточных шагов конвертации
> > > - **Визуализация**: Дашборд Codecov показывает тренды покрытия, diff покрытия в PR и интерактивные sunburst-диаграммы
> > > - **Интеграция с PR**: Автоматические отчеты о покрытии в комментариях PR, показывающие какие именно строки покрыты/не покрыты
> > > - **Branch protection**: Настройка минимальных порогов покрытия для падения CI при снижении coverage
> > > - **Rust toolchain**: Использует встроенное инструментирование rustc, гарантируя совместимость со всеми фичами Rust
> > </details>
> >
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> >
> > ### Пример Unit теста
> >
> > ```rust
> > #[cfg(test)]
> > mod tests {
> >     use super::*;
> >
> >     #[test]
> >     fn test_basic_math() {
> >         assert_eq!(2 + 2, 4);
> >     }
> > }
> > ```
> >
> > ### Пример интеграционного теста
> >
> > ```rust
> > // tests/config_tests.rs
> > use my_crate::load_config;
> >
> > #[test]
> > fn load_valid_config() {
> >     let result = load_config("tests/data/valid.toml");
> >     assert!(result.is_ok());
> > }
> > ```
> >
> > <p>
> >   Этот рабочий процесс обеспечивает корректность на каждом шаге: разработчики не могут закоммитить сломанный код, а CI гарантирует, что ничего не проскользнет при слиянии.
> > </p>
> > </details>
>
> </details>

<h2>8. Документация кода (без инлайн комментариев)</h2>

> [!NOTE]
>
> <p>
>   <strong>Никаких инлайн комментариев в коде. Все пояснения находятся в doc-блоках, прикрепленных к модулям, структурам, енамам, трейтам, функциям и методам.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > - **Никаких строчных комментариев в коде:**
> >   Избегайте `// ...` и `/* ... */` для объяснения поведения, намерений или инвариантов. Держите код чистым и самоочевидным.
> >
> > - **Используйте Rust doc комментарии консистентно:**
> >   - Крейт/модуль: используйте <code>//!</code> в начале <code>lib.rs</code> или файлов модулей для документации уровня модуля.
> >   - Элементы (структуры, енамы, трейты, функции, методы): используйте <code>///</code> на элементе.
> >
> > - **Структурируйте doc-блоки для IDE и LSP:**
> >   Используйте заголовки, которые понимает Rustdoc, чтобы всплывающие подсказки и Treesitter outlines были стабильными и информативными:
> >   - <code># Overview</code> краткое назначение
> >   - <code># Examples</code> минимальные, компилируемые примеры
> >   - <code># Errors</code> точные режимы сбоя для <code>Result</code>
> >   - <code># Panics</code> только если неизбежно (должно быть редко)
> >   - <code># Safety</code> если используется <code>unsafe</code> (не должно быть)
> >   - <code># Performance</code> если важна сложность или аллокации
> >
> > - **Пишите для других инженеров:**
> >   Будьте явными о контрактах, входах, выходах, инвариантах и граничных случаях. Держите примеры запускаемыми. Предпочитайте ясность изощренности.
> >
> > - **Держите документацию рядом с кодом:**
> >   Обновляйте doc-блоки вместе с изменениями кода в том же PR. Устаревшая документация хуже, чем её отсутствие.
> >
> >
> > <details>
> > <summary><strong>Правильно vs Неправильно (Rust)</strong></summary>
> >
> > **Неправильно (инлайн комментарии, которые не появятся во всплывающих подсказках):**
> >
> > ```rust
> > // Calculates checksum and validates header
> > // Returns Err if invalid
> > pub fn verify(pkt: &Packet) -> Result<(), VerifyError> {
> >     // fast path
> >     if pkt.header.len() < MIN {
> >         return Err(VerifyError::TooShort);
> >     }
> >     // slow path...
> >     Ok(())
> > }
> > ```
> >
> > **Правильно (doc-блоки; IDE hover показывает контракт):**
> >
> > ```rust
> > /// # Overview
> > /// Verifies packet header and payload consistency.
> > ///
> > /// # Examples
> > /// ```
> > /// # use mynet::{Packet, verify};
> > /// # fn demo(mut p: Packet) {
> > /// #   // prepare p...
> > /// #   let _ = verify(&p).unwrap();
> > /// # }
> > /// ```
> > ///
> > /// # Errors
> > /// - `VerifyError::TooShort` when header is smaller than the required minimum.
> > /// - `VerifyError::ChecksumMismatch` when computed checksum differs.
> > pub fn verify(pkt: &Packet) -> Result<(), VerifyError> {
> >     if pkt.header.len() < MIN {
> >         return Err(VerifyError::TooShort);
> >     }
> >     // internal micro-notes for maintainers are allowed if they aid refactoring
> >     // (but not to explain business logic). Keep them brief.
> >     Ok(())
> > }
> > ```
> >
> > **Документация уровня модуля вместо баннера комментариев:**
> >
> > ```rust
> > //! Cryptographic key management and signing primitives.
> > //!
> > //! Provides deterministic ECDSA with explicit domain separation.
> > //!
> > //! # Examples
> > //! ```
> > //! # use keys::{Keypair, Signer};
> > //! # fn demo() {
> > //! #   let kp = Keypair::generate();
> > //! #   let sig = kp.sign(b"payload");
> > //! #   assert!(kp.verify(b"payload", &sig).is_ok());
> > //! # }
> > //! ```
> > pub mod crypto { /* ... */ }
> > ```
> > </details>
> >
> > <details>
> > <summary><strong>Реальное обоснование</strong></summary>
> > <p>
> >   Эта политика обеспечивает стабильные всплывающие подсказки IDE/LSP, лучшие Treesitter outlines и надежную навигацию. Инженеры сразу видят контракты, CI может линтить документацию, а примеры остаются компилируемыми. Код остается чистым, а документация — обнаруживаемой и точной.
> > </p>
> > </details>
>
> </details>


<h2>9. Методология Code Review</h2>

> [!TIP]
>
> <p>
>   <strong>Используйте комплексную методологию код-ревью для систематического поиска уязвимостей, проблем производительности и качества.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > **Доступно на двух языках:**
> > - [English Documentation](./code-review-methodology/en/INDEX.md)
> > - [Русская документация](./code-review-methodology/ru/INDEX.md)
> >
> > **Быстрые ссылки:**
> >
> > | Тема | EN | RU |
> > |------|----|----|
> > | Шпаргалка | [Cheat Sheet](./code-review-methodology/en/quick-reference.md) | [Шпаргалка](./code-review-methodology/ru/quick-reference.md) |
> > | Безопасность | [Vulnerabilities](./code-review-methodology/en/security-vulnerabilities.md) | [Уязвимости](./code-review-methodology/ru/security-vulnerabilities.md) |
> > | Производительность | [Issues](./code-review-methodology/en/performance-issues.md) | [Проблемы](./code-review-methodology/ru/performance-issues.md) |
> > | Качество кода | [Quality](./code-review-methodology/en/code-quality.md) | [Качество](./code-review-methodology/ru/code-quality.md) |
> > | Rust паттерны | [Specifics](./code-review-methodology/en/rust-specific.md) | [Специфика](./code-review-methodology/ru/rust-specific.md) |
> > | Примеры | [Real Cases](./code-review-methodology/en/examples.md) | [Примеры](./code-review-methodology/ru/examples.md) |
> >
> >
> > <details>
> > <summary><strong>Что покрыто</strong></summary>
> >
> > > **Уязвимости безопасности:**
> > > - Replay атаки и обход аутентификации
> > > - SQL/Command инъекции
> > > - Утечки секретов и проблемы криптографии
> > > - Проблемы валидации входных данных
> > >
> > > **Проблемы производительности:**
> > > - Неэффективные аллокации и ненужное клонирование
> > > - O(n^2) алгоритмы где возможен O(n)
> > > - Дублирование операций и двойной парсинг
> > > - Блокирующие операции в async коде
> > >
> > > **Качество кода:**
> > > - Нарушения DRY и дублирование кода
> > > - Именование и читаемость
> > > - Стандарты документации
> > > - Покрытие тестами
> > >
> > > **Rust-специфика:**
> > > - Паттерны ownership и borrowing
> > > - Обработка Panic vs Result
> > > - Ревью unsafe кода
> > > - Trait bounds и generics
> > </details>
> >
> > <details>
> > <summary><strong>Быстрый 5-минутный чеклист</strong></summary>
> >
> > **Безопасность (2 мин):**
> > - [ ] Нет секретов в коде
> > - [ ] Нет `unwrap()`/`expect()` в продакшене
> > - [ ] Входные данные валидируются
> > - [ ] Нет SQL/Command инъекций
> >
> > **Производительность (1 мин):**
> > - [ ] Нет очевидных O(n^2)
> > - [ ] Нет дублирования операций
> > - [ ] `Vec::with_capacity()` где нужно
> >
> > **Качество (2 мин):**
> > - [ ] Нет дублирования кода (> 3 раз)
> > - [ ] Функции < 50 строк
> > - [ ] Тесты для новой логики
> > </details>
>
> </details>


<p align=center>
  <em>Следование этим рекомендациям гарантирует, что наш Rust код будет высококачественным, поддерживаемым и масштабируемым.</em>
</p>
