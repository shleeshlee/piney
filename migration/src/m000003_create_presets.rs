//! 迁移：创建 presets 表
//!
//! 存储 SillyTavern 聊天补全预设及其配套正则

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // 检查表是否已存在（确保幂等性，避免重复创建导致崩溃）
        let result = conn
            .query_all(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT COUNT(*) as cnt FROM sqlite_master WHERE type='table' AND name='presets'"
                    .to_string(),
            ))
            .await?;

        if let Some(row) = result.first() {
            let count: i32 = row.try_get("", "cnt").unwrap_or(0);
            if count > 0 {
                // 表已存在，跳过创建（幂等迁移）
                return Ok(());
            }
        }

        conn.execute_unprepared(
            "CREATE TABLE presets (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                data TEXT NOT NULL,
                regex_data TEXT NOT NULL DEFAULT '[]',
                user_note TEXT NOT NULL DEFAULT '',
                pipi_study TEXT NOT NULL DEFAULT '',
                version TEXT NOT NULL DEFAULT '1.0.0',
                created_at DATETIME NOT NULL,
                updated_at DATETIME NOT NULL
            );",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS presets;")
            .await?;

        Ok(())
    }
}
