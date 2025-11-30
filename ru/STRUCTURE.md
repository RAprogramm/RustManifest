# Принципы структурного проектирования

<p align="right">
  <a href="../STRUCTURE.md">English</a> | <strong>Русский</strong>
</p>

## 1. Именование сущностей

> [!IMPORTANT]
>
> <p>
>   <strong>Структуры представляют сущности, а не действия. Избегайте суффиксов <code>-er</code>, <code>-or</code>, <code>-manager</code>, <code>-handler</code>.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > | Избегать | Предпочтительно |
> > |----------|-----------------|
> > | `ConfigLoader` | `Config` |
> > | `MessageParser` | `Message` |
> > | `RequestHandler` | `Request` |
> > | `DataValidator` | `Data` |
> > | `ConnectionManager` | `ConnectionPool` |
> > | `EventDispatcher` | `Events` |
> > | `FileReader` | `File` |
> > | `TokenGenerator` | `Token` |
> >
> > Структура отвечает на вопрос «**что это?**», а не «**что она делает?**».
> >
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > <p>Суффикс <code>-er</code> подразумевает процедурное мышление: «есть нечто, что делает что-то с другими вещами». Это разделяет данные и поведение, приводя к анемичным доменным моделям, где структуры данных не имеют поведения, а классы-«делатели» оперируют пассивными данными.</p>
> > >
> > > <p>Когда структура названа как сущность, она естественным образом инкапсулирует и данные, и операции над ними. Сущность становится ответственной за собственный жизненный цикл и трансформации.</p>
> > >
> > > <details>
> > > <summary><strong>Примеры и пояснения</strong></summary>
> > >
> > > <p><strong>Процедурный подход — данные и поведение разделены:</strong></p>
> > >
> > > ```rust
> > > struct JsonParser;
> > > impl JsonParser {
> > >     fn parse(&self, input: &str) -> Result<Value> { ... }
> > > }
> > >
> > > struct JsonSerializer;
> > > impl JsonSerializer {
> > >     fn serialize(&self, value: &Value) -> String { ... }
> > > }
> > >
> > > // Использование требует знания нескольких типов
> > > let parser = JsonParser;
> > > let value = parser.parse(input)?;
> > > let serializer = JsonSerializer;
> > > let output = serializer.serialize(&value);
> > > ```
> > >
> > > <p><strong>Подход через сущности — данные и поведение объединены:</strong></p>
> > >
> > > ```rust
> > > struct Json {
> > >     value: Value
> > > }
> > >
> > > impl Json {
> > >     fn from_str(input: &str) -> Result<Self> { ... }
> > >     fn to_string(&self) -> String { ... }
> > > }
> > >
> > > // Интуитивно понятное использование
> > > let json = Json::from_str(input)?;
> > > let output = json.to_string();
> > > ```
> > >
> > > <p><strong>Преимущества именования сущностей:</strong></p>
> > >
> > > | Преимущество | Пояснение |
> > > |--------------|-----------|
> > > | Обнаруживаемость | Все операции над JSON в одном месте |
> > > | Инкапсуляция | Внутреннее представление может меняться без влияния на пользователей |
> > > | Уменьшение связанности | Не нужно координировать несколько типов |
> > > | Естественный API | Код читается как естественный язык: «Json из строки» |
> > > | Единая ответственность | Сущность отвечает за свой жизненный цикл |
> > >
> > > </details>
> >
> > </details>
>
> </details>

> [!TIP]
>
> <p>
>   <strong>Некоторые имена с <code>-er</code> допустимы, когда они представляют устоявшиеся паттерны.</strong>
> </p>
>
> <details>
> <summary><strong>Исключения</strong></summary>
>
> > - `Iterator` — конвенция стандартной библиотеки
> > - `Builder` — широко признанный паттерн создания
> > - `Visitor` — паттерн проектирования с определённой семантикой
> > - `Formatter` — трейт стандартной библиотеки
>
> </details>


## 2. Именование методов

