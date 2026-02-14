//! 数据库迁移库入口
//!
//! v1.0.0 - 合并版迁移脚本

pub use sea_orm_migration::prelude::*;

mod m000001_v1_init;
mod m000002_add_avatar_version;
mod m000003_create_presets;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m000001_v1_init::Migration),
            Box::new(m000002_add_avatar_version::Migration),
            Box::new(m000003_create_presets::Migration),
        ]
    }
}
