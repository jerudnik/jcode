use crate::storage;
use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};

pub use jcode_task_types::TodoItem;

pub fn load_todos(session_id: &str) -> Result<Vec<TodoItem>> {
    let path = todo_path(session_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    storage::read_json(&path).or_else(|_| Ok(Vec::new()))
}

pub fn todos_exist(session_id: &str) -> Result<bool> {
    Ok(todo_path(session_id)?.exists())
}

pub fn save_todos(session_id: &str, todos: &[TodoItem]) -> Result<()> {
    let path = todo_path(session_id)?;
    storage::write_json_fast(&path, todos)
}

fn todo_path(session_id: &str) -> Result<PathBuf> {
    validate_todo_session_id(session_id)?;
    let base = storage::jcode_dir()?;
    Ok(base.join("todos").join(format!("{}.json", session_id)))
}

fn validate_todo_session_id(session_id: &str) -> Result<()> {
    if session_id.is_empty()
        || Path::new(session_id)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || session_id.contains(std::path::MAIN_SEPARATOR)
        || session_id.contains('/')
        || session_id.contains('\\')
    {
        bail!("invalid todo session id: {session_id:?}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TestHome {
        _lock: crate::storage::TestEnvLockGuard,
        _dir: TempDir,
        previous_home: Option<std::ffi::OsString>,
    }

    impl TestHome {
        fn new() -> Self {
            let lock = crate::storage::lock_test_env();
            let previous_home = std::env::var_os("JCODE_HOME");
            let dir = tempfile::tempdir().expect("create temp JCODE_HOME");
            jcode_core::env::set_var("JCODE_HOME", dir.path());
            Self {
                _lock: lock,
                _dir: dir,
                previous_home,
            }
        }
    }

    impl Drop for TestHome {
        fn drop(&mut self) {
            if let Some(home) = &self.previous_home {
                jcode_core::env::set_var("JCODE_HOME", home);
            } else {
                jcode_core::env::remove_var("JCODE_HOME");
            }
        }
    }

    fn todo(id: &str, status: &str) -> TodoItem {
        TodoItem {
            id: id.to_string(),
            content: format!("task {id}"),
            status: status.to_string(),
            priority: "high".to_string(),
            blocked_by: Vec::new(),
            assigned_to: None,
        }
    }

    #[test]
    fn save_and_load_todos_roundtrip() {
        let _home = TestHome::new();
        let todos = vec![todo("one", "pending"), todo("two", "completed")];

        save_todos("session-1", &todos).expect("save todos");
        let loaded = load_todos("session-1").expect("load todos");

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "one");
        assert_eq!(loaded[1].status, "completed");
        assert!(todos_exist("session-1").expect("todos exist check"));
    }

    #[test]
    fn load_todos_recovers_from_backup_when_primary_is_corrupt() {
        let _home = TestHome::new();
        let old_todos = vec![todo("old", "pending")];
        let new_todos = vec![todo("new", "completed")];

        save_todos("session-2", &old_todos).expect("save old todos");
        save_todos("session-2", &new_todos).expect("save new todos creates backup");

        let path = todo_path("session-2").expect("todo path");
        std::fs::write(&path, b"not json").expect("corrupt primary todo file");

        let loaded = load_todos("session-2").expect("load recovered todos");

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "old");
    }

    #[test]
    fn load_todos_returns_empty_for_unrecoverable_corrupt_file() {
        let _home = TestHome::new();
        let path = todo_path("session-3").expect("todo path");
        std::fs::create_dir_all(path.parent().expect("todo parent")).expect("create todo dir");
        std::fs::write(&path, b"not json").expect("write corrupt todo file");

        let loaded = load_todos("session-3").expect("corrupt todo files are non-fatal");

        assert!(loaded.is_empty());
    }

    #[test]
    fn todo_path_rejects_path_traversal_session_ids() {
        let _home = TestHome::new();

        assert!(todo_path("../outside").is_err());
        assert!(todo_path("nested/session").is_err());
        assert!(todo_path("nested\\session").is_err());
        assert!(todo_path("").is_err());
    }
}
