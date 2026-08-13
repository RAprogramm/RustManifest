// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the engine states about the ordinary programs of the ladder.
//!
//! A greeting, a calculator, a create-read-update-delete store, a task
//! list, the same store served over the wire, the same store with a
//! database behind it, and the server straight over the database with
//! no collection anywhere: the ladder the meaning and keeping tiers are
//! measured on. The first rung is covered where
//! the sentence is built; the rest are covered here, because what they
//! prove is not one recognizer but the whole chain — a value read from
//! outside the process, carried through a call, computed on, and
//! written out or served.
//!
//! Every assertion below failed before the work it guards. They are the
//! only record that it holds: a run in a terminal is gone the moment the
//! terminal is.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::path::PathBuf;

use rustmanifest_engine::{Narrative, Source, Workspace, program_meaning, purpose};

/// What a compiler settled about one fixture, written down.
///
/// Every reading of the engine answers from a resolution and from nothing
/// else: what a value is, which type holds a field, what a call hands
/// back. A fixture asking what a whole program means therefore carries the
/// compiler's answers beside its text. Each is named by the written
/// fragment it stands over and holds at every place the fixture writes it
/// — these programs write a name for one thing throughout.
struct Settled {
    /// File the fragments stand in.
    file: PathBuf,
    /// Text of that file.
    text: String,
    /// Offset the fragments named next are looked for from.
    from: usize,
    /// What has been written down so far.
    held: rustmanifest_schema::Resolution
}

impl Settled {
    /// Starts writing down what a compiler settled about `text`.
    fn over(path: &str, text: &str) -> Self {
        Self {
            file: PathBuf::from(path),
            text: text.to_owned(),
            from: 0,
            held: rustmanifest_schema::Resolution {
                resolver: "fixture".to_owned(),
                ..rustmanifest_schema::Resolution::default()
            }
        }
    }

    /// Reads the fragments named next from after `marker`, so a name one
    /// body writes for one thing and another writes for another is stated
    /// once per body rather than once for the file.
    fn after(mut self, marker: &str) -> Self {
        let at = self.text[self.from..]
            .find(marker)
            .expect("the fixture writes the marker from here on");
        self.from += at + marker.len();
        self
    }

    /// Reads the fragments named next from the start of the file again.
    const fn rewound(mut self) -> Self {
        self.from = 0;
        self
    }

    /// Every place `fragment` is written from the cursor on, as a
    /// location.
    fn places(&self, fragment: &str) -> Vec<rustmanifest_schema::Location> {
        let mut found = Vec::new();
        let mut cursor = self.from;
        while let Some(at) = self.text[cursor..].find(fragment) {
            let start = cursor + at;
            cursor = start + fragment.len();
            found.push(rustmanifest_schema::Location {
                file:  self.file.clone(),
                range: rustmanifest_schema::Range {
                    start: u32::try_from(start).expect("a fixture is small"),
                    end:   u32::try_from(cursor).expect("a fixture is small")
                }
            });
        }
        assert!(!found.is_empty(), "the fixture writes {fragment:?} nowhere");
        found
    }

    /// States the type the compiler settled for everything written as
    /// `fragment`.
    fn typed(mut self, fragment: &str, rendered: &str) -> Self {
        for at in self.places(fragment) {
            self.held.types.push(rustmanifest_schema::ResolvedType {
                at,
                rendered: rendered.to_owned()
            });
        }
        self
    }

    /// States the type holding every field written as `fragment` and the
    /// type that field carries.
    fn field(mut self, fragment: &str, owner: &str, produced: &str) -> Self {
        for at in self.places(fragment) {
            self.held.fields.push(rustmanifest_schema::ResolvedField {
                at,
                owner: owner.to_owned(),
                produced: produced.to_owned()
            });
        }
        self
    }

    /// States what every call written as `fragment` hands back and the
    /// type of the receiver it stands on.
    fn call(mut self, fragment: &str, callee: &str, returns: &str, receiver: Option<&str>) -> Self {
        for at in self.places(fragment) {
            self.held.calls.push(rustmanifest_schema::ResolvedCall {
                at,
                callee: callee.to_owned(),
                returns: returns.to_owned(),
                receiver: receiver.map(ToOwned::to_owned),
                receiver_path: None,
                reaches: None,
                takes: None
            });
        }
        self
    }

    /// Hands back the resolution, in the order every lookup bisects.
    fn done(mut self) -> rustmanifest_schema::Resolution {
        self.held.settle();
        self.held
    }
}

