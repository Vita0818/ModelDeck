//! Copilot app model records. Never snapshot or replace its account/session DB.
use crate::{
    config::{atomic_write_private, get_app_config_dir, get_home_dir},
    model_clients::ClientProfile,
};
use base64::Engine;
use rusqlite::{
    types::{Value as SqlValue, ValueRef},
    Connection, OpenFlags, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf, time::Duration};

const PROVIDER_ID: &str = "058f3eb5-ea87-5fbf-98c0-420ffbc3f7b6";
const SELECTION_KEY: &str = "copilot-selected-model";
type Row = BTreeMap<String, Value>;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct ModelState {
    provider: Option<Row>,
    models: Vec<Row>,
    selection: Option<Row>,
}

#[derive(Serialize, Deserialize)]
struct Backup {
    version: u32,
    database: PathBuf,
    original: ModelState,
    applied: ModelState,
}

fn backup_path() -> PathBuf {
    get_app_config_dir().join("deck-backups/copilot-desktop.json")
}

fn database_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("COPILOT_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| get_home_dir().join(".copilot"));
    if !home.is_absolute() {
        return Err("Use an absolute COPILOT_HOME directory".into());
    }
    Ok(home.join("data.db"))
}

fn database_error(error: rusqlite::Error) -> String {
    format!("Cannot update Copilot app model configuration: {error}")
}

