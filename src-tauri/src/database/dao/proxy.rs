//! 代理功能数据访问层
//!
//! 处理代理配置、Provider健康状态和使用统计的数据库操作

use std::str::FromStr;

use crate::error::AppError;
use crate::proxy::types::*;
use rust_decimal::Decimal;

use super::super::{lock_conn, Database};

pub(crate) const PRICING_SOURCE_RESPONSE: &str = "response";
pub(crate) const PRICING_SOURCE_REQUEST: &str = "request";

pub(crate) fn validate_cost_multiplier(value: &str) -> Result<Decimal, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::localized(
            "error.multiplierEmpty",
            "倍率不能为空",
            "Multiplier cannot be empty",
        ));
    }
    let parsed = Decimal::from_str(trimmed).map_err(|e| {
        AppError::localized(
            "error.invalidMultiplier",
            format!("无效倍率: {value} - {e}"),
            format!("Invalid multiplier: {value} - {e}"),
        )
    })?;
    if parsed < Decimal::ZERO {
        return Err(AppError::localized(
            "error.invalidMultiplier",
            format!("无效倍率: {value} - 倍率不能为负数"),
            format!("Invalid multiplier: {value} - multiplier cannot be negative"),
        ));
    }
    Ok(parsed)
}

pub(crate) fn validate_pricing_source(value: &str) -> Result<&str, AppError> {
    let trimmed = value.trim();
    if trimmed == PRICING_SOURCE_RESPONSE || trimmed == PRICING_SOURCE_REQUEST {
        Ok(trimmed)
    } else {
        Err(AppError::localized(
            "error.invalidPricingMode",
            format!("无效计费模式: {value}"),
            format!("Invalid pricing mode: {value}"),
        ))
    }
}

impl Database {
    // ==================== Global Proxy Config ====================

    /// 获取全局代理配置（统一字段）
    ///
    /// 从 claude 行读取（三行镜像一致）
    pub async fn get_global_proxy_config(&self) -> Result<GlobalProxyConfig, AppError> {
        // 使用 block 限制 conn 的作用域，避免跨 await 持有锁
        let result = {
            let conn = lock_conn!(self.conn);
            conn.query_row(
                "SELECT proxy_enabled, listen_address, listen_port, enable_logging
                 FROM proxy_config WHERE app_type = 'claude'",
                [],
                |row| {
                    Ok(GlobalProxyConfig {
                        proxy_enabled: row.get::<_, i32>(0)? != 0,
                        listen_address: row.get(1)?,
                        listen_port: row.get::<_, i32>(2)? as u16,
                        enable_logging: row.get::<_, i32>(3)? != 0,
                    })
                },
            )
        };
        // conn 已在 block 结束时释放

        match result {
            Ok(config) => Ok(config),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // 如果不存在，创建默认配置
                self.init_proxy_config_rows().await?;
                Ok(GlobalProxyConfig {
                    proxy_enabled: false,
                    listen_address: "127.0.0.1".to_string(),
                    listen_port: 15721,
                    enable_logging: true,
                })
            }
            Err(e) => Err(AppError::Database(e.to_string())),
        }
    }

    /// 更新全局代理配置（镜像写三行）
    pub async fn update_global_proxy_config(
        &self,
        config: GlobalProxyConfig,
    ) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);

        conn.execute(
            "UPDATE proxy_config SET
                proxy_enabled = ?1,
                listen_address = ?2,
                listen_port = ?3,
                enable_logging = ?4,
                updated_at = datetime('now')",
            rusqlite::params![
                if config.proxy_enabled { 1 } else { 0 },
                config.listen_address,
                config.listen_port as i32,
                if config.enable_logging { 1 } else { 0 },
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(())
    }

    /// 获取默认成本倍率
    pub async fn get_default_cost_multiplier(&self, app_type: &str) -> Result<String, AppError> {
        let result = {
            let conn = lock_conn!(self.conn);
            conn.query_row(
                "SELECT default_cost_multiplier FROM proxy_config WHERE app_type = ?1",
                [app_type],
                |row| row.get(0),
            )
        };

        match result {
            Ok(value) => Ok(value),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                self.init_proxy_config_rows().await?;
                Ok("1".to_string())
            }
            Err(e) => Err(AppError::Database(e.to_string())),
        }
    }

    /// 设置默认成本倍率
    pub async fn set_default_cost_multiplier(
        &self,
        app_type: &str,
        value: &str,
    ) -> Result<(), AppError> {
        validate_cost_multiplier(value)?;
        let trimmed = value.trim();

        // 确保行存在
        self.ensure_proxy_config_row_exists(app_type)?;

        let conn = lock_conn!(self.conn);
        conn.execute(
            "UPDATE proxy_config SET
                default_cost_multiplier = ?2,
                updated_at = datetime('now')
             WHERE app_type = ?1",
            rusqlite::params![app_type, trimmed],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(())
    }

    /// 获取计费模式来源
    pub async fn get_pricing_model_source(&self, app_type: &str) -> Result<String, AppError> {
        let result = {
            let conn = lock_conn!(self.conn);
            conn.query_row(
                "SELECT pricing_model_source FROM proxy_config WHERE app_type = ?1",
                [app_type],
                |row| row.get(0),
            )
        };

        match result {
            Ok(value) => Ok(value),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                self.init_proxy_config_rows().await?;
                Ok(PRICING_SOURCE_RESPONSE.to_string())
            }
            Err(e) => Err(AppError::Database(e.to_string())),
        }
    }

    /// 设置计费模式来源
    pub async fn set_pricing_model_source(
        &self,
        app_type: &str,
        value: &str,
    ) -> Result<(), AppError> {
        let trimmed = validate_pricing_source(value)?;

        // 确保行存在
        self.ensure_proxy_config_row_exists(app_type)?;

        let conn = lock_conn!(self.conn);
        conn.execute(
            "UPDATE proxy_config SET
                pricing_model_source = ?2,
                updated_at = datetime('now')
             WHERE app_type = ?1",
            rusqlite::params![app_type, trimmed],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(())
    }
}