/// The whole statement the engine makes about the entry point of
/// `sources`, licensed clauses and refused one alike.
fn stated(sources: &[(&str, &str)]) -> String {
    stated_with(sources, &rustmanifest_schema::Resolution::default())
}

/// The same, read with what a compiler settled about the fixture.
fn stated_with(sources: &[(&str, &str)], settled: &rustmanifest_schema::Resolution) -> String {
    let owned: Vec<Source> = sources
        .iter()
        .map(|(path, body)| Source::new(PathBuf::from(path), (*body).to_owned()))
        .collect();
    program_meaning(&Workspace::resolved(&owned, settled))
        .first()
        .map(|entry| match &entry.narrative {
            Narrative::Whole(said) => said.clone(),
            Narrative::Parts {
                refused: (clause, gap),
                licensed
            } => format!("[{clause} refused: {gap}] {}", licensed.join(" "))
        })
        .unwrap_or_default()
}

/// An ordinary three-argument calculator: two operands read off the
/// command line through a helper, an operator chosen among five written
/// alternatives, the result printed.
const CALCULATOR: &str = "use std::{env, num::ParseIntError, process::ExitCode};\n\
    fn operand(args: &[String], position: usize) -> Result<i64, ParseIntError> {\n\
    \x20   args[position].parse::<i64>()\n\
    }\n\
    fn main() -> ExitCode {\n\
    \x20   let args: Vec<String> = env::args().collect();\n\
    \x20   let left = match operand(&args, 1) {\n\
    \x20       Ok(value) => value,\n\
    \x20       Err(_) => return ExitCode::from(2),\n\
    \x20   };\n\
    \x20   let right = match operand(&args, 3) {\n\
    \x20       Ok(value) => value,\n\
    \x20       Err(_) => return ExitCode::from(2),\n\
    \x20   };\n\
    \x20   let result = match args[2].as_str() {\n\
    \x20       \"+\" => left + right,\n\
    \x20       \"-\" => left - right,\n\
    \x20       \"*\" => left * right,\n\
    \x20       \"/\" => left / right,\n\
    \x20       _ => return ExitCode::from(2),\n\
    \x20   };\n\
    \x20   println!(\"{result}\");\n\
    \x20   ExitCode::SUCCESS\n\
    }\n";

/// The same store as a tutorial actually writes it: commands off the
/// command line, a fallback where none was given, a file behind it.
const TUTORIAL: &str = "use std::{env, fs, io::{self, Write}, process::ExitCode};\n\
    fn save(path: &str, done: bool) -> io::Result<()> { fs::write(path, \
    \x20   format!(\"{done}\")) }\n\
    fn main() -> ExitCode {\n\
    \x20   let args: Vec<String> = env::args().collect();\n\
    \x20   let command = args.get(1).map(String::as_str).unwrap_or(\"list\");\n\
    \x20   let id = args.get(2).and_then(|raw| raw.parse::<usize>().ok()).unwrap_or(0);\n\
    \x20   let stdout = io::stdout();\n\
    \x20   let mut handle = stdout.lock();\n\
    \x20   let _ = writeln!(handle, \"{command} {id}\");\n\
    \x20   match save(\"todo.txt\", true) {\n\
    \x20       Ok(()) => ExitCode::SUCCESS,\n\
    \x20       Err(_) => ExitCode::from(2),\n\
    \x20   }\n\
    }\n";

/// A store with the four operations of a tutorial, written to a handle
/// rather than to the stream macro.
const STORE: &str = "use std::{collections::BTreeMap, io::{self, Write}, process::ExitCode};\n\
    struct Task { done: bool }\n\
    struct Store { tasks: BTreeMap<u64, Task> }\n\
    impl Store {\n\
    \x20   fn new() -> Self { Self { tasks: BTreeMap::new() } }\n\
    \x20   fn create(&mut self, id: u64) { self.tasks.insert(id, Task { done: false }); }\n\
    \x20   fn read(&self, id: u64) -> Option<&Task> { self.tasks.get(&id) }\n\
    \x20   fn update(&mut self, id: u64) -> bool {\n\
    \x20       match self.tasks.get_mut(&id) { Some(task) => { task.done = true; true } None => \
                     false }\n\
    \x20   }\n\
    \x20   fn delete(&mut self, id: u64) -> bool { self.tasks.remove(&id).is_some() }\n\
    }\n\
    fn main() -> ExitCode {\n\
    \x20   let mut store = Store::new();\n\
    \x20   store.create(1);\n\
    \x20   store.update(1);\n\
    \x20   let listed = store.read(1).is_some();\n\
    \x20   store.delete(1);\n\
    \x20   let stdout = io::stdout();\n\
    \x20   let mut handle = stdout.lock();\n\
    \x20   let _ = writeln!(handle, \"{listed}\");\n\
    \x20   ExitCode::SUCCESS\n\
    }\n";