fn connect(path: &std::path::Path) -> Result<Connection, String> {
    if !path.is_file() {
        return Err(
            "Run GitHub Copilot once to initialize its desktop configuration, then apply again"
                .into(),
        );
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(database_error)?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(database_error)?;
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(database_error)?;
    for sql in [
        "SELECT id,name,type,base_url,wire_api,azure_api_version,auth_kind,headers_json,settings_json,created_at,updated_at FROM model_providers LIMIT 0",
        "SELECT id,provider_id,model_id,wire_model,display_name,max_prompt_tokens,max_output_tokens,wire_api_override,created_at,updated_at FROM provider_models LIMIT 0",
        "SELECT key,value,updated_at FROM app_state LIMIT 0",
    ] {
        conn.prepare(sql).map_err(|_| "This Copilot app model database schema is not supported; no configuration was changed".to_string())?;
    }
    Ok(conn)
}

fn read_rows(conn: &Connection, sql: &str, parameter: &str) -> Result<Vec<Row>, String> {
    let mut stmt = conn.prepare(sql).map_err(database_error)?;
    let rows = stmt
        .query_map([parameter], |row| {
            let mut values = Row::new();
            for index in 0..row.as_ref().column_count() {
                let value = match row.get_ref(index)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(value) => json!(value),
                    ValueRef::Real(value) => json!(value),
                    ValueRef::Text(_) => Value::String(row.get(index)?),
                    ValueRef::Blob(value) => {
                        json!({"$blob":base64::engine::general_purpose::STANDARD.encode(value)})
                    }
                };
                values.insert(row.as_ref().column_name(index)?.into(), value);
            }
            Ok(values)
        })
        .map_err(database_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(database_error)
}

fn read_state(conn: &Connection) -> Result<ModelState, String> {
    Ok(ModelState {
        provider: read_rows(
            conn,
            "SELECT * FROM model_providers WHERE id=?1",
            PROVIDER_ID,
        )?
        .pop(),
        models: read_rows(
            conn,
            "SELECT * FROM provider_models WHERE provider_id=?1 ORDER BY id",
            PROVIDER_ID,
        )?,
        selection: read_rows(conn, "SELECT * FROM app_state WHERE key=?1", SELECTION_KEY)?.pop(),
    })
}

fn sql_value(value: &Value) -> Result<SqlValue, String> {
    Ok(match value {
        Value::Null => SqlValue::Null,
        Value::String(value) => SqlValue::Text(value.clone()),
        Value::Number(value) if value.is_i64() => SqlValue::Integer(value.as_i64().unwrap()),
        Value::Number(value) if value.is_f64() => SqlValue::Real(value.as_f64().unwrap()),
        Value::Object(value) if value.len() == 1 && value.contains_key("$blob") => SqlValue::Blob(
            base64::engine::general_purpose::STANDARD
                .decode(
                    value["$blob"]
                        .as_str()
                        .ok_or("Invalid native model backup")?,
                )
                .map_err(|_| "Invalid native model backup")?,
        ),
        _ => return Err("Unsupported value in Copilot app model configuration".into()),
    })
}

fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn upsert(conn: &Connection, table: &str, key: &str, row: &Row) -> Result<(), String> {
    let columns: Vec<_> = row.keys().map(|name| quote_identifier(name)).collect();
    let values = row.values().map(sql_value).collect::<Result<Vec<_>, _>>()?;
    let updates = row
        .keys()
        .filter(|name| name.as_str() != key)
        .map(|name| {
            let name = quote_identifier(name);
            format!("{name}=excluded.{name}")
        })
        .collect::<Vec<_>>()
        .join(",");
    let placeholders = (1..=columns.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",");
    conn.execute(&format!("INSERT INTO {table} ({}) VALUES ({placeholders}) ON CONFLICT ({}) DO UPDATE SET {updates}", columns.join(","), quote_identifier(key)), rusqlite::params_from_iter(values.iter()))
        .map_err(database_error)?;
    Ok(())
}

fn replace_state(conn: &Connection, state: &ModelState) -> Result<(), String> {
    if state
        .provider
        .as_ref()
        .is_some_and(|row| row.get("id").and_then(Value::as_str) != Some(PROVIDER_ID))
        || state
            .models
            .iter()
            .any(|row| row.get("provider_id").and_then(Value::as_str) != Some(PROVIDER_ID))
        || state
            .selection
            .as_ref()
            .is_some_and(|row| row.get("key").and_then(Value::as_str) != Some(SELECTION_KEY))
    {
        return Err("Invalid Copilot desktop model backup ownership".into());
    }
    for current in read_rows(
        conn,
        "SELECT * FROM provider_models WHERE provider_id=?1",
        PROVIDER_ID,
    )? {
        if !state
            .models
            .iter()
            .any(|model| model.get("id") == current.get("id"))
        {
            let id = current
                .get("id")
                .and_then(Value::as_str)
                .ok_or("Invalid native model ID")?;
            conn.execute(
                "DELETE FROM provider_models WHERE id=?1 AND provider_id=?2",
                [id, PROVIDER_ID],
            )
            .map_err(database_error)?;
        }
    }
    if let Some(provider) = &state.provider {
        upsert(conn, "model_providers", "id", provider)?;
        for model in &state.models {
            upsert(conn, "provider_models", "id", model)?;
        }
    } else {
        conn.execute("DELETE FROM model_providers WHERE id=?1", [PROVIDER_ID])
            .map_err(database_error)?;
    }
    if let Some(selection) = &state.selection {
        upsert(conn, "app_state", "key", selection)?;
    } else {
        conn.execute("DELETE FROM app_state WHERE key=?1", [SELECTION_KEY])
            .map_err(database_error)?;
    }
    Ok(())
}

fn row(value: Value) -> Row {
    value
        .as_object()
        .expect("native record object")
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn model_row_id(alias: &str) -> String {
    let hash = Sha256::digest(format!("{PROVIDER_ID}\0{alias}").as_bytes());
    let mut bytes: [u8; 16] = hash[..16].try_into().unwrap();
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

fn projected_state(profile: &ClientProfile, before: &ModelState) -> Result<ModelState, String> {
    if !profile
        .models
        .iter()
        .any(|model| model.id == profile.default_model)
    {
        return Err("Default model is unavailable".into());
    }
    let (kind, api) = match profile.protocol.as_str() {
        "openai-chat" => ("custom", "completions"),
        "openai-responses" => ("custom", "responses"),
        "anthropic" => ("anthropic", "completions"),
        _ => return Err("Unsupported Copilot desktop protocol".into()),
    };
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let base = format!("{}/v1", profile.endpoint.trim_end_matches('/'));
    // This grants only access to the local ModelDeck endpoint. No upstream
    // secret or GitHub/keychain credential is read or written by this adapter.
    let headers = json!({"Authorization":format!("Bearer {}", profile.local_token)});
    let mut settings = before
        .provider
        .as_ref()
        .and_then(|provider| provider.get("settings_json"))
        .and_then(Value::as_str)
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    for (key, value) in json!({"baseUrl":base,"wireApi":api,"azureApiVersion":null,"authKind":"none","headers":headers}).as_object().unwrap() {
        settings[key] = value.clone();
    }
    let mut provider = before.provider.clone().unwrap_or_default();
    provider.extend(row(json!({"id":PROVIDER_ID,"name":"ModelDeck","type":kind,"base_url":base,"wire_api":api,"azure_api_version":null,"auth_kind":"none","headers_json":headers.to_string(),"settings_json":settings.to_string(),"updated_at":now})));
    provider.entry("created_at".into()).or_insert(json!(now));
    let models = profile.models.iter().map(|model| {
        let mut record = before.models.iter().find(|row| row.get("model_id").and_then(Value::as_str) == Some(&model.id)).cloned().unwrap_or_default();
        record.extend(row(json!({"id":model_row_id(&model.id),"provider_id":PROVIDER_ID,"model_id":model.id,"wire_model":model.id,"display_name":model.name,"max_prompt_tokens":model.context.saturating_sub(model.output).max(1),"max_output_tokens":model.output,"wire_api_override":null,"updated_at":now})));
        record.entry("created_at".into()).or_insert(json!(now));
        record
    }).collect();
    Ok(ModelState {
        provider: Some(provider),
        models,
        selection: Some(row(
            json!({"key":SELECTION_KEY,"value":format!("{PROVIDER_ID}/{}",profile.default_model),"updated_at":now}),
        )),
    })
}

fn read_backup() -> Result<Option<(Backup, Vec<u8>)>, String> {
    match fs::read(backup_path()) {
        Ok(bytes) => {
            let backup: Backup = serde_json::from_slice(&bytes)
                .map_err(|_| "Invalid Copilot desktop model backup")?;
            if backup.version != 1 {
                return Err("Unsupported Copilot desktop model backup".into());
            }
            Ok(Some((backup, bytes)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn restore_database(conn: &mut Connection, state: &ModelState) -> Result<(), String> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(database_error)?;
    replace_state(&tx, state)?;
    tx.commit().map_err(database_error)
}

pub(super) fn apply(
    profile: &ClientProfile,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let database = database_path()?;
    let mut conn = connect(&database)?;
    let existing = read_backup()?;
    if existing
        .as_ref()
        .is_some_and(|(backup, _)| backup.database != database)
    {
        return Err(
            "Restore the Copilot desktop connection before changing its configuration directory"
                .into(),
        );
    }
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(database_error)?;
    let before = read_state(&tx)?;
    let desired = projected_state(profile, &before)?;
    replace_state(&tx, &desired)?;
    let applied = read_state(&tx)?;
    let backup = Backup {
        version: 1,
        database,
        original: existing
            .as_ref()
            .map(|(backup, _)| backup.original.clone())
            .unwrap_or_else(|| before.clone()),
        applied,
    };
    atomic_write_private(
        &backup_path(),
        &serde_json::to_vec(&backup).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let result = tx.commit().map_err(database_error).and_then(|()| commit());
    if let Err(error) = result {
        restore_database(&mut conn, &before)?;
        if let Some((_, bytes)) = existing {
            atomic_write_private(&backup_path(), &bytes).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(backup_path()).map_err(|e| e.to_string())?;
        }
        return Err(error);
    }
    Ok(())
}

pub(super) fn restore(commit: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    let (backup, _) =
        read_backup()?.ok_or("Original Copilot desktop model backup is unavailable")?;
    let mut conn = connect(&backup.database)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(database_error)?;
    let before = read_state(&tx)?;
    if before.provider != backup.applied.provider || before.models != backup.applied.models {
        return Err("Copilot desktop model settings changed after ModelDeck applied them; restore was stopped to preserve those edits".into());
    }
    let mut original = backup.original;
    // A selection in another provider belongs to the user. Only undo the
    // selection when it still references this connection's native provider.
    let managed_selection = before
        .selection
        .as_ref()
        .and_then(|row| row.get("value"))
        .and_then(Value::as_str)
        .is_some_and(|value| value.starts_with(&format!("{PROVIDER_ID}/")));
    if !managed_selection {
        original.selection = before.selection.clone();
    }
    replace_state(&tx, &original)?;
    tx.commit().map_err(database_error)?;
    if let Err(error) = commit() {
        restore_database(&mut conn, &before)?;
        return Err(error);
    }
    fs::remove_file(backup_path()).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests;
