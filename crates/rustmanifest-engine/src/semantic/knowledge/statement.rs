// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What one written SQL statement does and which table it names.
//!
//! The statement is the program's own text: the kind is its first
//! keyword, and the table stands right after `INTO`, `UPDATE` or
//! `FROM`, spelled by the author. Both readings stop where proof
//! stops — a statement joining tables, selecting from a subquery, or
//! built at run time names no single table, and silence is the answer
//! there, never the first name that looks plausible.

use crate::semantic::behavior::Statement;

/// The kind a written statement proves, by its first keyword.
pub(in crate::semantic) fn statement_of(sql: &str) -> Option<Statement> {
    match first_keyword(sql).as_str() {
        "SELECT" => Some(Statement::Selects),
        "INSERT" => Some(Statement::Inserts),
        "UPDATE" => Some(Statement::Updates),
        "DELETE" => Some(Statement::Deletes),
        _ => None
    }
}

/// The one table a written statement provably names, as the author
/// spelled it.
///
/// `INSERT INTO t`, `UPDATE t` and `DELETE FROM t` name their table in
/// the grammar itself. `SELECT` names one only when its `FROM` is
/// followed by a single plain name: a join, a comma-joined list or a
/// subquery reads more than one source, and which of them holds the
/// records is not written in the keyword — so nothing is claimed.
pub(in crate::semantic) fn table_of(sql: &str) -> Option<String> {
    let words: Vec<&str> = sql.split_whitespace().collect();
    let after = |marker: &str| {
        words
            .iter()
            .position(|word| word.eq_ignore_ascii_case(marker))
            .and_then(|at| words.get(at + 1))
            .and_then(|word| plain_name(word))
    };
    match statement_of(sql)? {
        Statement::Inserts => after("INTO"),
        Statement::Updates => words.get(1).and_then(|word| plain_name(word)),
        Statement::Deletes => after("FROM"),
        Statement::Selects => single_source(&words)
    }
}

/// The one table a `SELECT` reads, where its `FROM` provably lists one
/// source and nothing else.
fn single_source(words: &[&str]) -> Option<String> {
    if words.iter().any(|word| word.eq_ignore_ascii_case("JOIN")) {
        return None;
    }
    let mut froms = words
        .iter()
        .enumerate()
        .filter(|(_, word)| word.eq_ignore_ascii_case("FROM"))
        .map(|(at, _)| at);
    let (at, second) = (froms.next()?, froms.next());
    if second.is_some() {
        return None;
    }
    let named = words.get(at + 1)?;
    let listed =
        named.ends_with(',') || words.get(at + 2).is_some_and(|next| next.starts_with(','));
    if listed {
        return None;
    }
    plain_name(named)
}

/// The first keyword of a statement, upper-cased.
fn first_keyword(sql: &str) -> String {
    sql.trim_start()
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect::<String>()
        .to_ascii_uppercase()
}

/// A token as a plain table name, quoting stripped, cut at the first
/// character a name cannot contain.
///
/// A token that opens a parenthesis is a subquery or a column list
/// standing where the name was expected, and names nothing.
fn plain_name(word: &str) -> Option<String> {
    if word.starts_with('(') {
        return None;
    }
    let cleaned: String = word
        .trim_matches(|edge| edge == '"' || edge == '`')
        .chars()
        .take_while(|part| part.is_ascii_alphanumeric() || *part == '_' || *part == '.')
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{Statement, statement_of, table_of};

    #[test]
    fn each_data_keyword_names_its_kind_and_ddl_names_none() {
        assert_eq!(statement_of("SELECT 1"), Some(Statement::Selects));
        assert_eq!(statement_of("insert into t"), Some(Statement::Inserts));
        assert_eq!(statement_of("CREATE TABLE t (id INT)"), None);
    }

    #[test]
    fn the_grammar_slot_after_the_keyword_names_the_table() {
        assert_eq!(
            table_of("INSERT INTO tasks (title) VALUES ($1)"),
            Some("tasks".to_owned())
        );
        assert_eq!(
            table_of("UPDATE tasks SET done = TRUE WHERE id = $1"),
            Some("tasks".to_owned())
        );
        assert_eq!(
            table_of("DELETE FROM tasks WHERE id = $1"),
            Some("tasks".to_owned())
        );
        assert_eq!(
            table_of("SELECT id, title FROM tasks ORDER BY id"),
            Some("tasks".to_owned())
        );
    }

    /// More than one source in a `SELECT` proves no single table, and
    /// the reading says nothing instead of picking one.
    #[test]
    fn a_statement_reading_several_sources_names_no_table() {
        assert_eq!(
            table_of("SELECT a.id FROM tasks a JOIN users b ON TRUE"),
            None
        );
        assert_eq!(table_of("SELECT id FROM tasks, users"), None);
        assert_eq!(
            table_of("SELECT id FROM (SELECT id FROM tasks) inner_rows"),
            None
        );
    }

    #[test]
    fn quoting_is_stripped_and_a_column_list_is_not_a_name() {
        assert_eq!(
            table_of("DELETE FROM \"tasks\" WHERE id = $1"),
            Some("tasks".to_owned())
        );
        assert_eq!(table_of("INSERT INTO (title) VALUES ($1)"), None);
    }
}
