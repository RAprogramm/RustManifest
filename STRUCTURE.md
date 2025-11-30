# Structural Design Principles

<p align="right">
  <strong>English</strong> | <a href="./ru/STRUCTURE.md">Русский</a>
</p>

## 1. Entity Naming

> [!IMPORTANT]
>
> <p>
>   <strong>Structures represent entities, not actions. Avoid <code>-er</code>, <code>-or</code>, <code>-manager</code>, <code>-handler</code> suffixes.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > | Avoid | Prefer |
> > |-------|--------|
> > | `ConfigLoader` | `Config` |
> > | `MessageParser` | `Message` |
> > | `RequestHandler` | `Request` |
> > | `DataValidator` | `Data` |
> > | `ConnectionManager` | `ConnectionPool` |
> > | `EventDispatcher` | `Events` |
> > | `FileReader` | `File` |
> > | `TokenGenerator` | `Token` |
> >
> > A structure answers the question "**what is it?**", not "**what does it do?**".
> >
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > <p>The <code>-er</code> suffix implies procedural thinking: "there is a thing that does something to other things." This separates data from behavior, leading to anemic domain models where data structures have no behavior and "doer" classes operate on passive data.</p>
> > >
> > > <p>When a structure is named as an entity, it naturally encapsulates both data and the operations on that data. The entity becomes responsible for its own lifecycle and transformations.</p>
> > >
> > > <details>
> > > <summary><strong>Examples & Further Explanation</strong></summary>
> > >
> > > <p><strong>Procedural approach — data and behavior are separated:</strong></p>
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
> > > // Usage requires knowing about multiple types
> > > let parser = JsonParser;
> > > let value = parser.parse(input)?;
> > > let serializer = JsonSerializer;
> > > let output = serializer.serialize(&value);
> > > ```
> > >
> > > <p><strong>Entity approach — data and behavior are unified:</strong></p>
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
> > > // Usage is intuitive
> > > let json = Json::from_str(input)?;
> > > let output = json.to_string();
> > > ```
> > >
> > > <p><strong>Benefits of entity naming:</strong></p>
> > >
> > > | Benefit | Explanation |
> > > |---------|-------------|
> > > | Discoverability | All operations on JSON are found in one place |
> > > | Encapsulation | Internal representation can change without affecting users |
> > > | Reduced coupling | No need to coordinate multiple types |
> > > | Natural API | Code reads like natural language: "Json from string" |
> > > | Single responsibility | The entity is responsible for its own lifecycle |
> > >
> > > </details>
> >
> > </details>
>
> </details>

> [!TIP]
>
> <p>
>   <strong>Some <code>-er</code> names are acceptable when they represent well-established patterns.</strong>
> </p>
>
> <details>
> <summary><strong>Exceptions</strong></summary>
>
> > - `Iterator` — standard library convention
> > - `Builder` — widely recognized creational pattern
> > - `Visitor` — design pattern with specific semantics
> > - `Formatter` — standard library trait
>
> </details>


## 2. Method Naming