> [!IMPORTANT]
>
> <p>
>   <strong>Имена методов отражают их назначение через грамматическую форму: аксессоры — существительные, мутаторы — глаголы, предикаты — прилагательные.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > | Избегать | Предпочтительно | Категория |
> > |----------|-----------------|-----------|
> > | `get_name()` | `name()` | Аксессор |
> > | `get_length()` | `length()` или `len()` | Аксессор |
> > | `get_value()` | `value()` | Аксессор |
> > | `is_empty()` | `empty()` | Предикат |
> > | `is_valid()` | `valid()` | Предикат |
> > | `has_children()` | `children().is_empty()` | Предикат |
> >
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > <p>Префикс <code>get_</code> — это Java-конвенция, которая добавляет шум без информации. В Rust вызов <code>document.name()</code> однозначен — он возвращает имя. Отсутствие глагола подразумевает чистый аксессор.</p>
> > >
> > > <p>Префикс <code>is_</code> для булевых значений создаёт неуклюжий английский: «if document is empty» vs «if document empty». Хотя <code>is_empty()</code> идиоматичен в std, для доменных предикатов прилагательные часто читаются лучше.</p>
> > >
> > > <details>
> > > <summary><strong>Примеры и пояснения</strong></summary>
> > >
> > > ```rust
> > > impl Document {
> > >     // Аксессоры — существительные (возвращают данные)
> > >     pub fn title(&self) -> &str { &self.title }
> > >     pub fn length(&self) -> usize { self.content.len() }
> > >     pub fn author(&self) -> &Author { &self.author }
> > >
> > >     // Предикаты — прилагательные (возвращают boolean)
> > >     pub fn empty(&self) -> bool { self.content.is_empty() }
> > >     pub fn valid(&self) -> bool { self.validate().is_ok() }
> > >     pub fn published(&self) -> bool { self.published_at.is_some() }
> > >
> > >     // Мутаторы — глаголы (выполняют действия)
> > >     pub fn save(&self, path: &Path) -> Result<()> { ... }
> > >     pub fn publish(&mut self) -> Result<()> { ... }
> > >     pub fn delete(self) -> Result<()> { ... }
> > > }
> > > ```
> > >
> > > <p><strong>Соответствие стандартной библиотеке:</strong></p>
> > >
> > > ```rust
> > > // std использует аксессоры-существительные
> > > vec.len()           // не get_length()
> > > vec.capacity()      // не get_capacity()
> > > string.chars()      // не get_chars()
> > > path.parent()       // не get_parent()
> > > option.as_ref()     // не get_as_ref()
> > > ```
> > >
> > > <p><strong>Преимущества:</strong></p>
> > >
> > > | Преимущество | Пояснение |
> > > |--------------|-----------|
> > > | Краткость | Меньше печатать, меньше читать |
> > > | Ясность | Грамматическая форма указывает на поведение |
> > > | Согласованность | Соответствует стилю стандартной библиотеки |
> > > | Fluent API | Цепочки читаются естественно: `doc.content().lines().count()` |
> > >
> > > </details>
> >
> > </details>
>
> </details>


## 3. Размер структуры

