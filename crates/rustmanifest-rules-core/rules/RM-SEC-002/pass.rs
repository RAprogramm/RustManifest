// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

async fn lookup(pool: &sqlx::PgPool, id: i64) -> sqlx::Result<User> {
    sqlx::query_as!(User, "SELECT id, name FROM users WHERE id = $1", id)
        .fetch_one(pool)
        .await
}

struct User {
    id:   i64,
    name: String
}