> [!IMPORTANT]
>
> <p>
>   <strong>Method names reflect their purpose through grammatical form: accessors are nouns, mutators are verbs, predicates are adjectives.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > | Avoid | Prefer | Category |
> > |-------|--------|----------|
> > | `get_name()` | `name()` | Accessor |
> > | `get_length()` | `length()` or `len()` | Accessor |
> > | `get_value()` | `value()` | Accessor |
> > | `is_empty()` | `empty()` | Predicate |
> > | `is_valid()` | `valid()` | Predicate |
> > | `has_children()` | `children().is_empty()` | Predicate |
> >
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > <p>The <code>get_</code> prefix is a Java convention that adds noise without information. In Rust, calling <code>document.name()</code> is unambiguous — it returns the name. The absence of a verb implies a pure accessor.</p>
> > >
> > > <p>The <code>is_</code> prefix for booleans creates awkward English: "if document is empty" vs "if document empty". While <code>is_empty()</code> is idiomatic in std, for domain-specific predicates, adjectives often read better.</p>
> > >
> > > <details>
> > > <summary><strong>Examples & Further Explanation</strong></summary>
> > >
> > > ```rust
> > > impl Document {
> > >     // Accessors — nouns (return data)
> > >     pub fn title(&self) -> &str { &self.title }
> > >     pub fn length(&self) -> usize { self.content.len() }
> > >     pub fn author(&self) -> &Author { &self.author }
> > >
> > >     // Predicates — adjectives (return boolean)
> > >     pub fn empty(&self) -> bool { self.content.is_empty() }
> > >     pub fn valid(&self) -> bool { self.validate().is_ok() }
> > >     pub fn published(&self) -> bool { self.published_at.is_some() }
> > >
> > >     // Mutators — verbs (perform actions)
> > >     pub fn save(&self, path: &Path) -> Result<()> { ... }
> > >     pub fn publish(&mut self) -> Result<()> { ... }
> > >     pub fn delete(self) -> Result<()> { ... }
> > > }
> > > ```
> > >
> > > <p><strong>Standard library alignment:</strong></p>
> > >
> > > ```rust
> > > // std uses noun-style accessors
> > > vec.len()           // not get_length()
> > > vec.capacity()      // not get_capacity()
> > > string.chars()      // not get_chars()
> > > path.parent()       // not get_parent()
> > > option.as_ref()     // not get_as_ref()
> > > ```
> > >
> > > <p><strong>Benefits:</strong></p>
> > >
> > > | Benefit | Explanation |
> > > |---------|-------------|
> > > | Conciseness | Less typing, less reading |
> > > | Clarity | Grammatical form indicates behavior |
> > > | Consistency | Matches standard library style |
> > > | Fluent API | Chains read naturally: `doc.content().lines().count()` |
> > >
> > > </details>
> >
> > </details>
>
> </details>


## 3. Structure Size