> [!IMPORTANT]
>
> <p>
>   <strong>Структура должна иметь не более 4 полей. Большее количество полей указывает на множественные ответственности, которые следует разделить через композицию.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > <p>Большие структуры — это запах кода, указывающий на нарушение принципа единственной ответственности. Когда структура имеет много полей:</p>
> > >
> > > - **Тестирование усложняется** — много комбинаций для покрытия
> > > - **Изменения расползаются** — модификация одного аспекта влияет на несвязанный код
> > > - **Понимание затруднено** — сложно уловить назначение структуры
> > > - **Переиспользование ограничено** — нельзя использовать части независимо
> >
> > </details>
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> >
> > <p><strong>До — 14 полей, слишком много ответственностей:</strong></p>
> >
> > ```rust
> > pub struct User {
> >     id: Uuid,
> >     username: String,
> >     email: String,
> >     password_hash: String,
> >     first_name: Option<String>,
> >     last_name: Option<String>,
> >     avatar_url: Option<String>,
> >     created_at: DateTime<Utc>,
> >     updated_at: DateTime<Utc>,
> >     last_login: Option<DateTime<Utc>>,
> >     role: Role,
> >     permissions: Vec<Permission>,
> >     settings: UserSettings,
> >     status: AccountStatus
> > }
> > ```
> >
> > <p>Эта структура обрабатывает: идентичность, аутентификацию, профиль, временные метки, авторизацию, настройки и состояние.</p>
> >
> > <p><strong>После — композиция с фокусированными компонентами:</strong></p>
> >
> > ```rust
> > pub struct User {
> >     identity: UserIdentity,
> >     credentials: Credentials,
> >     profile: UserProfile,
> >     access: AccessControl
> > }
> >
> > pub struct UserIdentity {
> >     id: Uuid,
> >     username: String,
> >     timestamps: Timestamps
> > }
> >
> > pub struct Credentials {
> >     email: Email,
> >     password_hash: PasswordHash,
> >     last_login: Option<DateTime<Utc>>
> > }
> >
> > pub struct UserProfile {
> >     name: PersonName,
> >     avatar_url: Option<Url>,
> >     settings: UserSettings
> > }
> >
> > pub struct AccessControl {
> >     role: Role,
> >     permissions: Permissions,
> >     status: AccountStatus
> > }
> >
> > // Переиспользуется между сущностями
> > pub struct Timestamps {
> >     created_at: DateTime<Utc>,
> >     updated_at: DateTime<Utc>
> > }
> > ```
> >
> > <p><strong>Преимущества композиции:</strong></p>
> >
> > | Преимущество | Пояснение |
> > |--------------|-----------|
> > | Тестируемость | Каждый компонент тестируется независимо |
> > | Переиспользуемость | `Timestamps` работает для любой сущности |
> > | Ясность | Назначение очевидно из структуры |
> > | Поддерживаемость | Изменения локализованы |
> > | Типобезопасность | `Email` vs `String` предотвращает ошибки |
> >
> > </details>
>
> </details>


## 4. Размер публичного API

> [!IMPORTANT]
>
> <p>
>   <strong>Публичный интерфейс структуры должен иметь не более 5 методов. Большее количество методов указывает, что структура делает слишком много и должна быть разделена.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > <p>Большой публичный API указывает на смешение нескольких ответственностей:</p>
> > >
> > > - Пользователям нужно понимать больше для использования типа
> > > - Документация становится громоздкой
> > > - Изменения становятся рискованными
> > > - Площадь тестирования взрывается
> > >
> > > <p>Правило «5 методов» заставляет определить основную ответственность и выделить вторичные заботы в отдельные типы.</p>
> >
> > </details>
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> >
> > <p><strong>До — 10+ методов, структура делает слишком много:</strong></p>
> >
> > ```rust
> > impl Document {
> >     pub fn new() -> Self
> >     pub fn from_file(path: &Path) -> Result<Self>
> >     pub fn save(&self, path: &Path) -> Result<()>
> >     pub fn content(&self) -> &str
> >     pub fn set_content(&mut self, content: &str)
> >     pub fn validate(&self) -> Result<()>
> >     pub fn render_html(&self) -> String
> >     pub fn render_pdf(&self) -> Vec<u8>
> >     pub fn compress(&self) -> Vec<u8>
> >     pub fn encrypt(&self, key: &Key) -> Vec<u8>
> >     pub fn share(&self) -> Url
> > }
> > ```
> >
> > <p><strong>После — разделение ответственностей:</strong></p>
> >
> > ```rust
> > // Основные операции с документом
> > impl Document {
> >     pub fn new(title: &str) -> Self
> >     pub fn load(source: impl Source) -> Result<Self>
> >     pub fn save(&self, target: impl Target) -> Result<()>
> >     pub fn content(&self) -> &Content
> >     pub fn metadata(&self) -> &Metadata
> > }
> >
> > // Рендеринг — отдельная забота
> > pub struct Renderer;
> > impl Renderer {
> >     pub fn html(doc: &Document) -> String
> >     pub fn pdf(doc: &Document) -> Vec<u8>
> > }
> >
> > // Операции экспорта/трансформации
> > pub struct Exporter;
> > impl Exporter {
> >     pub fn compress(doc: &Document) -> Vec<u8>
> >     pub fn encrypt(doc: &Document, key: &Key) -> Vec<u8>
> > }
> > ```
> >
> > <p><strong>Что учитывается в лимите:</strong></p>
> >
> > | Учитывать | Не учитывать |
> > |-----------|--------------|
> > | Методы, определяющие поведение типа | Реализации трейтов (`Display`, `Debug`, `From`) |
> > | Кастомные конструкторы | `new()` и `default()` |
> > | Методы публичного API | Приватные хелперы |
> >
> > </details>
>
> </details>


