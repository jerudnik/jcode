#[test]
fn artifact_type_system_compile_contracts() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/secret_path_rejects_config_lock.rs");
    tests.compile_fail("tests/ui/durable_path_rejects_claude_oauth.rs");
    tests.compile_fail("tests/ui/external_secret_is_read_only.rs");
    tests.compile_fail("tests/ui/session_inbox_rejects_string_key.rs");
    tests.pass("tests/ui/artifact_id_is_exhaustive.rs");
}
