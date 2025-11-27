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


<h2>10. Docker и CI кэширование для Rust</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Используйте cargo-chef для кэширования слоёв Docker и registry cache для CI. Это кардинально сокращает время сборки при неизменных зависимостях.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > **Проблема:**
> > - Компиляция Rust медленная, особенно при большом дереве зависимостей
> > - Docker пересобирает всё при любом изменении файла
> > - `--mount=type=cache` не сохраняется между CI раннерами
> > - Каждый запуск CI начинается с нуля без правильного кэширования
> >
> > **Решение: cargo-chef + Registry Cache**
> >
> > 1. **cargo-chef** отделяет компиляцию зависимостей от компиляции исходников
> > 2. **Registry cache** сохраняет слои Docker между запусками CI
> > 3. Зависимости кэшируются в отдельный слой, который пересобирается только при изменении Cargo.toml/Cargo.lock
> >
> > <details>
> > <summary><strong>Паттерн Dockerfile</strong></summary>
> >
> > ```dockerfile
> > # syntax=docker/dockerfile:1
> > ARG RUST_VERSION=1.83.0
> >
> > # Chef stage - установка cargo-chef
> > FROM rust:${RUST_VERSION} AS chef
> > RUN cargo install cargo-chef --locked
> > WORKDIR /app
> >
> > # Planner - создание recipe только из зависимостей
> > FROM chef AS planner
> > COPY Cargo.toml Cargo.lock ./
> > COPY my-crate/Cargo.toml my-crate/
> > COPY crates crates/
> > RUN cargo chef prepare --recipe-path recipe.json
> >
> > # Builder - сборка зависимостей, затем исходников
> > FROM chef AS builder
> >
> > # Сборка зависимостей (кэшируется если recipe.json не изменился)
> > COPY --from=planner /app/recipe.json recipe.json
> > RUN cargo chef cook --release --recipe-path recipe.json
> >
> > # Сборка приложения (только этот слой пересобирается при изменении кода)
> > COPY . .
> > RUN cargo build --release && strip target/release/my-binary
> >
> > # Runtime - минимальный образ
> > FROM debian:bookworm-slim
> > COPY --from=builder /app/target/release/my-binary /usr/local/bin/
> > CMD ["my-binary"]
> > ```
> >
> > **Ключевые моменты:**
> > - Planner stage копирует только Cargo.toml файлы (не исходный код)
> > - `cargo chef prepare` создаёт recipe.json из зависимостей
> > - `cargo chef cook` компилирует зависимости - этот слой кэшируется
> > - Исходный код копируется после сборки зависимостей
> > - Только финальный `cargo build` перекомпилируется при изменении кода
> > </details>
> >
> > <details>
> > <summary><strong>Паттерн GitHub Actions CI</strong></summary>
> >
> > ```yaml
> > jobs:
> >   build:
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >
> >       - uses: docker/setup-buildx-action@v3
> >
> >       - uses: docker/login-action@v3
> >         with:
> >           registry: ${{ env.REGISTRY }}
> >           username: ${{ secrets.REGISTRY_USERNAME }}
> >           password: ${{ secrets.REGISTRY_TOKEN }}
> >
> >       - name: Build image
> >         uses: docker/build-push-action@v6
> >         with:
> >           context: .
> >           file: ./Dockerfile
> >           push: false
> >           load: true
> >           tags: ${{ env.REGISTRY }}/my-image:${{ env.TAG }}
> >           cache-from: |
> >             type=registry,ref=${{ env.REGISTRY }}/my-image:cache
> >           cache-to: |
> >             type=registry,ref=${{ env.REGISTRY }}/my-image:cache,mode=max
> > ```
> >
> > **Ключевые моменты:**
> > - `cache-from` скачивает закэшированные слои из registry перед сборкой
> > - `cache-to` загружает новые слои кэша после сборки
> > - `mode=max` кэширует все промежуточные слои (не только финальный)
> > - Тег кэша отдельный от тегов образа (например, `:cache`)
> > - Работает на разных CI раннерах и ветках
> > </details>
> >
> > <details>
> > <summary><strong>Чего НЕ делать</strong></summary>
> >
> > **Не используйте `--mount=type=cache` для CI:**
> > ```dockerfile
> > # ПЛОХО - кэш не сохраняется между CI раннерами
> > RUN --mount=type=cache,target=/usr/local/cargo/registry \
> >     cargo build --release
> > ```
> >
> > **Не копируйте все исходники до зависимостей:**
> > ```dockerfile
> > # ПЛОХО - любое изменение файла инвалидирует кэш зависимостей
> > COPY . .
> > RUN cargo build --release
> > ```
> >
> > **Не используйте GHA cache для больших Rust сборок:**
> > ```yaml
> > # ПЛОХО - GHA cache имеет лимит 10GB, target/ Rust легко его превышает
> > - uses: actions/cache@v4
> >   with:
> >     path: target/
> >     key: rust-${{ hashFiles('Cargo.lock') }}
> > ```
> > </details>
> >
> > <details>
> > <summary><strong>Влияние на производительность</strong></summary>
> >
> > | Сценарий | Без кэширования | С cargo-chef + Registry Cache |
> > |----------|----------------|-------------------------------|
> > | Первая сборка | 15-30 мин | 15-30 мин |
> > | Только изменение кода | 15-30 мин | 2-5 мин |
> > | Изменение зависимостей | 15-30 мин | 15-30 мин |
> > | Без изменений | 15-30 мин | 30 сек - 1 мин |
> >
> > Ключевой инсайт: большинство CI запусков меняют только код приложения, не зависимости.
> > С правильным кэшированием такие сборки пропускают 90%+ времени компиляции.
> > </details>
>
> </details>

