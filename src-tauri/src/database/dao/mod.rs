//! Data Access Object layer
//!
//! Database access operations for each domain

pub mod providers;
pub mod providers_seed;
pub mod proxy;
pub mod settings;
mod universal_providers;
pub mod usage_rollup;

// 所有 DAO 方法都通过 Database impl 提供，无需单独导出
// 导出 FailoverQueueItem / Profile 供外部使用