impl Database {
    /// 初始化 proxy_config 表的三行数据
    ///
    /// 使用与 schema.rs seed 相同的 per-app 默认值
    async fn init_proxy_config_rows(&self) -> Result<(), AppError> {
        let conn = lock_conn!(self.conn);

        // 使用与 schema.rs seed 相同的 per-app 默认值
        // claude: 更激进的重试和超时配置
        conn.execute(
            "INSERT OR IGNORE INTO proxy_config (
                app_type, max_retries,
                streaming_first_byte_timeout, streaming_idle_timeout, non_streaming_timeout,
                circuit_failure_threshold, circuit_success_threshold, circuit_timeout_seconds,
                circuit_error_rate_threshold, circuit_min_requests
            ) VALUES ('claude', 6, 90, 180, 600, 8, 3, 90, 0.7, 15)",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        // codex: 默认配置
        conn.execute(
            "INSERT OR IGNORE INTO proxy_config (
                app_type, max_retries,
                streaming_first_byte_timeout, streaming_idle_timeout, non_streaming_timeout,
                circuit_failure_threshold, circuit_success_threshold, circuit_timeout_seconds,
                circuit_error_rate_threshold, circuit_min_requests
            ) VALUES ('codex', 3, 60, 120, 600, 4, 2, 60, 0.6, 10)",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        // gemini: 稍高的重试次数
        conn.execute(
            "INSERT OR IGNORE INTO proxy_config (
                app_type, max_retries,
                streaming_first_byte_timeout, streaming_idle_timeout, non_streaming_timeout,
                circuit_failure_threshold, circuit_success_threshold, circuit_timeout_seconds,
                circuit_error_rate_threshold, circuit_min_requests
            ) VALUES ('gemini', 5, 60, 120, 600, 4, 2, 60, 0.6, 10)",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        // grokbuild: Responses protocol, same timeout defaults as Codex.
        conn.execute(
            "INSERT OR IGNORE INTO proxy_config (
                app_type, max_retries,
                streaming_first_byte_timeout, streaming_idle_timeout, non_streaming_timeout,
                circuit_failure_threshold, circuit_success_threshold, circuit_timeout_seconds,
                circuit_error_rate_threshold, circuit_min_requests
            ) VALUES ('grokbuild', 3, 60, 120, 600, 4, 2, 60, 0.6, 10)",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(())
    }

