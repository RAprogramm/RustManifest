// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a program keeps, and what it does to what it keeps.
//!
//! A value tier says where each value came from. It does not say that a
//! program is *about* something — that it holds records and adds to
//! them, marks them, drops them, reads them out. That is the next
//! statement up, and it is derived rather than guessed: a store is a
//! field whose declared type is a collection of a type the workspace
//! declares, and each verb is read from what a method does to that very
//! field, with the site that proves it.
//!
//! Nothing here reads a name for meaning. A method called `add` that
//! touches no store is not an addition, and a method called `apply` that
//! inserts into one is.

mod answering;
mod asking;
mod ending;
mod fed;
mod outliving;
mod receiving;
mod said;
mod served;
mod store;
mod table;
mod taker;
mod telling;
mod verb;

pub use self::{
    asking::{Command, commands},
    ending::{Ending, endings},
    fed::{Feeding, feedings},
    outliving::{Direction, Outliving, outliving},
    said::purpose,
    served::{Serving, servings},
    store::{Store, stores},
    table::{TableStore, table_keeping, table_key, tables},
    telling::{Telling, telling},
    verb::{Keeping, Verb, keeping}
};

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{Direction, Verb, commands, keeping, outliving, purpose, stores, tables};
    use crate::{
        semantic::{
            behavior::behavior_with,
            settling::{Settling, settle_identity},
            workspace::Workspace
        },
        source::Source
    };

    /// What a compiler settles about these fixtures.
    ///
    /// Every question the module asks starts from a field: which type
    /// holds `tasks`, and what that field carries. The written text names
    /// neither — `self` is spelled `self` and a handler's state arrives
    /// through a pattern — so the whole reading rests on the resolver
    /// saying it. Which body each written call reaches is settled beside
    /// it, so a command reaching a store's method is an edge and not a
    /// guess.
    fn settled_over_text(sources: &[Source]) -> rustmanifest_schema::Resolution {
        let resolution = Settling::over("src/lib.rs", sources[0].text())
            .fields("self.tasks", "Store", "BTreeMap<u64, Task>")
            .rewound()
            .fields("state.tasks", "AppState", "Mutex<BTreeMap<u64, Task>>")
            .rewound()
            .fields("store.tasks", "Store", "BTreeMap<u64, Task>")
            .rewound()
            .fields("task.done", "Task", "bool")
            .rewound()
            .fields("task.title", "Task", "String")
            .rewound()
            .all_typed("self.tasks", "BTreeMap<u64, Task>")
            .rewound()
            .all_typed("state.tasks", "Mutex<BTreeMap<u64, Task>>")
            .rewound()
            .all_typed("store.tasks", "BTreeMap<u64, Task>")
            .rewound()
            .all_typed("store", "&mut Store")
            .rewound()
            .all_typed("state", "AppState")
            .rewound()
            .all_typed("tasks", "BTreeMap<u64, Task>")
            .done();
        let mut resolution = resolution;
        crate::semantic::settling::settle_declared(&mut resolution, &sources);
        crate::semantic::settling::settle_standard_macros(&mut resolution, &sources);
        crate::semantic::settling::settle_declared(&mut resolution, sources);
        crate::semantic::settling::settle_standard_macros(&mut resolution, sources);
        let facts = behavior_with(sources, Some(&resolution));
        settle_identity(&mut resolution, sources, &facts);
        resolution
    }


    const TASK_STORE: &str = r"use std::collections::BTreeMap;

pub struct Task {
    pub title: String,
    pub done:  bool
}

pub struct Store {
    pub tasks: BTreeMap<u64, Task>
}

impl Store {
    pub fn add(&mut self, title: String) -> u64 {
        let next = self.tasks.keys().last().copied().unwrap_or(0) + 1;
        self.tasks.insert(next, Task { title, done: false });
        next
    }

    pub fn complete(&mut self, id: u64) -> bool {
        match self.tasks.get_mut(&id) {
            Some(task) => {
                task.done = true;
                true
            }
            None => false
        }
    }

