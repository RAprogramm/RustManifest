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

mod outliving;
mod store;
mod verb;

pub use self::{
    outliving::{Direction, Outliving, outliving},
    store::{Store, stores},
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

    use super::{Direction, Verb, keeping, outliving, stores};
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
