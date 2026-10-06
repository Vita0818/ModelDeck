//! Claude Code 初次安装确认开关。
//!
//! 读写 `~/.claude.json` 根对象的 `hasCompletedOnboarding`。该文件同时承载
//! Claude Code 自己的其他设置，因此这里只做该字段的增量写入 / 删除。

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{atomic_write, get_claude_mcp_path};
use crate::error::AppError;

fn user_config_path() -> PathBuf {
    get_claude_mcp_path()
}

fn read_json_value(path: &Path) -> Result<Value, AppError> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content = fs::read_to_string(path).map_err(|e| AppError::io(path, e))?;
    let value: Value = serde_json::from_str(&content).map_err(|e| AppError::json(path, e))?;
    Ok(value)
}

fn write_json_value(path: &Path, value: &Value) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
    }
    let json =
        serde_json::to_string_pretty(value).map_err(|e| AppError::JsonSerialize { source: e })?;
    atomic_write(path, json.as_bytes())
}

/// 在 ~/.claude.json 根对象写入 hasCompletedOnboarding=true（用于跳过 Claude Code 初次安装确认）
/// 仅增量写入该字段，其他字段保持不变
pub fn set_has_completed_onboarding() -> Result<bool, AppError> {
    let path = user_config_path();
    let mut root = if path.exists() {
        read_json_value(&path)?
    } else {
        serde_json::json!({})
    };

    let obj = root
        .as_object_mut()
        .ok_or_else(|| AppError::Config("~/.claude.json 根必须是对象".into()))?;

    let already = obj
        .get("hasCompletedOnboarding")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if already {
        return Ok(false);
    }

    obj.insert("hasCompletedOnboarding".into(), Value::Bool(true));
    write_json_value(&path, &root)?;
    Ok(true)
}

/// 删除 ~/.claude.json 根对象的 hasCompletedOnboarding 字段（恢复 Claude Code 初次安装确认）
/// 仅增量删除该字段，其他字段保持不变
pub fn clear_has_completed_onboarding() -> Result<bool, AppError> {
    let path = user_config_path();
    if !path.exists() {
        return Ok(false);
    }

    let mut root = read_json_value(&path)?;
    let obj = root
        .as_object_mut()
        .ok_or_else(|| AppError::Config("~/.claude.json 根必须是对象".into()))?;

    let existed = obj.remove("hasCompletedOnboarding").is_some();
    if !existed {
        return Ok(false);
    }

    write_json_value(&path, &root)?;
    Ok(true)
}
