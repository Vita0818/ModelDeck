//! Read-only compatibility import for model connections stored by older builds.
use crate::{database::Database, error::AppError, provider::UniversalProvider};
use std::collections::HashMap;
impl Database {
    pub(crate) fn get_all_universal_providers(
        &self,
    ) -> Result<HashMap<String, UniversalProvider>, AppError> {
        match self.get_setting("universal_providers")? {
            Some(value) => serde_json::from_str(&value)
                .map_err(|e| AppError::Database(format!("Invalid legacy provider data: {e}"))),
            None => Ok(HashMap::new()),
        }
    }
}
