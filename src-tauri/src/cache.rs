//! Remembers commit nodes in the app's database, so that a repository's
//! graph does not have to be read out of git's packs every time it opens.
//!
//! The table is defined with the rest of the schema in `src/lib/db/schema.ts`.
//! What it holds can never be stale: a hash names one commit for good.

use std::collections::HashMap;

use git2::Oid;
use sqlx::{Pool, QueryBuilder, Sqlite};
use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};

use crate::{DATABASE, history::Node};

/// The database connection the frontend opened, if it has by now.
pub async fn pool(app: &AppHandle) -> Option<Pool<Sqlite>> {
    let instances = app.try_state::<DbInstances>()?;
    let instances = instances.0.read().await;
    #[allow(
        unreachable_patterns,
        reason = "the enum grows with the plugin's other drivers"
    )]
    match instances.get(DATABASE)? {
        DbPool::Sqlite(pool) => Some(pool.clone()),
        _ => None,
    }
}

/// Everything remembered about the commits of the repository `repo`.
pub async fn load(pool: &Pool<Sqlite>, repo: &str) -> Result<HashMap<Oid, Node>, sqlx::Error> {
    let rows: Vec<(Vec<u8>, i64, Vec<u8>)> =
        sqlx::query_as("SELECT hash, time, parents FROM commit_node WHERE repo_id = ?")
            .bind(repo)
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(hash, time, parents)| {
            let parents = parents
                .chunks(20)
                .map(Oid::from_bytes)
                .collect::<Result<_, _>>()
                .ok()?;
            Some((Oid::from_bytes(&hash).ok()?, Node { time, parents }))
        })
        .collect())
}

/// Remembers `nodes` for the repository `repo`.
pub async fn store(
    pool: &Pool<Sqlite>,
    repo: &str,
    nodes: &[(Oid, Node)],
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    // SQLite takes a limited number of values per statement
    for chunk in nodes.chunks(5_000) {
        QueryBuilder::new("INSERT OR IGNORE INTO commit_node (repo_id, hash, time, parents) ")
            .push_values(chunk, |mut row, (id, node)| {
                row.push_bind(repo)
                    .push_bind(id.as_bytes().to_vec())
                    .push_bind(node.time)
                    .push_bind(
                        node.parents
                            .iter()
                            .flat_map(Oid::as_bytes)
                            .copied()
                            .collect::<Vec<u8>>(),
                    );
            })
            .build()
            .execute(&mut *transaction)
            .await?;
    }
    transaction.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nodes_come_back_as_they_were_stored() {
        tauri::async_runtime::block_on(async {
            let pool = Pool::<Sqlite>::connect("sqlite::memory:").await.unwrap();
            for migration in crate::migrations() {
                sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
            }
            sqlx::raw_sql(
                "INSERT INTO workspace (id, name) VALUES ('w', 'Work');
                 INSERT INTO repo (id, workspace_id, name, path) VALUES ('r', 'w', 'r', '/r');",
            )
            .execute(&pool)
            .await
            .unwrap();
            let id = |byte: u8| Oid::from_bytes(&[byte; 20]).unwrap();
            let nodes = vec![
                (
                    id(3),
                    Node {
                        time: 30,
                        parents: vec![id(2), id(1)],
                    },
                ),
                (
                    id(1),
                    Node {
                        time: -10,
                        parents: vec![],
                    },
                ),
            ];

            store(&pool, "r", &nodes).await.unwrap();
            // storing again changes nothing
            store(&pool, "r", &nodes).await.unwrap();

            assert_eq!(load(&pool, "r").await.unwrap(), nodes.into_iter().collect());
            assert!(load(&pool, "other").await.unwrap().is_empty());
        });
    }
}