## 5. Проектирование конструкторов

> [!IMPORTANT]
>
> <p>
>   <strong>Конструкторы должны только присваивать поля. Вся обработка, валидация и ввод-вывод принадлежат методам, обеспечивая ленивое вычисление и упрощая тестирование.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > <p>Когда конструкторы содержат логику:</p>
> > >
> > > - **Они могут падать** — усложняя создание объектов
> > > - **Они нетерпеливые** — работа происходит, даже если не используется
> > > - **Они негибкие** — нет способа создать объект иначе
> > > - **Их сложно тестировать** — требуют реальных ресурсов
> > >
> > > <p>Перенос логики в методы обеспечивает:</p>
> > >
> > > - **Безошибочное создание** — объект всегда создаётся
> > > - **Ленивое вычисление** — работа происходит когда нужно
> > > - **Множественные пути создания** — `from_data()` для тестов
> > > - **Кэширование** — дорогие операции происходят один раз
> >
> > </details>
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> >
> > <p><strong>Неправильно — логика в конструкторе:</strong></p>
> >
> > ```rust
> > impl Config {
> >     pub fn new(path: &str) -> Result<Self> {
> >         // Валидация в конструкторе
> >         if path.is_empty() {
> >             return Err(Error::InvalidPath);
> >         }
> >
> >         // Ввод-вывод в конструкторе
> >         let content = std::fs::read_to_string(path)?;
> >
> >         // Парсинг в конструкторе
> >         let data: ConfigData = toml::from_str(&content)?;
> >
> >         Ok(Self { data })
> >     }
> > }
> >
> > // Проблемы:
> > // - Конструктор может упасть несколькими способами
> > // - Нельзя создать Config без доступа к файлу
> > // - Сложно тестировать — нужны реальные файлы
> > // - Файл читается, даже если конфиг никогда не используется
> > ```
> >
> > <p><strong>Правильно — только присваивание, логика в методах:</strong></p>
> >
> > ```rust
> > impl Config {
> >     /// Создаёт конфиг, который загрузится из указанного пути.
> >     /// Не читает файл — загрузка происходит при первом доступе.
> >     pub fn new(path: impl Into<PathBuf>) -> Self {
> >         Self {
> >             path: path.into(),
> >             cached: OnceCell::new()
> >         }
> >     }
> >
> >     /// Создаёт конфиг с предзагруженными данными.
> >     /// Полезно для тестирования или когда данные приходят из других источников.
> >     pub fn from_data(data: ConfigData) -> Self {
> >         Self {
> >             path: PathBuf::new(),
> >             cached: OnceCell::from(data)
> >         }
> >     }
> >
> >     /// Возвращает данные конфигурации, загружая из файла при необходимости.
> >     /// Результаты кэшируются для последующих вызовов.
> >     pub fn data(&self) -> Result<&ConfigData> {
> >         self.cached.get_or_try_init(|| {
> >             let content = std::fs::read_to_string(&self.path)?;
> >             let data: ConfigData = toml::from_str(&content)?;
> >             Self::validate(&data)?;
> >             Ok(data)
> >         })
> >     }
> > }
> >
> > // Преимущества:
> > // - Конструктор никогда не падает
> > // - Легко создать тестовые конфиги через from_data()
> > // - Ленивая загрузка — файл читается только когда нужно
> > // - Автоматическое кэширование — файл читается один раз
> > ```
> >
> > </details>
>
> </details>


## 6. Делегирующие конструкторы

