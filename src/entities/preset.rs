//! `SeaORM` Entity - Preset
//!
//! 预设实体定义

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "presets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub title: String,
    #[sea_orm(column_type = "Text")]
    pub data: String,
    #[sea_orm(column_type = "Text")]
    pub regex_data: String,
    #[sea_orm(column_type = "Text")]
    pub user_note: String,
    #[sea_orm(column_type = "Text")]
    pub pipi_study: String,
    #[sea_orm(column_type = "Text")]
    pub version: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
