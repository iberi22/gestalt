//! StateBackend trait — abstract persistent storage.
//! Native: SQLite via rusqlite.
//! WASM: localStorage via web-sys.

/// Abstract persistent storage backend.
pub trait StateBackend: Send + Sync {
    /// Store a key-value pair.
    fn put(&self, key: &str, value: &str) -> Result<(), String>;
    /// Retrieve a value by key.
    fn get(&self, key: &str) -> Result<Option<String>, String>;
    /// Delete a key.
    fn delete(&self, key: &str) -> Result<(), String>;
    /// List all keys with a prefix.
    fn list(&self, prefix: &str) -> Result<Vec<String>, String>;
}

/// SQLite-backed state backend (native only).
#[cfg(not(target_arch = "wasm32"))]
pub mod native {
    use super::StateBackend;
    use std::sync::Mutex;

    pub struct NativeStateBackend {
        conn: Mutex<rusqlite::Connection>,
    }

    impl NativeStateBackend {
        pub fn new(path: &str) -> Result<Self, String> {
            let conn = rusqlite::Connection::open(path)
                .map_err(|e| format!("DB open: {e}"))?;
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);"
            ).map_err(|e| format!("DB init: {e}"))?;
            Ok(Self { conn: Mutex::new(conn) })
        }

        pub fn in_memory() -> Result<Self, String> {
            let conn = rusqlite::Connection::open_in_memory()
                .map_err(|e| format!("DB in-memory: {e}"))?;
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);"
            ).map_err(|e| format!("DB init: {e}"))?;
            Ok(Self { conn: Mutex::new(conn) })
        }
    }

    impl StateBackend for NativeStateBackend {
        fn put(&self, key: &str, value: &str) -> Result<(), String> {
            let conn = self.conn.lock().map_err(|e| format!("Lock: {e}"))?;
            conn.execute(
                "INSERT OR REPLACE INTO kv (key, value) VALUES (?1, ?2)",
                rusqlite::params![key, value],
            ).map_err(|e| format!("Put: {e}"))?;
            Ok(())
        }

        fn get(&self, key: &str) -> Result<Option<String>, String> {
            let conn = self.conn.lock().map_err(|e| format!("Lock: {e}"))?;
            let mut stmt = conn.prepare("SELECT value FROM kv WHERE key = ?1")
                .map_err(|e| format!("Get prepare: {e}"))?;
            let result = stmt.query_row(rusqlite::params![key], |row| row.get(0)).ok();
            Ok(result)
        }

        fn delete(&self, key: &str) -> Result<(), String> {
            let conn = self.conn.lock().map_err(|e| format!("Lock: {e}"))?;
            conn.execute("DELETE FROM kv WHERE key = ?1", rusqlite::params![key])
                .map_err(|e| format!("Delete: {e}"))?;
            Ok(())
        }

        fn list(&self, prefix: &str) -> Result<Vec<String>, String> {
            let conn = self.conn.lock().map_err(|e| format!("Lock: {e}"))?;
            let mut stmt = conn.prepare("SELECT key FROM kv WHERE key LIKE ?1")
                .map_err(|e| format!("List prepare: {e}"))?;
            let pattern = format!("{}%", prefix);
            let keys: Vec<String> = stmt.query_map(rusqlite::params![pattern], |row| row.get(0))
                .map_err(|e| format!("List query: {e}"))?
                .filter_map(|r| r.ok())
                .collect();
            Ok(keys)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_put_get_roundtrip() {
            let backend = NativeStateBackend::in_memory().unwrap();
            backend.put("hello", "world").unwrap();
            assert_eq!(backend.get("hello").unwrap(), Some("world".to_string()));
        }

        #[test]
        fn test_get_missing() {
            let backend = NativeStateBackend::in_memory().unwrap();
            assert_eq!(backend.get("nothing").unwrap(), None);
        }

        #[test]
        fn test_delete() {
            let backend = NativeStateBackend::in_memory().unwrap();
            backend.put("temp", "value").unwrap();
            backend.delete("temp").unwrap();
            assert_eq!(backend.get("temp").unwrap(), None);
        }

        #[test]
        fn test_list_prefix() {
            let backend = NativeStateBackend::in_memory().unwrap();
            backend.put("a:1", "v1").unwrap();
            backend.put("a:2", "v2").unwrap();
            backend.put("b:1", "v3").unwrap();
            let keys = backend.list("a:").unwrap();
            assert_eq!(keys.len(), 2);
        }
    }
}

pub use native::NativeStateBackend;

/// LocalStorage-backed state backend (WASM only).
#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use super::StateBackend;
    use std::cell::RefCell;

    pub struct WasmStateBackend {
        prefix: String,
        storage: RefCell<Option<web_sys::Storage>>,
    }

    impl WasmStateBackend {
        pub fn new(namespace: &str) -> Self {
            let storage = if let Some(window) = web_sys::window() {
                window.local_storage().ok().flatten()
            } else {
                None
            };
            Self {
                prefix: format!("gestalt:{}:", namespace),
                storage: RefCell::new(storage),
            }
        }

        fn prefixed(&self, key: &str) -> String {
            format!("{}{}", self.prefix, key)
        }
    }

    impl StateBackend for WasmStateBackend {
        fn put(&self, key: &str, value: &str) -> Result<(), String> {
            let storage = self.storage.borrow();
            let s = storage.as_ref().ok_or("localStorage not available")?;
            s.set_item(&self.prefixed(key), value)
                .map_err(|e| format!("localStorage set: {:?}", e))
        }

        fn get(&self, key: &str) -> Result<Option<String>, String> {
            let storage = self.storage.borrow();
            let s = storage.as_ref().ok_or("localStorage not available")?;
            s.get_item(&self.prefixed(key))
                .map_err(|e| format!("localStorage get: {:?}", e))
        }

        fn delete(&self, key: &str) -> Result<(), String> {
            let storage = self.storage.borrow();
            let s = storage.as_ref().ok_or("localStorage not available")?;
            s.remove_item(&self.prefixed(key))
                .map_err(|e| format!("localStorage remove: {:?}", e))
        }

        fn list(&self, prefix: &str) -> Result<Vec<String>, String> {
            let storage = self.storage.borrow();
            let s = storage.as_ref().ok_or("localStorage not available")?;
            let search = self.prefixed(prefix);
            let len = s.length().map_err(|e| format!("length: {:?}", e))?;
            let mut keys = Vec::new();
            for i in 0..len {
                if let Ok(Some(k)) = s.key(i) {
                    if k.starts_with(&search) {
                        keys.push(k[self.prefix.len()..].to_string());
                    }
                }
            }
            Ok(keys)
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::WasmStateBackend;