> [!TIP]
>
> <p>
>   <strong>Один главный конструктор принимает все параметры. Остальные конструкторы делегируют ему, обеспечивая согласованную инициализацию.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > <p>Без главного конструктора каждый конструктор инициализирует поля независимо:</p>
> > >
> > > ```rust
> > > // Проблема: инициализация полей дублируется
> > > impl Server {
> > >     pub fn new(addr: SocketAddr) -> Self {
> > >         Self {
> > >             addr,
> > >             tls: None,
> > >             options: ServerOptions::default(),
> > >             state: ServerState::Stopped  // дублируется
> > >         }
> > >     }
> > >
> > >     pub fn secure(addr: SocketAddr, tls: TlsConfig) -> Self {
> > >         Self {
> > >             addr,
> > >             tls: Some(tls),
> > >             options: ServerOptions::default(),
> > >             state: ServerState::Stopped  // дублируется
> > >         }
> > >     }
> > > }
> > > ```
> > >
> > > <p>Проблемы:</p>
> > >
> > > - Добавление поля требует обновления каждого конструктора
> > > - Легко забыть инициализацию в одном конструкторе
> > > - Значения по умолчанию могут разойтись между конструкторами
> >
> > </details>
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> >
> > <p><strong>Правильно — один главный, остальные делегируют:</strong></p>
> >
> > ```rust
> > impl Server {
> >     // Главный конструктор — принимает всю конфигурацию
> >     pub fn with_config(
> >         addr: SocketAddr,
> >         tls: Option<TlsConfig>,
> >         options: ServerOptions
> >     ) -> Self {
> >         Self {
> >             addr,
> >             tls,
> >             options,
> >             state: ServerState::Stopped
> >         }
> >     }
> >
> >     // Удобные конструкторы делегируют главному
> >     pub fn new(addr: SocketAddr) -> Self {
> >         Self::with_config(addr, None, ServerOptions::default())
> >     }
> >
> >     pub fn secure(addr: SocketAddr, tls: TlsConfig) -> Self {
> >         Self::with_config(addr, Some(tls), ServerOptions::default())
> >     }
> >
> >     pub fn localhost(port: u16) -> Self {
> >         Self::new(SocketAddr::from(([127, 0, 0, 1], port)))
> >     }
> >
> >     pub fn localhost_secure(port: u16, tls: TlsConfig) -> Self {
> >         Self::secure(SocketAddr::from(([127, 0, 0, 1], port)), tls)
> >     }
> > }
> > ```
> >
> > <p><strong>Преимущества:</strong></p>
> >
> > | Преимущество | Пояснение |
> > |--------------|-----------|
> > | Единый источник истины | Вся инициализация в одном месте |
> > | Безопасность | Невозможно забыть инициализировать поля |
> > | Согласованность | Все объекты инициализируются одинаково |
> > | Расширяемость | Добавление полей требует одного изменения |
> > | Удобство | Легко добавлять новые сокращения |
> >
> > </details>
>
> </details>


## 7. Неизменяемость прежде всего

> [!IMPORTANT]
>
> <p>
>   <strong>Предпочитайте возврат новых объектов мутации существующих. Используйте <code>self</code> вместо <code>&mut self</code> где возможно.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > <p>Мутабельные объекты вносят сложность:</p>
> > >
> > > - **Баги разделяемого состояния** — объект изменяется неожиданно
> > > - **Потокобезопасность** — требуется синхронизация
> > > - **Временная связанность** — порядок операций важен
> > > - **Неполное состояние** — объект может быть частично сконфигурирован
> > >
> > > <p>Неизменяемые объекты обеспечивают:</p>
> > >
> > > - **Предсказуемость** — состояние объекта фиксировано после создания
> > > - **Потокобезопасность** — безопасно делиться без блокировок
> > > - **Отсутствие временной связанности** — операции независимы
> > > - **Атомарность** — объект всегда полон
> >
> > </details>
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> >
> > <p><strong>Мутабельный подход:</strong></p>
> >
> > ```rust
> > impl Request {
> >     pub fn set_header(&mut self, key: &str, value: &str) {
> >         self.headers.insert(key.into(), value.into());
> >     }
> >
> >     pub fn set_body(&mut self, body: Vec<u8>) {
> >         self.body = body;
> >     }
> > }
> >
> > // Использование — требует mut привязку
> > let mut request = Request::new(Method::GET, url);
> > request.set_header("Content-Type", "application/json");
> > request.set_header("Authorization", token);
> > request.set_body(payload);
> > ```
> >
> > <p><strong>Неизменяемый подход:</strong></p>
> >
> > ```rust
> > impl Request {
> >     pub fn header(mut self, key: &str, value: &str) -> Self {
> >         self.headers.insert(key.into(), value.into());
> >         self
> >     }
> >
> >     pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
> >         self.body = body.into();
> >         self
> >     }
> > }
> >
> > // Использование — mut не нужен, fluent цепочка
> > let request = Request::new(Method::GET, url)
> >     .header("Content-Type", "application/json")
> >     .header("Authorization", token)
> >     .body(payload);
> > ```
> >
> > <p><strong>Преимущества:</strong></p>
> >
> > | Преимущество | Пояснение |
> > |--------------|-----------|
> > | Потокобезопасность | Синхронизация не нужна |
> > | Предсказуемость | Нет неожиданных мутаций |
> > | Fluent API | Естественные цепочки методов |
> > | Отладка | Состояние не меняется неожиданно |
> > | Атомарность | Объект всегда валиден или не создан |
> >
> > </details>
>
> </details>