/// The command line reaches the arithmetic through a binding that carries
/// a written type and through a helper call. Either link broken, the
/// sentence falls back to naming a value it did not derive.
/// A task list printing every entry of a map it iterates.
const TASK_LIST: &str = r#"use std::collections::BTreeMap;

struct Task {
    title: String,
    done:  bool
}

struct Store {
    tasks: BTreeMap<u64, Task>
}

fn main() {
    let store = Store { tasks: BTreeMap::new() };
    for (id, task) in &store.tasks {
        println!("{id} {} {}", task.done, task.title);
    }
}
"#;

/// A program with a helper nothing calls, whose reading refuses.
const UNREACHED_HELPER: &str = r#"fn unused(values: Vec<u8>) -> u8 {
    let mut total = 0;
    for value in values {
        total = value;
    }
    total
}

fn main() {
    println!("ready");
}
"#;

/// The first program anyone writes.
const GREETING: &str = r#"fn main() {
    println!("hello world");
}
"#;

/// The next rung after the task list: the same store served over the
/// wire — a shared map behind a lock, four handlers taking the state
/// through the extractor pattern, a router naming the paths.
const API_SERVER: &str = r#"use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex}
};

struct Task {
    title: String,
    done:  bool
}

struct AppState {
    tasks: Mutex<BTreeMap<u64, Task>>
}

async fn list(State(state): State<Arc<AppState>>) -> String {
    let tasks = state.tasks.lock().unwrap();
    let mut out = String::new();
    for (id, task) in tasks.iter() {
        out.push_str(&format!("{id} {} {}\n", task.done, task.title));
    }
    out
}

async fn read(State(state): State<Arc<AppState>>, Path(id): Path<u64>) -> String {
    let tasks = state.tasks.lock().unwrap();
    match tasks.get(&id) {
        Some(task) => format!("{id} {} {}\n", task.done, task.title),
        None => "not found\n".to_owned()
    }
}

async fn create(State(state): State<Arc<AppState>>, title: String) -> String {
    let mut tasks = state.tasks.lock().unwrap();
    let id = tasks.keys().last().copied().unwrap_or(0) + 1;
    tasks.insert(id, Task { title, done: false });
    format!("{id}\n")
}

async fn remove(State(state): State<Arc<AppState>>, Path(id): Path<u64>) -> String {
    let mut tasks = state.tasks.lock().unwrap();
    match tasks.remove(&id) {
        Some(_) => "removed\n".to_owned(),
        None => "not found\n".to_owned()
    }
}

fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/tasks", get(list).post(create))
        .route("/tasks/{id}", get(read).delete(remove))
        .with_state(state)
}
"#;

/// What the keeping tier says about `sources`, one sentence per store,
/// read with what a compiler settled about them.
fn kept_with(sources: &[(&str, &str)], settled: &rustmanifest_schema::Resolution) -> Vec<String> {
    let owned: Vec<Source> = sources
        .iter()
        .map(|(path, body)| Source::new(PathBuf::from(path), (*body).to_owned()))
        .collect();
    purpose(&Workspace::resolved(&owned, settled))
}

/// The same, over sources no resolver ran on.
fn kept(sources: &[(&str, &str)]) -> Vec<String> {
    kept_with(sources, &rustmanifest_schema::Resolution::default())
}

/// The task list a junior writes: read the command line, load a file,
/// add, complete, list, save.
const TODO: &str = r#"use std::{collections::BTreeMap, env, fs, io, process::ExitCode};

struct Task {
    title: String,
    done:  bool
}

struct Store {
    path:  String,
    tasks: BTreeMap<u64, Task>
}

impl Store {
    fn load(path: String) -> io::Result<Self> {
        let text = fs::read_to_string(&path)?;
        let mut tasks = BTreeMap::new();
        for line in text.lines() {
            let mut parts = line.splitn(3, '\t');
            let id = parts.next().unwrap_or("0");
            let done = parts.next().unwrap_or("0");
            let title = parts.next().unwrap_or("");
            let number = id.parse::<u64>().unwrap_or(0);
            tasks.insert(number, Task {
                title: title.to_owned(),
                done:  done == "1"
            });
        }
        Ok(Self { path, tasks })
    }

