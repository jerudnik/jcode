use super::*;

#[test]
fn create_and_resume_goal_persists_project_goal() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path().join("repo");
    std::fs::create_dir_all(&project).expect("project dir");
    let _env_guard = EnvGuard::set("JCODE_HOME", temp.path());

    let goal = create_goal(
        GoalCreateInput {
            title: "Ship mobile MVP".to_string(),
            scope: GoalScope::Project,
            next_steps: vec!["finish reconnect flow".to_string()],
            progress_percent: Some(40),
            ..GoalCreateInput::default()
        },
        Some(&project),
    )
    .expect("create goal");
    assert_eq!(goal.id, "ship-mobile-mvp");

    let loaded = load_goal(&goal.id, Some(GoalScope::Project), Some(&project))
        .expect("load")
        .expect("goal exists");
    assert_eq!(loaded.title, "Ship mobile MVP");

    let manager = crate::memory::MemoryManager::new().with_project_dir(&project);
    let graph = manager.load_project_graph().expect("load graph");
    let goal_mem = graph
        .get_memory(&format!("goal:{}", goal.id))
        .expect("goal memory mirror");
    assert!(goal_mem.tags.iter().any(|tag| tag == "goal"));
    assert!(goal_mem.content.contains("Ship mobile MVP"));

    let session_id = "ses_goal_test";
    attach_goal_to_session(session_id, &goal, Some(&project)).expect("attach");
    let resumed = resume_goal(session_id, Some(&project))
        .expect("resume")
        .expect("goal resumed");
    assert_eq!(resumed.id, goal.id);
}

#[test]
fn write_goal_page_auto_focuses_first_goal_only() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path().join("repo");
    std::fs::create_dir_all(&project).expect("project dir");
    let _env_guard = EnvGuard::set("JCODE_HOME", temp.path());

    let session_id = "ses_goal_panel";
    let goal = create_goal(
        GoalCreateInput {
            title: "Ship mobile MVP".to_string(),
            scope: GoalScope::Project,
            ..GoalCreateInput::default()
        },
        Some(&project),
    )
    .expect("create goal");

    let first = write_goal_page(session_id, Some(&project), &goal, GoalDisplayMode::Auto)
        .expect("first write");
    assert_eq!(
        first.focused_page_id.as_deref(),
        Some("goal.ship-mobile-mvp")
    );

    crate::side_panel::write_markdown_page(session_id, "notes", Some("Notes"), "# Notes", true)
        .expect("notes");
    let second = write_goal_page(session_id, Some(&project), &goal, GoalDisplayMode::Auto)
        .expect("second write");
    assert_eq!(second.focused_page_id.as_deref(), Some("notes"));
}

#[test]
fn create_goal_empty_title_fails() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _env_guard = EnvGuard::set("JCODE_HOME", temp.path());

    let result = create_goal(
        GoalCreateInput {
            title: "   ".to_string(),
            scope: GoalScope::Global,
            ..GoalCreateInput::default()
        },
        None,
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "goal title cannot be empty"
    );
}

#[test]
fn create_goal_uses_custom_id() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _env_guard = EnvGuard::set("JCODE_HOME", temp.path());

    let goal = create_goal(
        GoalCreateInput {
            id: Some("  My Custom ID !  ".to_string()),
            title: "Custom ID Goal".to_string(),
            scope: GoalScope::Global,
            ..GoalCreateInput::default()
        },
        None,
    )
    .expect("create goal");

    assert_eq!(goal.id, "my-custom-id");
}

#[test]
fn create_goal_resolves_id_conflicts() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _env_guard = EnvGuard::set("JCODE_HOME", temp.path());

    let goal1 = create_goal(
        GoalCreateInput {
            title: "Duplicate Goal".to_string(),
            scope: GoalScope::Global,
            ..GoalCreateInput::default()
        },
        None,
    )
    .expect("create first goal");
    assert_eq!(goal1.id, "duplicate-goal");

    let goal2 = create_goal(
        GoalCreateInput {
            title: "Duplicate Goal".to_string(),
            scope: GoalScope::Global,
            ..GoalCreateInput::default()
        },
        None,
    )
    .expect("create second goal");
    assert_eq!(goal2.id, "duplicate-goal-2");

    let goal3 = create_goal(
        GoalCreateInput {
            title: "Duplicate Goal".to_string(),
            scope: GoalScope::Global,
            ..GoalCreateInput::default()
        },
        None,
    )
    .expect("create third goal");
    assert_eq!(goal3.id, "duplicate-goal-3");
}

#[test]
fn create_goal_clamps_progress() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _env_guard = EnvGuard::set("JCODE_HOME", temp.path());

    let goal = create_goal(
        GoalCreateInput {
            title: "Progress Clamp".to_string(),
            scope: GoalScope::Global,
            progress_percent: Some(150),
            ..GoalCreateInput::default()
        },
        None,
    )
    .expect("create goal");

    assert_eq!(goal.progress_percent, Some(100));
}

#[test]
fn create_goal_trims_global_fields_and_syncs_memory() {
    let _guard = crate::storage::lock_test_env();
    let temp = tempfile::tempdir().expect("tempdir");
    let _env_guard = EnvGuard::set("JCODE_HOME", temp.path());

    let goal = create_goal(
        GoalCreateInput {
            title: "Learn Rust".to_string(),
            scope: GoalScope::Global,
            description: Some("   Master borrow checker   ".to_string()),
            why: Some("   To write safe concurrent code   ".to_string()),
            success_criteria: vec!["   Write a web server   ".to_string(), " ".to_string()],
            ..GoalCreateInput::default()
        },
        None,
    )
    .expect("create goal");

    assert_eq!(goal.id, "learn-rust");
    assert_eq!(goal.description, "Master borrow checker");
    assert_eq!(goal.why, "To write safe concurrent code");
    assert_eq!(
        goal.success_criteria,
        vec!["Write a web server".to_string()]
    );

    let loaded = load_goal(&goal.id, Some(GoalScope::Global), None)
        .expect("load")
        .expect("goal exists");
    assert_eq!(loaded.title, "Learn Rust");
    assert_eq!(loaded.description, "Master borrow checker");
    assert_eq!(loaded.why, "To write safe concurrent code");
    assert_eq!(
        loaded.success_criteria,
        vec!["Write a web server".to_string()]
    );

    let manager = crate::memory::MemoryManager::new();
    let graph = manager.load_global_graph().expect("load graph");
    let goal_mem = graph
        .get_memory(&format!("goal:{}", goal.id))
        .expect("goal memory mirror");
    assert!(goal_mem.tags.iter().any(|tag| tag == "goal"));
    assert!(goal_mem.content.contains("Learn Rust"));
}

struct EnvGuard {
    key: &'static str,
    saved: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, val: impl AsRef<std::ffi::OsStr>) -> Self {
        let saved = std::env::var_os(key);
        crate::env::set_var(key, val);
        Self { key, saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(val) = &self.saved {
            crate::env::set_var(self.key, val);
        } else {
            crate::env::remove_var(self.key);
        }
    }
}