<h2>11. Продвинутые Quality Gates в CI</h2>

> [!TIP]
>
> <p>
>   <strong>Помимо базовых тестов и линтеров, профессиональные Rust проекты должны включать проверку лицензий, стабильности API, MSRV и аудит зависимостей в CI.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > | Инструмент | Назначение | Когда использовать |
> > |------------|------------|-------------------|
> > | `cargo-deny` | Соответствие лицензий, дублирующиеся зависимости, security advisories | Любой проект с зависимостями |
> > | `cargo-semver-checks` | Обнаружение ломающих изменений API | Библиотеки публикуемые на crates.io |
> > | MSRV check | Проверка минимальной поддерживаемой версии Rust | Проекты с `rust-version` в Cargo.toml |
> > | `cargo-machete` | Поиск неиспользуемых зависимостей | Уменьшение bloat, быстрее сборка |
> > | Doctests | Проверка компилируемости примеров в документации | Проекты с `///` doc комментариями |
> > | [`cargo-quality`](https://github.com/RAprogramm/cargo-quality) | Качество кода с hardcoded стандартами | Zero-config проверка качества |
> > | [`rust-diff-analyzer`](https://github.com/RAprogramm/rust-prod-diff-checker) | Семантический анализ размера PR | Контроль обозримости PR |
> > | [`sql-query-analyzer`](https://github.com/RAprogramm/sql-query-analyzer) | Статический анализ SQL + LLM оптимизация | Проекты с SQL запросами |
> >
> > <details>
> > <summary><strong>cargo-deny: Лицензии и безопасность</strong></summary>
> >
> > **Установка:**
> > ```bash
> > cargo install cargo-deny
> > ```
> >
> > **Конфигурация (`deny.toml`):**
> > ```toml
> > [advisories]
> > db-path = "~/.cargo/advisory-db"
> > vulnerability = "deny"
> > unmaintained = "warn"
> > yanked = "deny"
> >
> > [licenses]
> > allow = ["MIT", "Apache-2.0", "BSD-3-Clause", "ISC", "Zlib"]
> > copyleft = "deny"
> > unlicensed = "deny"
> >
> > [bans]
> > multiple-versions = "warn"
> > wildcards = "deny"
> >
> > [sources]
> > unknown-registry = "deny"
> > unknown-git = "deny"
> > ```
> >
> > **Интеграция в CI:**
> > ```yaml
> > - name: Check licenses and advisories
> >   run: cargo deny check
> > ```
> >
> > **Почему это важно:**
> > - Предотвращает случайные GPL/AGPL зависимости в MIT проектах
> > - Ловит известные уязвимости (RustSec)
> > - Предупреждает о дублирующихся версиях зависимостей (bloat)
> > </details>
> >
> > <details>
> > <summary><strong>cargo-semver-checks: Стабильность API</strong></summary>
> >
> > **Установка:**
> > ```bash
> > cargo install cargo-semver-checks
> > ```
> >
> > **Использование:**
> > ```bash
> > # Сравнение с последней опубликованной версией
> > cargo semver-checks check-release
> >
> > # Сравнение с конкретной версией
> > cargo semver-checks check-release --baseline-version 1.2.0
> > ```
> >
> > **Интеграция в CI:**
> > ```yaml
> > - name: Check semver compliance
> >   if: github.event_name == 'pull_request'
> >   run: |
> >     cargo install cargo-semver-checks
> >     cargo semver-checks check-release
> > ```
> >
> > **Что отлавливает:**
> > - Удаление публичных функций/типов (breaking)
> > - Изменение сигнатур функций (breaking)
> > - Добавление обязательных полей в структуры (breaking)
> > - Изменение вариантов enum (breaking)
> >
> > **Когда использовать:** Любая библиотека публикуемая на crates.io, от API которой зависят пользователи.
> > </details>
> >
> > <details>
> > <summary><strong>MSRV Check: Минимальная поддерживаемая версия Rust</strong></summary>
> >
> > **В Cargo.toml:**
> > ```toml
> > [package]
> > rust-version = "1.83"  # MSRV
> > edition = "2024"
> > ```
> >
> > **Интеграция в CI:**
> > ```yaml
> > jobs:
> >   msrv:
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >       - name: Extract MSRV
> >         id: msrv
> >         run: |
> >           MSRV=$(grep '^rust-version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
> >           echo "version=$MSRV" >> $GITHUB_OUTPUT
> >       - uses: dtolnay/rust-toolchain@master
> >         with:
> >           toolchain: ${{ steps.msrv.outputs.version }}
> >       - run: cargo check --all-features
> > ```
> >
> > **Почему это важно:**
> > - Edition 2024 требует Rust 1.85+
> > - Пользователи на старых версиях Rust получают понятные ошибки
> > - Предотвращает случайное использование новых фич
> > </details>
> >
> > <details>
> > <summary><strong>cargo-machete: Неиспользуемые зависимости</strong></summary>
> >
> > **Установка:**
> > ```bash
> > cargo install cargo-machete
> > ```
> >
> > **Использование:**
> > ```bash
> > cargo machete
> > ```
> >
> > **Интеграция в CI:**
> > ```yaml
> > - name: Check for unused dependencies
> >   run: |
> >     cargo install cargo-machete
> >     cargo machete
> > ```
> >
> > **Преимущества:**
> > - Быстрее время компиляции
> > - Меньше размер бинарника
> > - Уменьшенная поверхность атаки
> > - Чище дерево зависимостей
> > </details>
> >
> > <details>
> > <summary><strong>Doctests: Примеры в документации</strong></summary>
> >
> > **Запуск doctests явно:**
> > ```bash
> > cargo test --doc
> > ```
> >
> > **Интеграция в CI:**
> > ```yaml
> > - name: Run doctests
> >   run: cargo test --doc --all-features
> > ```
> >
> > **Пример doctest:**
> > ```rust
> > /// Вычисляет сумму двух чисел.
> > ///
> > /// # Examples
> > ///
> > /// ```
> > /// use mylib::add;
> > /// assert_eq!(add(2, 3), 5);
> > /// ```
> > pub fn add(a: i32, b: i32) -> i32 {
> >     a + b
> > }
> > ```
> >
> > **Зачем отдельные doctests:**
> > - `cargo test` запускает unit + integration + doc тесты вместе
> > - Doctests часто требуют другие feature flags
> > - Быстрее фидбек когда меняется документация, но не код
> > </details>
> >
> > <details>
> > <summary><strong>cargo-quality: Zero-Config проверка качества</strong></summary>
> >
> > **Проблема:**
> > - Команды разбрасывают `.rustfmt.toml`, `.clippy.toml` по репозиториям
> > - В разных проектах разные стандарты
> > - Новые разработчики не знают какие правила применяются
> >
> > **Решение:**
> > Все стандарты зашиты в один бинарник. Установил один раз — используешь везде.
> >
> > **Установка:**
> > ```bash
> > cargo install cargo-quality
> > ```
> >
> > **Команды:**
> > ```bash
> > cargo qual check src/           # Анализ без изменений
> > cargo qual fix --dry-run        # Предпросмотр исправлений
> > cargo qual fix                  # Применить исправления
> > cargo qual fmt                  # Форматирование (max_width: 99)
> > ```
> >
> > **Четыре анализатора:**
> >
> > | Анализатор | Обнаруживает | Авто-фикс |
> > |------------|--------------|-----------|
> > | `path_import` | Прямые пути модулей, которые должны быть импортами | Да |
> > | `format_args` | Позиционные аргументы в format макросах | Да |
> > | `empty_lines` | Пустые строки в функциях (признак сложности) | Да |
> > | `inline_comments` | Комментарии, которые должны быть doc-блоками | Нет |
> >
> > **Интеграция в CI:**
> > ```yaml
> > - uses: RAprogramm/cargo-quality@v0
> >   with:
> >     path: 'src/'
> >     fail_on_issues: 'true'
> >     post_comment: 'true'
> > ```
> >
> > **Почему cargo-quality:**
> > - Единый источник истины для всех репозиториев
> > - Ловит паттерны, которые rustfmt/clippy пропускают
> > - 86% покрытие тестами
> >
> > **Ссылки:** [GitHub](https://github.com/RAprogramm/cargo-quality) | [crates.io](https://crates.io/crates/cargo-quality) | [docs.rs](https://docs.rs/cargo-quality)
> > </details>
> >
> > <details>
> > <summary><strong>rust-diff-analyzer: Семантический анализ PR</strong></summary>
> >
> > **Проблема:**
> > - Лимиты по количеству строк бессмысленны (500 строк тестов ≠ 500 строк prod)
> > - Большие PR скрывают баги и замедляют ревью
> > - Тестовый код не должен учитываться в размере PR
> >
> > **Решение:**
> > AST-анализ, который понимает семантику Rust кода.
> >
> > **Установка:**
> > ```bash
> > cargo install rust-diff-analyzer
> > ```
> >
> > **Использование:**
> > ```bash
> > git diff main | rust-diff-analyzer
> > rust-diff-analyzer --diff-file changes.diff --max-units 50
> > ```
> >
> > **Система взвешенных баллов:**
> >
> > | Тип единицы | Public | Private |
> > |-------------|--------|---------|
> > | Function | 3 | 1 |
> > | Struct | 3 | 1 |
> > | Trait | 4 | 4 |
> > | Impl Block | 2 | 2 |
> >
> > **Умная классификация:**
> > - `tests/`, `benches/`, `examples/` → тестовый код (исключён)
> > - `#[test]`, `#[cfg(test)]` → тестовый код (исключён)
> > - Всё остальное → production код (учитывается в лимитах)
> >
> > **Интеграция в CI:**
> > ```yaml
> > - uses: RAprogramm/rust-prod-diff-checker@v1
> >   with:
> >     max_prod_units: 30
> >     max_weighted_score: 100
> >     fail_on_exceed: 'true'
> >     post_comment: 'true'
> > ```
> >
> > **Почему семантический анализ:**
> > - 100 строк тестов ≠ 100 строк бизнес-логики
> > - Изменения публичного API требуют больше внимания
> > - Управление размером PR на основе данных
> >
> > **Ссылки:** [GitHub](https://github.com/RAprogramm/rust-prod-diff-checker) | [crates.io](https://crates.io/crates/rust-diff-analyzer)
> > </details>
> >
> > <details>
> > <summary><strong>sql-query-analyzer: Статический анализ SQL</strong></summary>
> >
> > **Проблема:**
> > - SQL баги обнаруживаются в продакшене (отсутствующие индексы, N+1)
> > - Проблемы безопасности (UPDATE без WHERE) проходят через ревью
> > - Нет анализа со знанием схемы в существующих инструментах
> >
> > **Решение:**
> > 18 детерминированных правил + опциональная LLM-оптимизация.
> >
> > **Установка:**
> > ```bash
> > cargo install sql-query-analyzer
> > ```
> >
> > **Использование:**
> > ```bash
> > # Статический анализ (мгновенно, без API ключа)
> > sql-query-analyzer analyze -s schema.sql -q queries.sql
> >
> > # SARIF для GitHub Code Scanning
> > sql-query-analyzer analyze -s schema.sql -q queries.sql -f sarif > results.sarif
> > ```
> >
> > **18 встроенных правил:**
> >
> > | Категория | Правила | Примеры |
> > |-----------|---------|---------|
> > | Performance (11) | PERF001-011 | Unbounded SELECT, leading wildcards, N+1 |
> > | Security (2) | SEC001-002 | UPDATE/DELETE без WHERE |
> > | Style (2) | STYLE001-002 | SELECT *, отсутствующие алиасы |
> > | Schema (3) | SCHEMA001-003 | Отсутствующие индексы, невалидные колонки |
> >
> > **Интеграция в CI:**
> > ```yaml
> > - uses: RAprogramm/sql-query-analyzer@v1
> >   with:
> >     schema: db/schema.sql
> >     queries: db/queries.sql
> >     upload-sarif: 'true'
> >     post-comment: 'true'
> > ```
> >
> > **Почему sql-query-analyzer:**
> > - Знает схему (ваши индексы и колонки)
> > - Ловит N+1 паттерны до продакшена
> > - ~1000 запросов за <100мс (параллелизм rayon)
> >
> > **Ссылки:** [GitHub](https://github.com/RAprogramm/sql-query-analyzer)
> > </details>
> >
> > <details>
> > <summary><strong>Полный Quality Gate в CI</strong></summary>
> >
> > ```yaml
> > jobs:
> >   quality:
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >       - uses: dtolnay/rust-toolchain@stable
> >         with:
> >           components: clippy, rustfmt
> >
> >       - name: Format check
> >         run: cargo +nightly fmt -- --check
> >
> >       - name: Clippy
> >         run: cargo clippy --all-targets -- -D warnings
> >
> >       - name: Tests
> >         run: cargo test --all-features
> >
> >       - name: Doctests
> >         run: cargo test --doc --all-features
> >
> >       - name: Unused dependencies
> >         run: |
> >           cargo install cargo-machete
> >           cargo machete
> >
> >       - name: License & security
> >         run: |
> >           cargo install cargo-deny
> >           cargo deny check
> >
> >       # Качество кода (архитектурные паттерны)
> >       - uses: RAprogramm/cargo-quality@v0
> >         with:
> >           fail_on_issues: 'true'
> >           post_comment: 'true'
> >
> >   msrv:
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >       - uses: dtolnay/rust-toolchain@1.83.0
> >       - run: cargo check --all-features
> >
> >   semver:
> >     if: github.event_name == 'pull_request'
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >       - uses: dtolnay/rust-toolchain@stable
> >       - run: |
> >           cargo install cargo-semver-checks
> >           cargo semver-checks check-release
> >
> >   pr-size:
> >     if: github.event_name == 'pull_request'
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >         with:
> >           fetch-depth: 0
> >       - uses: RAprogramm/rust-prod-diff-checker@v1
> >         with:
> >           max_prod_units: 30
> >           max_weighted_score: 100
> >           fail_on_exceed: 'true'
> >           post_comment: 'true'
> >
> >   sql-analysis:
> >     runs-on: ubuntu-latest
> >     steps:
> >       - uses: actions/checkout@v5
> >       - uses: RAprogramm/sql-query-analyzer@v1
> >         with:
> >           schema: db/schema.sql
> >           queries: db/queries/
> >           upload-sarif: 'true'
> > ```
> > </details>
>
> </details>

<h2>12. CI/CD Архитектура: Один Workflow, Много Jobs</h2>

> [!IMPORTANT]
>
> <p>
>   <strong>Используйте один файл CI workflow с множеством jobs вместо множества отдельных workflow файлов. Это обеспечивает лучший контроль, видимость и управление ресурсами.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > **Проблемы множества Workflows:**
> > - Нет способа синхронизировать jobs между разными workflows
> > - Нельзя определить зависимости (Job C запускается после Job A и B)
> > - Сложнее управлять конкурентностью и отменой
> > - Дублирование конфигурации триггеров между файлами
> > - Распределённая логика CI усложняет отладку
> > - Множество запусков workflow на один коммит тратят больше ресурсов
> >
> > **Решение: Один Workflow с множеством Jobs**
> >
> > 1. **Один файл workflow** содержит всю CI/CD логику
> > 2. **Jobs** обрабатывают разные задачи (тест, сборка, деплой)
> > 3. **`needs`** определяет зависимости между jobs
> > 4. **Reusable workflows** (`_*.yml`) извлекают общие паттерны
> > 5. **Concurrency groups** предотвращают дублирование запусков
> >
> > <details>
> > <summary><strong>Архитектурный паттерн</strong></summary>
> >
> > ```
> > .github/workflows/
> > ├── ci.yml                    # Главный CI workflow (триггерится на push/PR)
> > ├── _build-service.yml        # Reusable: сборка Docker образа
> > ├── _deploy-service.yml       # Reusable: деплой в k8s
> > └── _quality-check.yml        # Reusable: запуск тестов/линтеров
> > ```
> >
> > **Ключевой принцип:** Файлы начинающиеся с `_` - это reusable workflows, вызываемые через `uses:`. Только `ci.yml` определяет триггеры.
> > </details>
> >
> > <details>
> > <summary><strong>Зависимости Jobs с `needs`</strong></summary>
> >
> > ```yaml
> > jobs:
> >   detect-changes:
> >     runs-on: ubuntu-latest
> >     outputs:
> >       api: ${{ steps.filter.outputs.api }}
> >       client: ${{ steps.filter.outputs.client }}
> >
> >   quality-check:
> >     needs: [detect-changes]
> >     if: needs.detect-changes.outputs.api == 'true'
> >
> >   build-api:
> >     needs: [detect-changes, quality-check]
> >     if: |
> >       always() &&
> >       needs.detect-changes.outputs.api == 'true' &&
> >       needs.quality-check.result == 'success'
> >
> >   deploy-api:
> >     needs: [build-api]
> >     if: needs.build-api.result == 'success'
> > ```
> >
> > **Ключевые моменты:**
> > - `needs` создаёт цепочку зависимостей
> > - Jobs выполняются параллельно, если `needs` не задаёт порядок
> > - Используйте `if: always()` для запуска даже если зависимости были пропущены
> > - Проверяйте `needs.<job>.result` для условного выполнения
> > </details>
> >
> > <details>
> > <summary><strong>Контроль конкурентности</strong></summary>
> >
> > ```yaml
> > name: CI/CD Pipeline
> >
> > on:
> >   push:
> >     branches: [main]
> >   pull_request:
> >
> > concurrency:
> >   group: ci-${{ github.ref }}
> >   cancel-in-progress: true
> > ```
> >
> > **Что это делает:**
> > - Группирует запуски по ветке/PR (`github.ref`)
> > - Новый push отменяет предыдущий запущенный workflow
> > - Предотвращает трату ресурсов на устаревшие коммиты
> > - Только один активный запуск на ветку в момент времени
> > </details>
> >
> > <details>
> > <summary><strong>Reusable Workflows</strong></summary>
> >
> > **Основной workflow вызывает reusable:**
> > ```yaml
> > # ci.yml
> > jobs:
> >   build-api:
> >     uses: ./.github/workflows/_build-service.yml
> >     with:
> >       service_name: api-server
> >       dockerfile: ./api-server/Dockerfile
> >     secrets:
> >       registry_token: ${{ secrets.REGISTRY_TOKEN }}
> > ```
> >
> > **Определение reusable workflow:**
> > ```yaml
> > # _build-service.yml
> > name: Build Service
> >
> > on:
> >   workflow_call:
> >     inputs:
> >       service_name:
> >         required: true
> >         type: string
> >       dockerfile:
> >         required: true
> >         type: string
> >     secrets:
> >       registry_token:
> >         required: true
> >     outputs:
> >       image_tag:
> >         value: ${{ jobs.build.outputs.tag }}
> >
> > jobs:
> >   build:
> >     runs-on: ubuntu-latest
> >     outputs:
> >       tag: ${{ steps.meta.outputs.tag }}
> >     steps:
> >       # ... логика сборки
> > ```
> >
> > **Преимущества:**
> > - DRY: одна логика сборки для всех сервисов
> > - Inputs/outputs для конфигурации
> > - Секреты передаются явно (безопасность)
> > - Легко обновить в одном месте
> > </details>
> >
> > <details>
> > <summary><strong>Независимые сборки сервисов</strong></summary>
> >
> > ```yaml
> > jobs:
> >   build-api:
> >     needs: [detect-changes, quality-api]
> >     if: needs.detect-changes.outputs.api == 'true'
> >     uses: ./.github/workflows/_build-service.yml
> >
> >   build-client:
> >     needs: [detect-changes, quality-client]
> >     if: needs.detect-changes.outputs.client == 'true'
> >     uses: ./.github/workflows/_build-service.yml
> >
> >   deploy-api:
> >     needs: [build-api]  # Зависит только от своей сборки
> >     if: needs.build-api.result == 'success'
> >
> >   deploy-client:
> >     needs: [build-client]  # Независим от api
> >     if: needs.build-client.result == 'success'
> > ```
> >
> > **Ключевой принцип:** Деплой каждого сервиса зависит только от своей сборки, не от других сервисов. Если сборка api-server падает, client всё равно может задеплоиться.
> > </details>
> >
> > <details>
> > <summary><strong>Чего НЕ делать</strong></summary>
> >
> > **Не создавайте отдельные файлы workflow для каждой задачи:**
> > ```
> > # ПЛОХО - синхронизация невозможна
> > .github/workflows/
> > ├── test.yml
> > ├── build-api.yml
> > ├── build-client.yml
> > ├── deploy-api.yml
> > ├── deploy-client.yml
> > └── cleanup.yml
> > ```
> >
> > **Не делайте все деплои зависимыми от всех сборок:**
> > ```yaml
> > # ПЛОХО - client ждёт api даже если не связаны
> > deploy-client:
> >   needs: [build-api, build-client, build-worker]
> > ```
> >
> > **Не пропускайте контроль конкурентности:**
> > ```yaml
> > # ПЛОХО - множество запусков тратят ресурсы
> > on:
> >   push:
> >     branches: [main]
> > # Отсутствует: concurrency group
> > ```
> > </details>
> >
> > <details>
> > <summary><strong>Полный пример структуры</strong></summary>
> >
> > ```yaml
> > name: CI/CD Pipeline
> >
> > on:
> >   push:
> >     branches: [main]
> >   workflow_dispatch:
> >     inputs:
> >       deploy_all:
> >         type: boolean
> >         default: false
> >
> > concurrency:
> >   group: ci-${{ github.ref }}
> >   cancel-in-progress: true
> >
> > jobs:
> >   # 1. Определяем что изменилось
> >   detect-changes:
> >     runs-on: ubuntu-latest
> >     outputs:
> >       api: ${{ steps.filter.outputs.api }}
> >       client: ${{ steps.filter.outputs.client }}
> >     steps:
> >       - uses: dorny/paths-filter@v3
> >         id: filter
> >         with:
> >           filters: |
> >             api:
> >               - 'api-server/**'
> >             client:
> >               - 'client/**'
> >
> >   # 2. Quality gates (параллельно)
> >   quality-api:
> >     needs: [detect-changes]
> >     if: needs.detect-changes.outputs.api == 'true'
> >     uses: ./.github/workflows/_quality-check.yml
> >
> >   quality-client:
> >     needs: [detect-changes]
> >     if: needs.detect-changes.outputs.client == 'true'
> >     uses: ./.github/workflows/_quality-check.yml
> >
> >   # 3. Сборка (после quality)
> >   build-api:
> >     needs: [detect-changes, quality-api]
> >     if: needs.quality-api.result == 'success'
> >     uses: ./.github/workflows/_build-service.yml
> >
> >   build-client:
> >     needs: [detect-changes, quality-client]
> >     if: needs.quality-client.result == 'success'
> >     uses: ./.github/workflows/_build-service.yml
> >
> >   # 4. Деплой (независимо для каждого сервиса)
> >   deploy-api:
> >     needs: [build-api]
> >     if: needs.build-api.result == 'success'
> >     uses: ./.github/workflows/_deploy-service.yml
> >
> >   deploy-client:
> >     needs: [build-client]
> >     if: needs.build-client.result == 'success'
> >     uses: ./.github/workflows/_deploy-service.yml
> > ```
> > </details>
>
> </details>

<p align=center>
  <em>Следование этим рекомендациям гарантирует, что наш Rust код будет высококачественным, поддерживаемым и масштабируемым.</em>
</p>