> [!NOTE]
>
> <p>
>   <strong>Некоторые случаи требуют <code>&mut self</code>: большие структуры данных, интеграция с I/O, критичные к производительности циклы, стандартные трейты вроде <code>Iterator::next</code>.</strong>
> </p>
>
> <details>
> <summary><strong>Когда мутабельность уместна</strong></summary>
>
> > ```rust
> > // Уместная мутабельность — большие данные, критична производительность
> > impl Document {
> >     pub fn apply_update(&mut self, update: &Update) {
> >         // Модификация большой CRDT структуры на месте
> >         // Копирование было бы непозволительно дорогим
> >     }
> > }
> > ```
>
> </details>


## 8. Инкапсуляция констант

> [!TIP]
>
> <p>
>   <strong>Константы принадлежат структурам, которые их используют, а не глобальной области. Это улучшает обнаруживаемость и предотвращает загрязнение пространства имён.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > <p>Глобальные константы создают скрытые зависимости и ухудшают обнаруживаемость:</p>
> > >
> > > ```rust
> > > // Глобальные константы — разбросаны, нет контекста
> > > pub const MAX_CONNECTIONS: usize = 100;
> > > pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
> > > pub const BUFFER_SIZE: usize = 8192;
> > > ```
> > >
> > > <p>Проблемы:</p>
> > >
> > > - **Нет контекста** — что использует `MAX_CONNECTIONS`?
> > > - **Конфликты имён** — нужны префиксы для различения
> > > - **Сложно найти** — разбросаны по кодовой базе
> > > - **Нет группировки документации**
> >
> > </details>
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> >
> > <p><strong>Правильно — константы принадлежат своим типам:</strong></p>
> >
> > ```rust
> > impl ConnectionPool {
> >     /// Максимальное количество соединений в пуле
> >     pub const MAX_SIZE: usize = 100;
> >
> >     /// Сколько ждать перед повтором неудавшегося соединения
> >     pub const RECONNECT_DELAY: Duration = Duration::from_secs(1);
> > }
> >
> > impl Client {
> >     /// Таймаут запроса по умолчанию
> >     pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
> > }
> >
> > impl Buffer {
> >     /// Ёмкость буфера по умолчанию в байтах
> >     pub const DEFAULT_CAPACITY: usize = 8192;
> > }
> >
> > // Использование понятно и обнаруживаемо
> > let pool = ConnectionPool::with_max(ConnectionPool::MAX_SIZE / 2);
> > let client = Client::new().timeout(Client::DEFAULT_TIMEOUT);
> > ```
> >
> > <p><strong>Для вычисляемых значений используйте ассоциированные функции:</strong></p>
> >
> > ```rust
> > impl Server {
> >     // Константа времени компиляции
> >     pub const MAX_HEADER_SIZE: usize = 8192;
> >
> >     // «Константа» времени выполнения — вычисляется один раз
> >     pub fn default_addr() -> SocketAddr {
> >         SocketAddr::from(([0, 0, 0, 0], 8080))
> >     }
> >
> >     // Зависит от окружения
> >     pub fn max_threads() -> usize {
> >         std::thread::available_parallelism()
> >             .map(|n| n.get())
> >             .unwrap_or(4)
> >     }
> > }
> > ```
> >
> > <p><strong>Преимущества:</strong></p>
> >
> > | Преимущество | Пояснение |
> > |--------------|-----------|
> > | Обнаруживаемость | Константы находятся там, где используются |
> > | Документация | Константы документируются вместе с типом |
> > | Пространство имён | Не нужны соглашения о префиксах |
> > | Инкапсуляция | Детали реализации остаются приватными |
> > | Рефакторинг | Легко менять без глобального поиска |
> >
> > </details>
>
> </details>