    pub fn drop_task(&mut self, id: u64) -> bool {
        self.tasks.remove(&id).is_some()
    }

    pub fn titles(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (_, task) in &self.tasks {
            out.push(task.title.clone());
        }
        out
    }
}
";

    fn workspace_of(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    #[test]
    fn the_sentence_names_the_records_the_verbs_and_whether_they_outlive_the_run() {
        let asking = r#"
pub fn run(store: &mut Store, asked: &str) {
    match asked {
        "add" => {
            store.add(String::new());
        }
        _ => {}
    }
}
"#;
        let sources = workspace_of(&format!("{TASK_STORE}{asking}"));
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1);
        assert!(
            said[0].contains("keeps `Task` records in `tasks` of `Store`"),
            "{said:?}"
        );
        assert!(said[0].contains("adds a record"), "{said:?}");
        assert!(said[0].contains("a run asks by name"), "{said:?}");
        assert!(
            said[0].contains("nothing carries them past the end of the run"),
            "{said:?}"
        );
    }

    #[test]
    fn a_field_holding_a_collection_of_a_declared_type_is_a_store() {
        let sources = workspace_of(TASK_STORE);
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let held = stores(&workspace);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].owner, "Store");
        assert_eq!(held[0].field, "tasks");
        assert_eq!(held[0].record, "Task");
    }

    #[test]
    fn a_collection_behind_a_lock_is_the_same_store() {
        let sources = workspace_of(
            "use std::{collections::BTreeMap, sync::{Arc, Mutex}};\n\npub struct Task {\n    pub \
             title: String,\n    pub done:  bool\n}\n\npub struct Shared {\n    pub tasks: \
             Arc<Mutex<BTreeMap<u64, Task>>>\n}\n"
        );
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let held = stores(&workspace);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].owner, "Shared");
        assert_eq!(held[0].record, "Task");
        assert_eq!(held[0].key.as_deref(), Some("u64"));
    }

    #[test]
    fn a_handler_taking_the_state_through_a_written_pattern_reads_the_records() {
        let sources = workspace_of(
            "use std::{collections::BTreeMap, sync::{Arc, Mutex}};\n\npub struct Task {\n    pub \
             title: String\n}\n\npub struct State<T>(pub T);\n\npub struct AppState {\n    pub \
             tasks: Mutex<BTreeMap<u64, Task>>\n}\n\npub fn list(State(state): \
             State<AppState>) -> usize {\n    let tasks = state.tasks.lock().unwrap();\n    \
             tasks.len()\n}\n"
        );
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let read = keeping(&workspace);
        let listed = read
            .iter()
            .find(|kept| kept.function.ends_with("list"))
            .expect("the handler must reach the store");
        assert_eq!(listed.store, "AppState.tasks");
        assert_eq!(listed.verb, Verb::Reads);
    }

    #[test]
    fn a_route_whose_handler_reaches_the_records_is_a_way_to_ask() {
        let sources = workspace_of(
            "use std::{collections::BTreeMap, sync::{Arc, Mutex}};\n\npub struct Task {\n    pub \
             title: String\n}\n\npub struct State<T>(pub T);\n\npub struct AppState {\n    pub \
             tasks: Mutex<BTreeMap<u64, Task>>\n}\n\npub fn list(State(state): \
             State<AppState>) -> usize {\n    let tasks = state.tasks.lock().unwrap();\n    \
             tasks.len()\n}\n\npub fn app() -> Router {\n    Router::new().route(\"/tasks\", \
             get(list))\n}\n"
        );
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1);
        assert!(
            said[0].contains("a request asks by route: `GET /tasks` reads the records"),
            "{said:?}"
        );
    }

    #[test]
    fn a_route_with_one_written_parameter_names_the_record_by_it() {
        let sources = workspace_of(
            "use std::{collections::BTreeMap, sync::{Arc, Mutex}};\n\npub struct Task {\n    pub \
             title: String\n}\n\npub struct State<T>(pub T);\n\npub struct Path<T>(pub \
             T);\n\npub struct AppState {\n    pub tasks: Mutex<BTreeMap<u64, \
             Task>>\n}\n\npub fn read(State(state): State<AppState>, Path(id): Path<u64>) \
             -> bool {\n    let tasks = state.tasks.lock().unwrap();\n    \
             tasks.get(&id).is_some()\n}\n\npub fn app() -> Router {\n    \
             Router::new().route(\"/tasks/{id}\", get(read))\n}\n"
        );
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1);
        assert!(
            said[0].contains(
                "`GET /tasks/{id}` reads the records named by the `{id}` the path carries"
            ),
            "{said:?}"
        );
    }

    #[test]
    fn a_path_the_lookup_never_asks_with_names_nothing() {
        let sources = workspace_of(
            "use std::{collections::BTreeMap, sync::{Arc, Mutex}};\n\npub struct Task {\n    pub \
             title: String\n}\n\npub struct State<T>(pub T);\n\npub struct Path<T>(pub \
             T);\n\npub struct AppState {\n    pub tasks: Mutex<BTreeMap<u64, \
             Task>>\n}\n\npub fn read(State(state): State<AppState>, Path(id): Path<u64>) \
             -> bool {\n    let tasks = state.tasks.lock().unwrap();\n    \
             tasks.get(&1).is_some()\n}\n\npub fn app() -> Router {\n    \
             Router::new().route(\"/tasks/{id}\", get(read))\n}\n"
        );
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1);
        assert!(
            !said[0].contains("named by"),
            "a lookup asking with a written key must not claim the path names it: {said:?}"
        );
    }

    #[test]
    fn a_written_alternative_that_reaches_the_records_is_a_command() {
        let asking = r#"
pub fn run(store: &mut Store, asked: &str) {
    match asked {
        "add" => {
            store.add(String::new());
        }
        "drop" => {
            store.drop_task(1);
        }
        _ => {}
    }
}
"#;
        let sources = workspace_of(&format!("{TASK_STORE}{asking}"));
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let asked = commands(&workspace);
        let named = |written: &str| {
            asked
                .iter()
                .find(|command| command.written == written)
                .map(|command| command.verb)
        };
        assert_eq!(named("\"add\""), Some(Verb::Adds));
        assert_eq!(named("\"drop\""), Some(Verb::Drops));
    }

    #[test]
    fn a_test_function_touching_a_store_is_not_one_of_its_methods() {
        let sources = workspace_of(&format!(
            "{TASK_STORE}\n#[cfg(test)]\nmod tests {{\n    use super::Store;\n\n    \
             #[test]\n    fn fixture_reads_the_store() {{\n        let store = Store {{ tasks: \
             std::collections::BTreeMap::new() }};\n        for (_, task) in &store.tasks {{\n    \
                     let _ = &task.title;\n        }}\n    }}\n}}\n"
        ));
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let read = keeping(&workspace);
        assert!(
            !read
                .iter()
                .any(|kept| kept.function.ends_with("fixture_reads_the_store")),
            "{read:?}"
        );
    }

    /// A program holding no collection anywhere still keeps records:
    /// the table its own statements name is the store, and each verb is
    /// the statement's kind.
    #[test]
    fn a_table_named_in_the_programs_own_statements_is_a_store() {
        let sources = workspace_of(
            "pub async fn create(pool: &sqlx::PgPool, title: String) {\n    \
             sqlx::query!(\"INSERT INTO tasks (title) VALUES ($1)\", title);\n}\npub async fn \
             list(pool: &sqlx::PgPool) {\n    sqlx::query!(\"SELECT id, title FROM tasks\");\n}\n"
        );
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let held = tables(&workspace);
        assert_eq!(held.len(), 1, "{held:?}");
        assert_eq!(held[0].table, "tasks");
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said[0].contains("keeps its records as rows of the `tasks` table"),
            "{said:?}"
        );
        assert!(said[0].contains("`create` adds a record"), "{said:?}");
        assert!(said[0].contains("`list` reads the records"), "{said:?}");
        assert!(
            said[0].contains("the rows stay in the database past the end of the run"),
            "{said:?}"
        );
    }

    /// A row-typing statement writing a workspace-declared type names
    /// the table's record; two statements writing two different types
    /// leave it unnamed.
    #[test]
    fn a_row_typing_statement_names_the_tables_record() {
        let one = workspace_of(
            "pub struct Task {\n    pub title: String\n}\n\npub async fn list(pool: \
             &sqlx::PgPool) {\n    sqlx::query_as::<_, Task>(\"SELECT id, title FROM \
             tasks\");\n}\n"
        );
        let settled = crate::semantic::settling::settled_over(&one);
        let workspace = Workspace::resolved(&one, &settled);
        let held = tables(&workspace);
        assert_eq!(held.len(), 1, "{held:?}");
        assert_eq!(held[0].record.as_deref(), Some("Task"));
        assert!(
            purpose(&workspace)[0].contains("keeps `Task` records as rows of the `tasks` table"),
            "{:?}",
            purpose(&workspace)
        );
        let two = workspace_of(
            "pub struct Task {\n    pub title: String\n}\npub struct Row {\n    pub title: \
             String\n}\n\npub async fn list(pool: &sqlx::PgPool) {\n    sqlx::query_as::<_, \
             Task>(\"SELECT title FROM tasks\");\n}\npub async fn titles(pool: &sqlx::PgPool) \
             {\n    sqlx::query_as::<_, Row>(\"SELECT title FROM tasks\");\n}\n"
        );
        let held = crate::semantic::settling::settled_over(&two);
        let disagreeing = Workspace::resolved(&two, &held);
        assert_eq!(
            tables(&disagreeing)[0].record,
            None,
            "two written row types must leave the record unnamed"
        );
    }

    /// A statement written with the very name the handler's path
    /// pattern binds names the record by it, and a statement bound with
    /// anything else claims nothing.
    #[test]
    fn a_statement_bound_with_the_path_binding_names_the_record_by_it() {
        let sources = workspace_of(
            "pub struct State<T>(pub T);\npub struct Path<T>(pub T);\n\npub async fn \
             remove(State(pool): State<sqlx::PgPool>, Path(id): Path<i64>) {\n    \
             sqlx::query!(\"DELETE FROM tasks WHERE id = $1\", id);\n}\n\npub fn app() -> Router \
             {\n    Router::new().route(\"/tasks/{id}\", delete(remove))\n}\n"
        );
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said[0].contains(
                "`DELETE /tasks/{id}` takes a record out named by the `{id}` the path carries"
            ),
            "{said:?}"
        );
    }

    /// The same table named from a body that also touches a declared
    /// field store is that store's far side, and gets no sentence of
    /// its own.
    #[test]
    fn a_table_tied_to_a_declared_store_is_not_a_second_store() {
        let sources = workspace_of(&format!(
            "{TASK_STORE}\nimpl Store {{\n    pub async fn persist(&self) {{\n        for (_, \
             task) in &self.tasks {{\n            sqlx::query!(\"INSERT INTO tasks (title) \
             VALUES ($1)\", task.title);\n        }}\n    }}\n}}\n"
        ));
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        assert!(tables(&workspace).is_empty(), "{:?}", tables(&workspace));
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1, "one store, one sentence: {said:?}");
    }

    /// A statement naming no single table builds no store, and the
    /// sentence stays silent rather than picking a plausible name.
    #[test]
    fn a_join_builds_no_table_store() {
        let sources = workspace_of(
            "pub async fn report(pool: &sqlx::PgPool) {\n    sqlx::query!(\"SELECT t.id FROM \
             tasks t JOIN users u ON TRUE\");\n}\n"
        );
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        assert!(tables(&workspace).is_empty());
        assert!(purpose(&workspace).is_empty());
    }

    #[test]
    fn a_store_written_to_a_file_outlives_the_run_that_made_it() {
        let sources = workspace_of(&format!(
            "{TASK_STORE}\nimpl Store {{\n    pub fn save(&self, path: &str) -> \
             std::io::Result<()> {{\n        let mut out = String::new();\n        for (_, task) \
             in &self.tasks {{\n            out.push_str(&task.title);\n        }}\n        \
             std::fs::write(path, out)\n    }}\n}}\n"
        ));
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let carried = outliving(&workspace);
        assert_eq!(carried.len(), 1);
        assert_eq!(carried[0].direction, Direction::Saves);
        assert!(carried[0].function.ends_with("save"));
    }

    #[test]
    fn a_store_written_to_the_database_outlives_the_run() {
        let sources = workspace_of(&format!(
            "{TASK_STORE}\nimpl Store {{\n    pub async fn persist(&self) {{\n        for (_, \
             task) in &self.tasks {{\n            sqlx::query!(\"INSERT INTO tasks (title) \
             VALUES ($1)\", task.title);\n        }}\n    }}\n}}\n"
        ));
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let carried = outliving(&workspace);
        let persisted = carried
            .iter()
            .find(|entry| entry.function.ends_with("persist"))
            .expect("the database write must carry the store past the run");
        assert_eq!(persisted.direction, Direction::Persists);
        assert_eq!(persisted.store, "Store.tasks");
    }

    #[test]
    fn a_store_written_through_the_query_function_outlives_the_run() {
        let sources = workspace_of(&format!(
            "{TASK_STORE}\nimpl Store {{\n    pub async fn persist(&self) {{\n        for (_, \
             task) in &self.tasks {{\n            sqlx::query(\"INSERT INTO tasks (title) VALUES \
             ($1)\").bind(&task.title);\n        }}\n    }}\n}}\n"
        ));
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let carried = outliving(&workspace);
        let persisted = carried
            .iter()
            .find(|entry| entry.function.ends_with("persist"))
            .expect("the query function must carry the store like the macro does");
        assert_eq!(persisted.direction, Direction::Persists);
    }

    /// The module's contract is silence for a touch that proves no verb.
    /// The earlier fallback answered "reads" instead, so a method
    /// replacing the collection with a fresh one and a method folding
    /// another store's state in were both named readers — of a method
    /// provably discarding every record, reading was claimed.
    #[test]
    fn a_touch_that_proves_no_verb_gets_none_rather_than_reads() {
        let sources = workspace_of(&format!(
            "{TASK_STORE}\nimpl Store {{\n    pub fn wipe(&mut self) {{\n        self.tasks = \
             BTreeMap::new();\n    }}\n    pub fn absorb(&mut self, other: BTreeMap<u64, Task>) \
             {{\n        self.tasks = other;\n    }}\n}}\n"
        ));
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let read = keeping(&workspace);
        for silenced in ["wipe", "absorb"] {
            assert!(
                !read.iter().any(|kept| kept.function.ends_with(silenced)),
                "a touch proving no verb must stay silent: {read:?}"
            );
        }
        assert!(
            read.iter()
                .any(|kept| kept.function.ends_with("titles") && kept.verb == Verb::Reads),
            "a read proven by its own channel must keep its verb: {read:?}"
        );
    }

    #[test]
    fn each_verb_is_read_from_what_the_method_does_to_the_store() {
        let sources = workspace_of(TASK_STORE);
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let read = keeping(&workspace);
        let verb = |name: &str| {
            read.iter()
                .find(|kept| kept.function.ends_with(name))
                .map(|kept| kept.verb)
        };
        assert_eq!(verb("add"), Some(Verb::Adds));
        assert_eq!(verb("complete"), Some(Verb::Marks));
        assert_eq!(verb("drop_task"), Some(Verb::Drops));
        assert_eq!(verb("titles"), Some(Verb::Reads));
    }
}