> [!IMPORTANT]
>
> <p>
>   <strong>A structure should have no more than 4 fields. More fields indicate multiple responsibilities that should be separated through composition.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > <p>Large structures are a code smell indicating violation of the Single Responsibility Principle. When a structure has many fields:</p>
> > >
> > > - **Testing becomes complex** — many combinations to cover
> > > - **Changes ripple** — modifying one aspect affects unrelated code
> > > - **Understanding is difficult** — hard to grasp the structure's purpose
> > > - **Reuse is limited** — can't use parts independently
> >
> > </details>
> >
> > <details>
> > <summary><strong>Examples & Further Explanation</strong></summary>
> >
> > <p><strong>Before — 14 fields, too many responsibilities:</strong></p>
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
> > <p>This structure handles: identity, authentication, profile, timestamps, authorization, preferences, and state.</p>
> >
> > <p><strong>After — composition with focused components:</strong></p>
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
> > // Reusable across entities
> > pub struct Timestamps {
> >     created_at: DateTime<Utc>,
> >     updated_at: DateTime<Utc>
> > }
> > ```
> >
> > <p><strong>Benefits of composition:</strong></p>
> >
> > | Benefit | Explanation |
> > |---------|-------------|
> > | Testability | Each component tests independently |
> > | Reusability | `Timestamps` works for any entity |
> > | Clarity | Purpose is obvious from structure |
> > | Maintainability | Changes are localized |
> > | Type safety | `Email` vs `String` prevents mistakes |
> >
> > </details>
>
> </details>


## 4. Public API Size

> [!IMPORTANT]
>
> <p>
>   <strong>A structure's public interface should have no more than 5 methods. More methods indicate the structure does too much and should be split.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > <p>A large public API indicates multiple responsibilities mixed together:</p>
> > >
> > > - Users must understand more to use the type
> > > - Documentation grows unwieldy
> > > - Changes become risky
> > > - Testing surface area explodes
> > >
> > > <p>The "5 methods" guideline forces you to identify the core responsibility and extract secondary concerns into separate types.</p>
> >
> > </details>
> >
> > <details>
> > <summary><strong>Examples & Further Explanation</strong></summary>
> >
> > <p><strong>Before — 10+ methods, structure does too much:</strong></p>
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
> > <p><strong>After — separation of concerns:</strong></p>
> >
> > ```rust
> > // Core document operations
> > impl Document {
> >     pub fn new(title: &str) -> Self
> >     pub fn load(source: impl Source) -> Result<Self>
> >     pub fn save(&self, target: impl Target) -> Result<()>
> >     pub fn content(&self) -> &Content
> >     pub fn metadata(&self) -> &Metadata
> > }
> >
> > // Rendering is a separate concern
> > pub struct Renderer;
> > impl Renderer {
> >     pub fn html(doc: &Document) -> String
> >     pub fn pdf(doc: &Document) -> Vec<u8>
> > }
> >
> > // Export/transform operations
> > pub struct Exporter;
> > impl Exporter {
> >     pub fn compress(doc: &Document) -> Vec<u8>
> >     pub fn encrypt(doc: &Document, key: &Key) -> Vec<u8>
> > }
> > ```
> >
> > <p><strong>What counts toward the limit:</strong></p>
> >
> > | Include | Exclude |
> > |---------|---------|
> > | Methods defining type's behavior | Trait implementations (`Display`, `Debug`, `From`) |
> > | Custom constructors | `new()` and `default()` |
> > | Public API methods | Private helpers |
> >
> > </details>
>
> </details>


## 5. Constructor Design

> [!IMPORTANT]
>
> <p>
>   <strong>Constructors should only assign fields. All processing, validation, and I/O belong in methods, enabling lazy evaluation and easier testing.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > <p>When constructors contain logic:</p>
> > >
> > > - **They can fail** — complicating object creation
> > > - **They're eager** — work happens even if unused
> > > - **They're inflexible** — no way to create object differently
> > > - **They're hard to test** — require real resources
> > >
> > > <p>Moving logic to methods enables:</p>
> > >
> > > - **Infallible construction** — object always created
> > > - **Lazy evaluation** — work happens when needed
> > > - **Multiple creation paths** — `from_data()` for tests
> > > - **Caching** — expensive operations happen once
> >
> > </details>
> >
> > <details>
> > <summary><strong>Examples & Further Explanation</strong></summary>
> >
> > <p><strong>Incorrect — logic in constructor:</strong></p>
> >
> > ```rust
> > impl Config {
> >     pub fn new(path: &str) -> Result<Self> {
> >         // Validation in constructor
> >         if path.is_empty() {
> >             return Err(Error::InvalidPath);
> >         }
> >
> >         // I/O in constructor
> >         let content = std::fs::read_to_string(path)?;
> >
> >         // Parsing in constructor
> >         let data: ConfigData = toml::from_str(&content)?;
> >
> >         Ok(Self { data })
> >     }
> > }
> >
> > // Problems:
> > // - Constructor can fail in multiple ways
> > // - Cannot create Config without file access
> > // - Hard to test — needs real files
> > // - File is read even if config is never used
> > ```
> >
> > <p><strong>Correct — assignment only, logic in methods:</strong></p>
> >
> > ```rust
> > impl Config {
> >     /// Creates a config that will load from the given path.
> >     /// Does not read the file — loading happens on first access.
> >     pub fn new(path: impl Into<PathBuf>) -> Self {
> >         Self {
> >             path: path.into(),
> >             cached: OnceCell::new()
> >         }
> >     }
> >
> >     /// Creates a config with pre-loaded data.
> >     /// Useful for testing or when data comes from other sources.
> >     pub fn from_data(data: ConfigData) -> Self {
> >         Self {
> >             path: PathBuf::new(),
> >             cached: OnceCell::from(data)
> >         }
> >     }
> >
> >     /// Returns the configuration data, loading from file if necessary.
> >     /// Results are cached for subsequent calls.
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
> > // Benefits:
> > // - Constructor never fails
> > // - Easy to create test configs with from_data()
> > // - Lazy loading — file read only when needed
> > // - Automatic caching — file read only once
> > ```
> >
> > </details>
>
> </details>


## 6. Delegating Constructors

> [!TIP]
>
> <p>
>   <strong>One primary constructor accepts all parameters. Other constructors delegate to it, ensuring consistent initialization.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > <p>Without a primary constructor, each constructor initializes fields independently:</p>
> > >
> > > ```rust
> > > // Problematic: field initialization duplicated
> > > impl Server {
> > >     pub fn new(addr: SocketAddr) -> Self {
> > >         Self {
> > >             addr,
> > >             tls: None,
> > >             options: ServerOptions::default(),
> > >             state: ServerState::Stopped  // duplicated
> > >         }
> > >     }
> > >
> > >     pub fn secure(addr: SocketAddr, tls: TlsConfig) -> Self {
> > >         Self {
> > >             addr,
> > >             tls: Some(tls),
> > >             options: ServerOptions::default(),
> > >             state: ServerState::Stopped  // duplicated
> > >         }
> > >     }
> > > }
> > > ```
> > >
> > > <p>Problems:</p>
> > >
> > > - Adding a field requires updating every constructor
> > > - Easy to forget initialization in one constructor
> > > - Default values may diverge between constructors
> >
> > </details>
> >
> > <details>
> > <summary><strong>Examples & Further Explanation</strong></summary>
> >
> > <p><strong>Correct — one primary, others delegate:</strong></p>
> >
> > ```rust
> > impl Server {
> >     // Primary constructor — accepts all configuration
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
> >     // Convenience constructors delegate to primary
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
> > <p><strong>Benefits:</strong></p>
> >
> > | Benefit | Explanation |
> > |---------|-------------|
> > | Single source of truth | All initialization in one place |
> > | Safety | Cannot forget to initialize fields |
> > | Consistency | All objects initialized the same way |
> > | Extensibility | Adding fields requires one change |
> > | Convenience | Easy to add new shortcuts |
> >
> > </details>
>
> </details>


## 7. Immutability First

> [!IMPORTANT]
>
> <p>
>   <strong>Prefer returning new objects over mutating existing ones. Use <code>self</code> instead of <code>&mut self</code> where practical.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > <p>Mutable objects introduce complexity:</p>
> > >
> > > - **Shared state bugs** — object modified unexpectedly
> > > - **Thread safety** — requires synchronization
> > > - **Temporal coupling** — order of operations matters
> > > - **Incomplete state** — object may be partially configured
> > >
> > > <p>Immutable objects provide:</p>
> > >
> > > - **Predictability** — object state is fixed after creation
> > > - **Thread safety** — safe to share without locks
> > > - **No temporal coupling** — operations are independent
> > > - **Atomicity** — object is always complete
> >
> > </details>
> >
> > <details>
> > <summary><strong>Examples & Further Explanation</strong></summary>
> >
> > <p><strong>Mutable approach:</strong></p>
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
> > // Usage — requires mut binding
> > let mut request = Request::new(Method::GET, url);
> > request.set_header("Content-Type", "application/json");
> > request.set_header("Authorization", token);
> > request.set_body(payload);
> > ```
> >
> > <p><strong>Immutable approach:</strong></p>
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
> > // Usage — no mut needed, fluent chain
> > let request = Request::new(Method::GET, url)
> >     .header("Content-Type", "application/json")
> >     .header("Authorization", token)
> >     .body(payload);
> > ```
> >
> > <p><strong>Benefits:</strong></p>
> >
> > | Benefit | Explanation |
> > |---------|-------------|
> > | Thread safety | No synchronization needed |
> > | Predictability | No surprise mutations |
> > | Fluent API | Natural method chaining |
> > | Debugging | State doesn't change unexpectedly |
> > | Atomicity | Object always valid or not created |
> >
> > </details>
>
> </details>

> [!NOTE]
>
> <p>
>   <strong>Some cases require <code>&mut self</code>: large data structures, I/O integration, performance-critical loops, standard traits like <code>Iterator::next</code>.</strong>
> </p>
>
> <details>
> <summary><strong>When mutability is appropriate</strong></summary>
>
> > ```rust
> > // Appropriate mutability — large data, performance critical
> > impl Document {
> >     pub fn apply_update(&mut self, update: &Update) {
> >         // Modifying large CRDT structure in place
> >         // Copying would be prohibitively expensive
> >     }
> > }
> > ```
>
> </details>


## 8. Constant Encapsulation

> [!TIP]
>
> <p>
>   <strong>Constants belong to the structures that use them, not in global scope. This improves discoverability and prevents namespace pollution.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > <p>Global constants create hidden dependencies and reduce discoverability:</p>
> > >
> > > ```rust
> > > // Global constants — scattered, no context
> > > pub const MAX_CONNECTIONS: usize = 100;
> > > pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
> > > pub const BUFFER_SIZE: usize = 8192;
> > > ```
> > >
> > > <p>Problems:</p>
> > >
> > > - **No context** — what uses `MAX_CONNECTIONS`?
> > > - **Naming conflicts** — need prefixes to disambiguate
> > > - **Hard to find** — scattered across codebase
> > > - **No documentation grouping**
> >
> > </details>
> >
> > <details>
> > <summary><strong>Examples & Further Explanation</strong></summary>
> >
> > <p><strong>Correct — constants belong to their types:</strong></p>
> >
> > ```rust
> > impl ConnectionPool {
> >     /// Maximum number of connections in the pool
> >     pub const MAX_SIZE: usize = 100;
> >
> >     /// How long to wait before retrying a failed connection
> >     pub const RECONNECT_DELAY: Duration = Duration::from_secs(1);
> > }
> >
> > impl Client {
> >     /// Default request timeout
> >     pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
> > }
> >
> > impl Buffer {
> >     /// Default buffer capacity in bytes
> >     pub const DEFAULT_CAPACITY: usize = 8192;
> > }
> >
> > // Usage is clear and discoverable
> > let pool = ConnectionPool::with_max(ConnectionPool::MAX_SIZE / 2);
> > let client = Client::new().timeout(Client::DEFAULT_TIMEOUT);
> > ```
> >
> > <p><strong>For computed values, use associated functions:</strong></p>
> >
> > ```rust
> > impl Server {
> >     // Compile-time constant
> >     pub const MAX_HEADER_SIZE: usize = 8192;
> >
> >     // Runtime "constant" — computed once
> >     pub fn default_addr() -> SocketAddr {
> >         SocketAddr::from(([0, 0, 0, 0], 8080))
> >     }
> >
> >     // Environment-dependent
> >     pub fn max_threads() -> usize {
> >         std::thread::available_parallelism()
> >             .map(|n| n.get())
> >             .unwrap_or(4)
> >     }
> > }
> > ```
> >
> > <p><strong>Benefits:</strong></p>
> >
> > | Benefit | Explanation |
> > |---------|-------------|
> > | Discoverability | Find constants where they're used |
> > | Documentation | Constants documented with their type |
> > | Namespacing | No prefix naming conventions needed |
> > | Encapsulation | Implementation details stay private |
> > | Refactoring | Easy to change without global search |
> >
> > </details>
>
> </details>


## 9. Testing with Fakes

> [!IMPORTANT]
>
> <p>
>   <strong>Use simple fake implementations instead of mock libraries. Fakes provide real behavior; mocks verify call sequences.</strong>
> </p>
>
> <details>
> <summary><strong>More information</strong></summary>
>
> > <details>
> > <summary><strong>Why is this important?</strong></summary>
> >
> > > | Aspect | Mocks | Fakes |
> > > |--------|-------|-------|
> > > | Test coupling | High — tests know implementation | Low — tests verify outcomes |
> > > | Maintenance | Breaks when implementation changes | Stable across refactoring |
> > > | Realism | Simulates interface only | Provides real behavior |
> > > | Complexity | DSL learning curve | Plain Rust code |
> > > | Debugging | Mock failures are cryptic | Standard assertions |
> >
> > </details>
> >
> > <details>
> > <summary><strong>Examples & Further Explanation</strong></summary>
> >
> > <p><strong>Mock approach — tightly coupled to implementation:</strong></p>
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
> >     // Setup expectations — coupled to implementation details
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
> >     // Problems:
> >     // - Breaks if implementation order changes
> >     // - Doesn't test actual storage behavior
> >     // - Complex setup for simple test
> > }
> > ```
> >
> > <p><strong>Fake approach — tests behavior, not implementation:</strong></p>
> >
> > ```rust
> > /// In-memory storage for testing
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
> >     /// Configure key to fail on next access
> >     fn fail_next(&self, key: &str) {
> >         self.fail_on.borrow_mut().insert(key.into());
> >     }
> >
> >     /// Check if key exists
> >     fn contains(&self, key: &str) -> bool {
> >         self.data.borrow().contains_key(key)
> >     }
> >
> >     /// Get stored value for assertions
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
> >     // Act
> >     let result = cache.get_or_fetch("user:123", || Ok(b"data".to_vec()))?;
> >
> >     // Assert actual behavior
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
> > <p><strong>Benefits of fakes:</strong></p>
> >
> > | Benefit | Explanation |
> > |---------|-------------|
> > | Behavior testing | Verify what code does, not how |
> > | Refactoring safety | Tests survive implementation changes |
> > | Reusability | One fake serves many tests |
> > | Simplicity | No mock library to learn |
> > | Debuggability | Standard Rust debugging |
> > | Documentation | Fake shows expected interface usage |
> >
> > </details>
>
> </details>

> [!NOTE]
>
> <p>
>   <strong>Mocks are appropriate when verifying specific interactions with external systems, testing that methods are NOT called, or testing complex protocols with strict ordering.</strong>
> </p>


## 10. Summary

> [!TIP]
>
> <p>
>   <strong>Quick reference table for all structural design principles.</strong>
> </p>
>
> <details>
> <summary><strong>Complete Reference</strong></summary>
>
> > | Principle | Guideline | Benefit |
> > |-----------|-----------|---------|
> > | Entity Naming | No `-er` suffixes | Natural API, encapsulation |
> > | Method Naming | Nouns for accessors, verbs for actions | Clarity, consistency |
> > | Structure Size | Maximum 4 fields | Testability, single responsibility |
> > | API Size | Maximum 5 public methods | Learnability, stability |
> > | Constructors | Assignment only, no logic | Reliability, testability |
> > | Delegation | One primary constructor | Consistency, safety |
> > | Immutability | Prefer `self` over `&mut self` | Thread safety, predictability |
> > | Constants | Encapsulate in types | Discoverability, namespacing |
> > | Testing | Fakes over mocks | Behavior verification, maintainability |
> >
> > <details>
> > <summary><strong>Code Quick Reference</strong></summary>
> >
> > ```rust
> > // Structure naming
> > struct Document;        // not DocumentManager
> > struct Connection;      // not ConnectionHandler
> >
> > // Method naming
> > fn name(&self);         // not get_name()
> > fn empty(&self);        // not is_empty()
> > fn save(&self);         // action verb
> >
> > // Structure size
> > struct User {
> >     identity: Identity,
> >     profile: Profile,
> >     access: Access       // max 4 fields
> > }
> >
> > // Constructor design
> > impl Config {
> >     fn new(path: PathBuf) -> Self {
> >         Self { path }    // assignment only
> >     }
> >
> >     fn data(&self) -> Result<&Data> {
> >         // logic here, not in constructor
> >     }
> > }
> >
> > // Immutable transformation
> > impl Request {
> >     fn header(mut self, k: &str, v: &str) -> Self {
> >         self.headers.insert(k.into(), v.into());
> >         self
> >     }
> > }
> >
> > // Encapsulated constants
> > impl Buffer {
> >     pub const DEFAULT_CAPACITY: usize = 8192;
> > }
> > ```
> >
> > </details>
>
> </details>

<p align="center">
  <em>Following these principles ensures clean, maintainable, and testable Rust code.</em>
</p>
