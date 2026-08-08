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
mod outliving;
mod receiving;
mod said;
mod served;
mod store;
mod taker;
mod telling;
mod verb;

pub use self::{
    asking::{Command, commands},
    ending::{Ending, endings},
    outliving::{Direction, Outliving, outliving},
    said::purpose,
    served::{Serving, servings},
    store::{Store, stores},
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

    use super::{Direction, Verb, commands, keeping, outliving, purpose, stores};
    use crate::{semantic::workspace::Workspace, source::Source};

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
        let workspace = Workspace::new(&sources);
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
        let workspace = Workspace::new(&sources);
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
        let workspace = Workspace::new(&sources);
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
             State<Arc<AppState>>) -> usize {\n    let tasks = state.tasks.lock().unwrap();\n    \
             tasks.len()\n}\n"
        );
        let workspace = Workspace::new(&sources);
        let read = keeping(&workspace);
        let listed = read
            .iter()
            .find(|kept| kept.function.ends_with("list"))
            .expect("the handler must reach the store");
        assert_eq!(listed.store, "AppState.tasks");
        assert_eq!(listed.verb, Verb::Reads);
    }

    #[test]
    fn a_handler_writing_through_the_opened_lock_adds_and_drops_records() {
        let sources = workspace_of(
            "use std::{collections::BTreeMap, sync::{Arc, Mutex}};\n\npub struct Task {\n    pub \
             title: String\n}\n\npub struct State<T>(pub T);\n\npub struct AppState {\n    pub \
             tasks: Mutex<BTreeMap<u64, Task>>\n}\n\npub fn create(State(state): \
             State<Arc<AppState>>) {\n    state.tasks.lock().unwrap().insert(1, Task { title: \
             String::new() });\n}\n\npub fn remove(State(state): State<Arc<AppState>>) {\n    let \
             mut tasks = state.tasks.lock().unwrap();\n    tasks.remove(&1);\n}\n"
        );
        let workspace = Workspace::new(&sources);
        let read = keeping(&workspace);
        let verb = |name: &str| {
            read.iter()
                .find(|kept| kept.function.ends_with(name))
                .map(|kept| kept.verb)
        };
        assert_eq!(verb("create"), Some(Verb::Adds));
        assert_eq!(verb("remove"), Some(Verb::Drops));
    }

    #[test]
    fn a_route_whose_handler_reaches_the_records_is_a_way_to_ask() {
        let sources = workspace_of(
            "use std::{collections::BTreeMap, sync::{Arc, Mutex}};\n\npub struct Task {\n    pub \
             title: String\n}\n\npub struct State<T>(pub T);\n\npub struct AppState {\n    pub \
             tasks: Mutex<BTreeMap<u64, Task>>\n}\n\npub fn list(State(state): \
             State<Arc<AppState>>) -> usize {\n    let tasks = state.tasks.lock().unwrap();\n    \
             tasks.len()\n}\n\npub fn app() -> Router {\n    Router::new().route(\"/tasks\", \
             get(list))\n}\n"
        );
        let workspace = Workspace::new(&sources);
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
             Task>>\n}\n\npub fn read(State(state): State<Arc<AppState>>, Path(id): Path<u64>) \
             -> bool {\n    let tasks = state.tasks.lock().unwrap();\n    \
             tasks.get(&id).is_some()\n}\n\npub fn app() -> Router {\n    \
             Router::new().route(\"/tasks/{id}\", get(read))\n}\n"
        );
        let workspace = Workspace::new(&sources);
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
             Task>>\n}\n\npub fn read(State(state): State<Arc<AppState>>, Path(id): Path<u64>) \
             -> bool {\n    let tasks = state.tasks.lock().unwrap();\n    \
             tasks.get(&1).is_some()\n}\n\npub fn app() -> Router {\n    \
             Router::new().route(\"/tasks/{id}\", get(read))\n}\n"
        );
        let workspace = Workspace::new(&sources);
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
        let workspace = Workspace::new(&sources);
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
        let workspace = Workspace::new(&sources);
        let read = keeping(&workspace);
        assert!(
            !read
                .iter()
                .any(|kept| kept.function.ends_with("fixture_reads_the_store")),
            "{read:?}"
        );
    }

    #[test]
    fn a_serializable_owner_shapes_records_for_the_wire_instead_of_keeping_them() {
        let sources = workspace_of(
            "pub struct Item {\n    pub name: String\n}\n\n#[derive(Serialize)]\npub struct \
             Envelope {\n    pub items: Vec<Item>\n}\n"
        );
        let workspace = Workspace::new(&sources);
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1);
        assert!(
            said[0].contains(
                "shapes `Item` records in `items` of `Envelope` for the wire, as its derive \
                 declares"
            ),
            "{said:?}"
        );
        assert!(!said[0].contains("keeps"), "{said:?}");
    }

    #[test]
    fn a_route_arriving_as_a_wire_shape_enters_its_sentence() {
        let sources = workspace_of(
            "pub struct Item {\n    pub name: String\n}\n\n#[derive(Deserialize)]\npub struct \
             CreateRequest {\n    pub items: Vec<Item>\n}\n\npub async fn create(Json(body): \
             Json<CreateRequest>) -> String {\n    String::new()\n}\n\npub fn app() -> Router \
             {\n    Router::new().route(\"/items\", post(create))\n}\n"
        );
        let workspace = Workspace::new(&sources);
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1);
        assert!(
            said[0].contains("a request arrives as it: `POST /items`"),
            "{said:?}"
        );
    }

    #[test]
    fn a_route_answering_with_a_wire_shape_enters_its_sentence() {
        let sources = workspace_of(
            "pub struct Item {\n    pub name: String\n}\n\n#[derive(Serialize)]\npub struct \
             Envelope {\n    pub items: Vec<Item>\n}\n\npub async fn list() -> Json<Envelope> \
             {\n    Json(Envelope { items: Vec::new() })\n}\n\npub fn app() -> Router {\n    \
             Router::new().route(\"/items\", get(list))\n}\n"
        );
        let workspace = Workspace::new(&sources);
        let said = purpose(&workspace);
        assert_eq!(said.len(), 1);
        assert!(
            said[0].contains("a request is answered with it: `GET /items`"),
            "{said:?}"
        );
    }

    #[test]
    fn a_store_written_to_a_file_outlives_the_run_that_made_it() {
        let sources = workspace_of(&format!(
            "{TASK_STORE}\nimpl Store {{\n    pub fn save(&self, path: &str) -> \
             std::io::Result<()> {{\n        let mut out = String::new();\n        for (_, task) \
             in &self.tasks {{\n            out.push_str(&task.title);\n        }}\n        \
             std::fs::write(path, out)\n    }}\n}}\n"
        ));
        let workspace = Workspace::new(&sources);
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
        let workspace = Workspace::new(&sources);
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
        let workspace = Workspace::new(&sources);
        let carried = outliving(&workspace);
        let persisted = carried
            .iter()
            .find(|entry| entry.function.ends_with("persist"))
            .expect("the query function must carry the store like the macro does");
        assert_eq!(persisted.direction, Direction::Persists);
    }

    #[test]
    fn each_verb_is_read_from_what_the_method_does_to_the_store() {
        let sources = workspace_of(TASK_STORE);
        let workspace = Workspace::new(&sources);
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