    /// 确保指定 app_type 的 proxy_config 行存在（同步版本，用于 set_* 函数）
    ///
    /// 使用与 schema.rs seed 相同的 per-app 默认值
    fn ensure_proxy_config_row_exists(&self, app_type: &str) -> Result<(), AppError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Lock(e.to_string()))?;

        // 根据 app_type 使用不同的默认值（与 schema.rs seed 保持一致）
        let (retries, fb_timeout, idle_timeout, cb_fail, cb_succ, cb_timeout, cb_rate, cb_min) =
            match app_type {
                "claude" => (6, 90, 180, 8, 3, 90, 0.7, 15),
                "codex" => (3, 60, 120, 4, 2, 60, 0.6, 10),
                "gemini" => (5, 60, 120, 4, 2, 60, 0.6, 10),
                "grokbuild" => (3, 60, 120, 4, 2, 60, 0.6, 10),
                _ => (3, 60, 120, 4, 2, 60, 0.6, 10), // 默认值
            };

        conn.execute(
            "INSERT OR IGNORE INTO proxy_config (
                app_type, max_retries,
                streaming_first_byte_timeout, streaming_idle_timeout, non_streaming_timeout,
                circuit_failure_threshold, circuit_success_threshold, circuit_timeout_seconds,
                circuit_error_rate_threshold, circuit_min_requests
            ) VALUES (?1, ?2, ?3, ?4, 600, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                app_type,
                retries,
                fb_timeout,
                idle_timeout,
                cb_fail,
                cb_succ,
                cb_timeout,
                cb_rate,
                cb_min
            ],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::database::Database;
    use crate::error::AppError;

    #[tokio::test]
    async fn test_default_cost_multiplier_round_trip() -> Result<(), AppError> {
        let db = Database::memory()?;

        let default = db.get_default_cost_multiplier("claude").await?;
        assert_eq!(default, "1");

        db.set_default_cost_multiplier("claude", "1.5").await?;
        let updated = db.get_default_cost_multiplier("claude").await?;
        assert_eq!(updated, "1.5");

        Ok(())
    }

    #[tokio::test]
    async fn test_default_cost_multiplier_validation() -> Result<(), AppError> {
        let db = Database::memory()?;

        let err = db
            .set_default_cost_multiplier("claude", "not-a-number")
            .await
            .unwrap_err();
        // AppError::localized returns AppError::Localized variant
        assert!(matches!(
            err,
            AppError::Localized {
                key: "error.invalidMultiplier",
                ..
            }
        ));

        Ok(())
    }

    #[tokio::test]
    async fn test_pricing_model_source_round_trip_and_validation() -> Result<(), AppError> {
        let db = Database::memory()?;

        let default = db.get_pricing_model_source("claude").await?;
        assert_eq!(default, "response");

        db.set_pricing_model_source("claude", "request").await?;
        let updated = db.get_pricing_model_source("claude").await?;
        assert_eq!(updated, "request");

        let err = db
            .set_pricing_model_source("claude", "invalid")
            .await
            .unwrap_err();
        // AppError::localized returns AppError::Localized variant
        assert!(matches!(
            err,
            AppError::Localized {
                key: "error.invalidPricingMode",
                ..
            }
        ));

        let err = db
            .set_default_cost_multiplier("claude", "-0.5")
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            AppError::Localized {
                key: "error.invalidMultiplier",
                ..
            }
        ));

        Ok(())
    }
}