    fn add(&mut self, title: String) -> u64 {
        let next = self.tasks.keys().last().copied().unwrap_or(0) + 1;
        self.tasks.insert(next, Task { title, done: false });
        next
    }

    fn complete(&mut self, id: u64) -> bool {
        match self.tasks.get_mut(&id) {
            Some(task) => {
                task.done = true;
                true
            }
            None => false
        }
    }

    fn save(&self) -> io::Result<()> {
        let mut out = String::new();
        for (id, task) in &self.tasks {
            out.push_str(&format!("{id}\t{}\t{}\n", u8::from(task.done), task.title));
        }
        fs::write(&self.path, out)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("list");
    let mut store = match Store::load("tasks.tsv".to_owned()) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("cannot read the task file: {error}");
            return ExitCode::FAILURE;
        }
    };
    match command {
        "add" => {
            let title = args.get(2).cloned().unwrap_or_default();
            let id = store.add(title);
            println!("added task {id}");
        }
        "done" => {
            let id = args.get(2).and_then(|text| text.parse::<u64>().ok()).unwrap_or(0);
            if store.complete(id) {
                println!("task {id} is done");
            } else {
                println!("no task {id}");
            }
        }
        _ => {
            for (id, task) in &store.tasks {
                println!("{id} {} {}", if task.done { "x" } else { " " }, task.title);
            }
        }
    }
    if let Err(error) = store.save() {
        eprintln!("cannot write the task file: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
"#;

/// The rung after the server: the same task list with a database behind
/// it — the store is rebuilt from rows at the start of the run and
/// written back as rows at its end, so the records outlive the process
/// in both directions.
const DATABASE_TODO: &str = r#"use std::{collections::BTreeMap, env, process::ExitCode};

struct Task {
    title: String,
    done:  bool
}

struct Store {
    tasks: BTreeMap<u64, Task>
}

impl Store {
    async fn restore(pool: &sqlx::PgPool) -> Result<Self, sqlx::Error> {
        let rows = sqlx::query!("SELECT id, title, done FROM tasks")
            .fetch_all(pool)
            .await?;
        let mut tasks = BTreeMap::new();
        for row in rows {
            tasks.insert(row.id, Task {
                title: row.title,
                done:  row.done
            });
        }
        Ok(Self { tasks })
    }

    fn add(&mut self, title: String) -> u64 {
        let next = self.tasks.keys().last().copied().unwrap_or(0) + 1;
        self.tasks.insert(next, Task { title, done: false });
        next
    }

    fn complete(&mut self, id: u64) -> bool {
        match self.tasks.get_mut(&id) {
            Some(task) => {
                task.done = true;
                true
            }
            None => false
        }
    }

    async fn persist(&self, pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
        sqlx::query!("DELETE FROM tasks").execute(pool).await?;
        for (id, task) in &self.tasks {
            sqlx::query!(
                "INSERT INTO tasks (id, title, done) VALUES ($1, $2, $3)",
                id,
                task.title,
                task.done
            )
            .execute(pool)
            .await?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("list");
    let pool = match sqlx::PgPool::connect("postgres://localhost/todo").await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("cannot reach the database: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut store = match Store::restore(&pool).await {
        Ok(store) => store,
        Err(error) => {
            eprintln!("cannot load the tasks: {error}");
            return ExitCode::FAILURE;
        }
    };
    match command {
        "add" => {
            let title = args.get(2).cloned().unwrap_or_default();
            let id = store.add(title);
            println!("added task {id}");
        }
        "done" => {
            let id = args.get(2).and_then(|text| text.parse::<u64>().ok()).unwrap_or(0);
            if store.complete(id) {
                println!("task {id} is done");
            } else {
                println!("no task {id}");
            }
        }
        _ => {
            for (id, task) in &store.tasks {
                println!("{id} {} {}", if task.done { "x" } else { " " }, task.title);
            }
        }
    }
    if let Err(error) = store.persist(&pool).await {
        eprintln!("cannot save the tasks: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
"#;

/// What a compiler settles about the database rung: the map behind the
/// field, the record each row fills, and what every call written on the
/// store stands on.
fn database_todo_settled() -> rustmanifest_schema::Resolution {
    Settled::over("src/main.rs", DATABASE_TODO)
        .field("self.tasks", "Store", "BTreeMap<u64, Task>")
        .field("task.done", "Task", "bool")
        .field("task.title", "Task", "String")
        .typed("self.tasks", "BTreeMap<u64, Task>")
        .typed("store", "Store")
        .typed("title", "String")
        .typed("command", "&str")
        .after("|text|")
        .typed("text", "&String")
        .rewound()
        .call("text.parse::<u64>()", "parse", "Result<u64, ParseIntError>", Some("&String"))
        .call("store.add(title)", "add", "u64", Some("&mut Store"))
        .call("store.complete(id)", "complete", "bool", Some("&mut Store"))
        .done()
}

/// The database rung: the store is the same map the file-backed rungs
/// keep, and the sentence must say both ways it crosses the end of the
/// run — filled from rows at the start, written back as rows at the
/// end — with each way proven by the query that performs it.
#[test]
fn the_database_todo_states_both_ways_its_records_outlive_the_run() {
    let said = kept_with(&[("src/main.rs", DATABASE_TODO)], &database_todo_settled());
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("keeps `Task` records in `tasks` of `Store`, each found by a `u64`"),
        "{said:?}"
    );
    assert!(
        said[0].contains("`Store::add` adds a record"),
        "the insert into the map is not read: {said:?}"
    );
    assert!(
        said[0].contains("`Store::persist` writes the records to the database"),
        "the insert query does not carry the store past the run: {said:?}"
    );
    assert!(
        said[0].contains("`Store::restore` fills the records from the database"),
        "the select query does not fill the store from an earlier run: {said:?}"
    );
    assert!(said[0].contains("so they outlive the run"), "{said:?}");
    assert!(
        !said[0].contains("nothing carries them past the end of the run"),
        "a database-backed store must not claim to die with the process: {said:?}"
    );
}

/// The same rung, the clauses around the store: each command a run asks
/// by carries its payload traced to the command line, and the run's
/// endings are stated. The title of `"add"` falls back to the type's
/// default where no argument was given, and the sentence names that
/// declaration without claiming its value — the flow table's one
/// admitted row about something written elsewhere.
#[test]
fn the_database_todo_traces_its_command_key_and_states_its_endings() {
    let said = kept_with(&[("src/main.rs", DATABASE_TODO)], &database_todo_settled());
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains(
            "\"add\" adds a record named by one of position 2 of the command-line and the \
             default its type declares"
        ),
        "the title the command carries is not traced through the declared-default fallback: \
         {said:?}"
    );
    assert!(
        said[0].contains(
            "\"done\" changes a record named by one of the result of conversion to u64 over \
             position 2 of the command-line and the written value 0"
        ),
        "the key the command hands over is not traced to the command line: {said:?}"
    );
    assert!(
        said[0].contains("the run can end with failure, can end with success"),
        "{said:?}"
    );
}

/// The rung above both: the server straight over the database. No
/// collection anywhere — the records live in rows, and the only place
/// their home is written is the SQL text of the handlers themselves.
const API_OVER_DATABASE: &str = r#"use std::sync::Arc;

struct Task {
    id:    i64,
    title: String,
    done:  bool
}

async fn list(State(pool): State<Arc<sqlx::PgPool>>) -> String {
    let mut out = String::new();
    let rows = sqlx::query_as::<_, Task>("SELECT id, title, done FROM tasks ORDER BY id")
        .fetch_all(pool.as_ref())
        .await
        .unwrap();
    for row in rows {
        out.push_str(&format!("{} {} {}\n", row.id, row.done, row.title));
    }
    out
}

async fn create(State(pool): State<Arc<sqlx::PgPool>>, title: String) -> String {
    sqlx::query!("INSERT INTO tasks (title, done) VALUES ($1, FALSE)", title)
        .execute(pool.as_ref())
        .await
        .unwrap();
    "created\n".to_owned()
}

async fn complete(State(pool): State<Arc<sqlx::PgPool>>, Path(id): Path<i64>) -> String {
    sqlx::query!("UPDATE tasks SET done = TRUE WHERE id = $1", id)
        .execute(pool.as_ref())
        .await
        .unwrap();
    "done\n".to_owned()
}

async fn remove(State(pool): State<Arc<sqlx::PgPool>>, Path(id): Path<i64>) -> String {
    sqlx::query!("DELETE FROM tasks WHERE id = $1", id)
        .execute(pool.as_ref())
        .await
        .unwrap();
    "removed\n".to_owned()
}

fn app(pool: Arc<sqlx::PgPool>) -> Router {
    Router::new()
        .route("/tasks", get(list).post(create))
        .route("/tasks/{id}", put(complete).delete(remove))
        .with_state(pool)
}
"#;

/// The database-server rung: a program holding no collection still
/// states what it keeps — the table its handlers' own statements name —
/// with each verb the statement's kind and each route joined to the
/// handler that performs it.
#[test]
fn the_server_over_the_database_states_its_table_its_verbs_and_its_routes() {
    let said = kept(&[("src/main.rs", API_OVER_DATABASE)]);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains(
            "keeps `Task` records as rows of the `tasks` table, named in its own statements"
        ),
        "a program with no collection anywhere must still state its store and its row type: \
         {said:?}"
    );
    assert!(said[0].contains("`create` adds a record"), "{said:?}");
    assert!(said[0].contains("`complete` changes a record"), "{said:?}");
    assert!(said[0].contains("`remove` takes a record out"), "{said:?}");
    assert!(said[0].contains("`list` reads the records"), "{said:?}");
    assert!(
        said[0].contains("`GET /tasks` reads the records"),
        "the route is not joined to the statement behind it: {said:?}"
    );
    assert!(said[0].contains("`POST /tasks` adds a record"), "{said:?}");
    assert!(
        said[0]
            .contains("`PUT /tasks/{id}` changes a record named by the `{id}` the path carries"),
        "the statement's own bind must tie the route's parameter to the record: {said:?}"
    );
    assert!(
        said[0].contains(
            "`DELETE /tasks/{id}` takes a record out named by the `{id}` the path carries"
        ),
        "{said:?}"
    );
    assert!(
        said[0].contains("the rows stay in the database past the end of the run"),
        "{said:?}"
    );
}

/// The server rung: the whole sentence, every clause standing on its
/// own proof — the store behind the lock, the verbs of the four
/// handlers, the four routes, and the record a one-parameter path
/// names.
#[test]
fn the_api_server_states_its_store_its_verbs_and_its_routes() {
    let settled = Settled::over("src/main.rs", API_SERVER)
        .field("state.tasks", "AppState", "Mutex<BTreeMap<u64, Task>>")
        .field("task.done", "Task", "bool")
        .field("task.title", "Task", "String")
        .typed("state.tasks", "Mutex<BTreeMap<u64, Task>>")
        .typed("tasks", "BTreeMap<u64, Task>")
        .typed("state", "Arc<AppState>")
        .done();
    let said = kept_with(&[("src/main.rs", API_SERVER)], &settled);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("keeps `Task` records in `tasks` of `AppState`, each found by a `u64`"),
        "{said:?}"
    );
    assert!(
        said[0].contains("`create` adds a record"),
        "the insert through the opened lock is not read: {said:?}"
    );
    assert!(
        said[0].contains("`remove` takes a record out"),
        "the removal through the opened lock is not read: {said:?}"
    );
    assert!(
        said[0].contains("`GET /tasks` reads the records"),
        "{said:?}"
    );
    assert!(said[0].contains("`POST /tasks` adds a record"), "{said:?}");
    assert!(
        said[0].contains(
            "`DELETE /tasks/{id}` takes a record out named by the `{id}` the path carries"
        ),
        "{said:?}"
    );
    assert!(
        said[0].contains("nothing carries them past the end of the run"),
        "an in-memory map must not claim to outlive the run: {said:?}"
    );
}

#[test]
fn the_greeting_states_what_it_writes_and_nothing_more() {
    let stated = stated(&[("src/main.rs", GREETING)]);
    assert!(
        stated.contains("unconditionally writes \"hello world\\n\" to stdout"),
        "the greeting is not stated: {stated}"
    );
    assert!(
        !stated.contains("did not derive"),
        "the greeting refuses somewhere: {stated}"
    );
}

/// What a compiler settles about the task-list rung: the map behind the
/// field, the record each entry holds, and what every call written on
/// them stands on and hands back.
fn todo_settled() -> rustmanifest_schema::Resolution {
    Settled::over("src/main.rs", TODO)
        .field("self.tasks", "Store", "BTreeMap<u64, Task>")
        .field("self.path", "Store", "String")
        .field("task.done", "Task", "bool")
        .field("task.title", "Task", "String")
        .typed("self.tasks", "BTreeMap<u64, Task>")
        .typed("store", "Store")
        .typed("title", "String")
        .typed("command", "&str")
        .after("|text|")
        .typed("text", "&String")
        .rewound()
        .call("text.parse::<u64>()", "parse", "Result<u64, ParseIntError>", Some("&String"))
        .call("id.parse::<u64>()", "parse", "Result<u64, ParseIntError>", Some("&str"))
        .call("store.add(title)", "add", "u64", Some("&mut Store"))
        .call("store.complete(id)", "complete", "bool", Some("&mut Store"))
        .call("store.save()", "save", "Result<(), io::Error>", Some("&Store"))
        .done()
}

#[test]
fn the_task_list_traces_every_position_it_prints() {
    let stated = stated_with(&[("src/main.rs", TODO)], &todo_settled());
    assert!(
        !stated.contains("<id>` is a value this analysis did not derive"),
        "the identifier the task list prints is not traced: {stated}"
    );
    assert!(
        stated.contains("conversion to u64 over position 2 of the command-line"),
        "the identifier is not traced to the command line: {stated}"
    );
    assert!(
        stated.contains("takes position 1 of its command-line"),
        "the command it was asked for is not stated: {stated}"
    );
    assert!(
        stated.contains("`<error>` is the value `fs::read_to_string` hands back"),
        "the failure it prints is not traced to the call that failed: {stated}"
    );
}

#[test]
fn a_refusal_nothing_reaches_leaves_the_statement_whole() {
    let stated = stated(&[("src/main.rs", UNREACHED_HELPER)]);
    assert!(
        stated.contains("the process unconditionally writes \"ready\\n\" to stdout"),
        "the write is not stated: {stated}"
    );
    assert!(
        !stated.starts_with("this analysis"),
        "a statement broke on a site no path reaches: {stated}"
    );
}

#[test]
fn the_task_list_states_what_it_prints_without_refusing_anywhere() {
    let settled = Settled::over("src/main.rs", TASK_LIST)
        .field("store.tasks", "Store", "BTreeMap<u64, Task>")
        .field("task.done", "Task", "bool")
        .field("task.title", "Task", "String")
        .typed("store.tasks", "BTreeMap<u64, Task>")
        .typed("id", "&u64")
        .typed("task", "&Task")
        .done();
    let stated = stated_with(&[("src/main.rs", TASK_LIST)], &settled);
    assert!(
        !stated.contains("did not derive"),
        "the statement refuses somewhere: {stated}"
    );
    assert!(
        stated.contains("the result of selection over the field `Store.tasks`"),
        "the key a map iteration hands out must trace to the map it came from: {stated}"
    );
}

#[test]
fn a_name_the_body_rewrites_refuses_while_its_neighbour_traces_home() {
    let stated = stated(&[(
        "src/main.rs",
        "fn source() -> Result<u8, u8> { Ok(1) }\n\nfn after_loop(values: Vec<u8>) -> u8 {\n    \
         let mut last = 0;\n    for value in values {\n        last = value;\n    }\n    \
         last\n}\n\nfn after_condition(fallback: u8) -> u8 {\n    if let Ok(value) = source() \
         {\n        return value;\n    }\n    fallback\n}\n\nfn main() {\n    let taken = \
         after_loop(vec![1, 2]);\n    let chosen = after_condition(7);\n    println!(\"{taken} \
         {chosen}\");\n}\n"
    )]);
    assert!(
        stated.contains("(rebound)"),
        "a name the body rewrites must refuse by name: {stated}"
    );
    assert!(
        stated.contains("one of the written value 1 and the written value 7"),
        "a value taken out of a callee's answer must trace to what the callee wrote: {stated}"
    );
}

/// What a compiler settles about the calculator rung: the sequence the
/// command line hands over, the text one position of it holds, and what
/// the helper answers with.
fn calculator_settled() -> rustmanifest_schema::Resolution {
    Settled::over("app/src/main.rs", CALCULATOR)
        .typed("args[position]", "String")
        .typed("args[2]", "String")
        .typed("left", "i64")
        .typed("right", "i64")
        .typed("value", "i64")
        .typed("result", "i64")
        .call("args[position].parse::<i64>()", "parse", "Result<i64, ParseIntError>", Some("String"))
        .call("args[2].as_str()", "as_str", "&str", Some("String"))
        .call("operand(&args, 1)", "operand", "Result<i64, ParseIntError>", None)
        .call("operand(&args, 3)", "operand", "Result<i64, ParseIntError>", None)
        .done()
}

#[test]
fn the_calculator_traces_its_result_back_to_the_command_line() {
    let said = stated_with(&[("app/src/main.rs", CALCULATOR)], &calculator_settled());
    assert!(
        said.contains("takes whole of its command-line"),
        "the intake is not stated: {said}"
    );
    assert!(
        said.contains(
            "conversion to i64 over the result of selection over whole of the \
                       command-line"
        ),
        "the chain from the command line to the number is broken: {said}"
    );
    assert!(
        said.contains("the result of addition over the result of conversion to i64"),
        "the arithmetic does not name what it operates on: {said}"
    );
    assert!(
        !said.contains("did not derive"),
        "the sentence still refuses somewhere: {said}"
    );
}

/// A `match` over the two halves of a fallible result is exhaustive by the
/// type, and the alternative that returns produces no value at all.
#[test]
fn the_calculator_states_its_choices_are_closed_and_that_one_arm_leaves() {
    let said = stated_with(&[("app/src/main.rs", CALCULATOR)], &calculator_settled());
    assert!(
        said.contains("which name every variant of a standard type"),
        "an exhaustive choice is still called unproven: {said}"
    );
    assert!(
        said.contains("of which one accepts every remaining value"),
        "the catch-all choice is not recognised: {said}"
    );
}

/// The written value reaches the handle-taking macro exactly as it reaches
/// the stream macro, a library predicate keeps the origin of its receiver,
/// and a library access keeps the collection the element came out of. The
/// whole chain is derived: no link is a refusal.
#[test]
fn the_store_traces_the_written_value_to_the_collection_it_came_from() {
    let said = stated_with(&[("app/src/main.rs", STORE)], &store_settled());
    assert!(
        said.contains(
            "the result of a check that a value is held over the result of selection over the \
             field `Store.tasks` and the written value 1"
        ),
        "the chain from the write to the collection is broken: {said}"
    );
    assert!(
        !said.contains("did not derive"),
        "the sentence still refuses somewhere: {said}"
    );
}

/// What a compiler settles about the store rung: the map behind the
/// field, the type each call stands on, and what each hands back.
fn store_settled() -> rustmanifest_schema::Resolution {
    Settled::over("app/src/main.rs", STORE)
        .field("self.tasks", "Store", "BTreeMap<u64, Task>")
        .field("task.done", "Task", "bool")
        .typed("self.tasks", "BTreeMap<u64, Task>")
        .typed("store", "Store")
        .typed("listed", "bool")
        .call("store.read(1)", "read", "Option<&Task>", Some("&Store"))
        .call("store.read(1).is_some()", "is_some", "bool", Some("Option<&Task>"))
        .call("self.tasks.get(&id)", "get", "Option<&Task>", Some("BTreeMap<u64, Task>"))
        .done()
}

/// The store touches the outside world only through the write it makes,
/// and the engine must not invent more than that.
#[test]
fn the_store_states_the_status_it_leaves_and_claims_nothing_further() {
    let said = stated(&[("app/src/main.rs", STORE)]);
    assert!(
        said.contains("the entry returns a status"),
        "the exit is not stated: {said}"
    );
    assert!(
        !said.contains("command-line"),
        "an intake nothing performs is claimed: {said}"
    );
}

/// A written position singles one element out; what follows transforms
/// that element and cannot change which one it was. Degrading the
/// selector there cost the whole intake clause its licence.
/// What a compiler settles about the tutorial rung: the text one
/// position of the command line holds, and what the closure written
/// beside the combinator stands on.
fn tutorial_settled() -> rustmanifest_schema::Resolution {
    Settled::over("app/src/main.rs", TUTORIAL)
        .typed("raw", "&String")
        .typed("command", "&str")
        .typed("id", "usize")
        .call("raw.parse::<usize>()", "parse", "Result<usize, ParseIntError>", Some("&String"))
        .done()
}

#[test]
fn the_tutorial_keeps_the_position_it_read_through_later_adapters() {
    let said = stated_with(&[("app/src/main.rs", TUTORIAL)], &tutorial_settled());
    assert!(
        said.contains("takes position 1 of its command-line"),
        "the position the source settled is lost: {said}"
    );
}

/// A fallback is a choice the source really makes, and the side it falls
/// back to is written right there.
#[test]
fn the_tutorial_states_both_sides_of_its_fallback() {
    let said = stated_with(&[("app/src/main.rs", TUTORIAL)], &tutorial_settled());
    assert!(
        said.contains("the written value \"list\""),
        "the fallback the source writes is not named: {said}"
    );
}

/// A combinator answers with what the closure written beside it hands
/// back, and that closure's own input stands for what the receiver holds.
/// Both steps are the library's guarantee, so the chain from the command
/// line through the conversion is stated whole.
#[test]
fn the_tutorial_reads_through_the_closure_written_beside_a_combinator() {
    let said = stated_with(&[("app/src/main.rs", TUTORIAL)], &tutorial_settled());
    assert!(
        said.contains("conversion to usize over position 2 of the command-line"),
        "the chain through the closure is broken: {said}"
    );
}