## 9. Тестирование с Fake-объектами

> [!IMPORTANT]
>
> <p>
>   <strong>Используйте простые fake-реализации вместо mock-библиотек. Fakes предоставляют реальное поведение; mocks проверяют последовательности вызовов.</strong>
> </p>
>
> <details>
> <summary><strong>Подробнее</strong></summary>
>
> > <details>
> > <summary><strong>Почему это важно?</strong></summary>
> >
> > > | Аспект | Mocks | Fakes |
> > > |--------|-------|-------|
> > > | Связанность тестов | Высокая — тесты знают реализацию | Низкая — тесты проверяют результаты |
> > > | Поддержка | Ломаются при изменении реализации | Стабильны при рефакторинге |
> > > | Реалистичность | Симулируют только интерфейс | Предоставляют реальное поведение |
> > > | Сложность | Кривая обучения DSL | Обычный Rust код |
> > > | Отладка | Ошибки моков криптичны | Стандартные assertions |
> >
> > </details>
> >
> > <details>
> > <summary><strong>Примеры и пояснения</strong></summary>
> >
> > <p><strong>Mock подход — тесно связан с реализацией:</strong></p>
> >
> > ```rust
> > use mockall::automock;
> >
> > #[automock]
> > trait Storage {
> >     fn save(&self, key: &str, data: &[u8]) -> Result<()>;
> >     fn load(&self, key: &str) -> Result<Vec<u8>>;
> > }
> >
> > #[test]
> > fn test_cache_with_mock() {
> >     let mut mock = MockStorage::new();
> >
> >     // Настройка ожиданий — связано с деталями реализации
> >     mock.expect_load()
> >         .with(eq("user:123"))
> >         .times(1)
> >         .returning(|_| Err(Error::NotFound));
> >
> >     mock.expect_save()
> >         .with(eq("user:123"), eq(b"data"))
> >         .times(1)
> >         .returning(|_, _| Ok(()));
> >
> >     let cache = Cache::new(mock);
> >     cache.get_or_fetch("user:123", || Ok(b"data".to_vec()))?;
> >
> >     // Проблемы:
> >     // - Ломается при изменении порядка вызовов
> >     // - Не тестирует реальное поведение хранилища
> >     // - Сложная настройка для простого теста
> > }
> > ```
> >
> > <p><strong>Fake подход — тестирует поведение, не реализацию:</strong></p>
> >
> > ```rust
> > /// In-memory хранилище для тестирования
> > struct FakeStorage {
> >     data: RefCell<HashMap<String, Vec<u8>>>,
> >     fail_on: RefCell<HashSet<String>>
> > }
> >
> > impl FakeStorage {
> >     fn new() -> Self {
> >         Self {
> >             data: RefCell::new(HashMap::new()),
> >             fail_on: RefCell::new(HashSet::new())
> >         }
> >     }
> >
> >     /// Настроить ключ для падения при следующем доступе
> >     fn fail_next(&self, key: &str) {
> >         self.fail_on.borrow_mut().insert(key.into());
> >     }
> >
> >     /// Проверить, существует ли ключ
> >     fn contains(&self, key: &str) -> bool {
> >         self.data.borrow().contains_key(key)
> >     }
> >
> >     /// Получить сохранённое значение для assertions
> >     fn get(&self, key: &str) -> Option<Vec<u8>> {
> >         self.data.borrow().get(key).cloned()
> >     }
> > }
> >
> > impl Storage for FakeStorage {
> >     fn save(&self, key: &str, data: &[u8]) -> Result<()> {
> >         if self.fail_on.borrow_mut().remove(key) {
> >             return Err(Error::Io("simulated failure".into()));
> >         }
> >         self.data.borrow_mut().insert(key.into(), data.into());
> >         Ok(())
> >     }
> >
> >     fn load(&self, key: &str) -> Result<Vec<u8>> {
> >         if self.fail_on.borrow_mut().remove(key) {
> >             return Err(Error::Io("simulated failure".into()));
> >         }
> >         self.data.borrow()
> >             .get(key)
> >             .cloned()
> >             .ok_or(Error::NotFound)
> >     }
> > }
> >
> > #[test]
> > fn test_cache_saves_on_miss() {
> >     let storage = FakeStorage::new();
> >     let cache = Cache::new(&storage);
> >
> >     // Действие
> >     let result = cache.get_or_fetch("user:123", || Ok(b"data".to_vec()))?;
> >
> >     // Проверка реального поведения
> >     assert_eq!(result, b"data");
> >     assert!(storage.contains("user:123"));
> >     assert_eq!(storage.get("user:123"), Some(b"data".to_vec()));
> > }
> >
> > #[test]
> > fn test_cache_handles_storage_failure() {
> >     let storage = FakeStorage::new();
> >     storage.fail_next("user:123");
> >
> >     let cache = Cache::new(&storage);
> >     let result = cache.get_or_fetch("user:123", || Ok(b"data".to_vec()));
> >
> >     assert!(result.is_err());
> > }
> > ```
> >
> > <p><strong>Преимущества fakes:</strong></p>
> >
> > | Преимущество | Пояснение |
> > |--------------|-----------|
> > | Тестирование поведения | Проверяем что код делает, а не как |
> > | Безопасность рефакторинга | Тесты переживают изменения реализации |
> > | Переиспользуемость | Один fake служит многим тестам |
> > | Простота | Не нужно изучать mock-библиотеку |
> > | Отлаживаемость | Стандартная отладка Rust |
> > | Документация | Fake показывает ожидаемое использование интерфейса |
> >
> > </details>
>
> </details>

> [!NOTE]
>
> <p>
>   <strong>Mocks уместны для проверки специфических взаимодействий с внешними системами, тестирования того, что методы НЕ вызываются, или тестирования сложных протоколов со строгим порядком.</strong>
> </p>


## 10. Сводка

> [!TIP]
>
> <p>
>   <strong>Краткая справочная таблица по всем принципам структурного проектирования.</strong>
> </p>
>
> <details>
> <summary><strong>Полная справка</strong></summary>
>
> > | Принцип | Руководство | Преимущество |
> > |---------|-------------|--------------|
> > | Именование сущностей | Без суффиксов `-er` | Естественный API, инкапсуляция |
> > | Именование методов | Существительные для аксессоров, глаголы для действий | Ясность, согласованность |
> > | Размер структуры | Максимум 4 поля | Тестируемость, единая ответственность |
> > | Размер API | Максимум 5 публичных методов | Изучаемость, стабильность |
> > | Конструкторы | Только присваивание, без логики | Надёжность, тестируемость |
> > | Делегирование | Один главный конструктор | Согласованность, безопасность |
> > | Неизменяемость | Предпочитать `self` вместо `&mut self` | Потокобезопасность, предсказуемость |
> > | Константы | Инкапсулировать в типах | Обнаруживаемость, пространство имён |
> > | Тестирование | Fakes вместо mocks | Проверка поведения, поддерживаемость |
> >
> > <details>
> > <summary><strong>Краткая справка по коду</strong></summary>
> >
> > ```rust
> > // Именование структур
> > struct Document;        // не DocumentManager
> > struct Connection;      // не ConnectionHandler
> >
> > // Именование методов
> > fn name(&self);         // не get_name()
> > fn empty(&self);        // не is_empty()
> > fn save(&self);         // глагол действия
> >
> > // Размер структуры
> > struct User {
> >     identity: Identity,
> >     profile: Profile,
> >     access: Access       // максимум 4 поля
> > }
> >
> > // Проектирование конструкторов
> > impl Config {
> >     fn new(path: PathBuf) -> Self {
> >         Self { path }    // только присваивание
> >     }
> >
> >     fn data(&self) -> Result<&Data> {
> >         // логика здесь, не в конструкторе
> >     }
> > }
> >
> > // Неизменяемая трансформация
> > impl Request {
> >     fn header(mut self, k: &str, v: &str) -> Self {
> >         self.headers.insert(k.into(), v.into());
> >         self
> >     }
> > }
> >
> > // Инкапсулированные константы
> > impl Buffer {
> >     pub const DEFAULT_CAPACITY: usize = 8192;
> > }
> > ```
> >
> > </details>
>
> </details>

<p align="center">
  <em>Следование этим принципам обеспечивает чистый, поддерживаемый и тестируемый Rust код.</em>
</p>
