from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from textwrap import dedent

REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS_DIR))

import check_panic_budget  # noqa: E402
import check_swallowed_error_budget  # noqa: E402
from rust_production_filter import is_test_rust_file, production_lines, production_lines_from_text  # noqa: E402


def production_text(source: str) -> str:
    return "\n".join(production_lines_from_text(dedent(source)))


class RustProductionFilterTests(unittest.TestCase):
    def test_budget_scripts_use_the_shared_classifier(self) -> None:
        self.assertIs(check_panic_budget.production_lines, production_lines)
        self.assertIs(check_swallowed_error_budget.production_lines, production_lines)

    def test_normal_production_code_is_retained(self) -> None:
        output = production_text(
            """
            pub fn production() {
                let _ = do_work();
                panic!("real production panic");
            }
            """
        )

        self.assertIn("let _ = do_work();", output)
        self.assertIn("panic!(\"real production panic\");", output)

    def test_inline_cfg_test_module_is_excluded(self) -> None:
        output = production_text(
            """
            pub fn production() { panic!("keep me"); }
            #[cfg(test)]
            mod tests {
                #[test]
                fn panics_in_tests_are_ignored() {
                    panic!("test-only panic");
                }
            }
            """
        )

        self.assertIn("keep me", output)
        self.assertNotIn("test-only panic", output)

    def test_cfg_test_module_with_opening_brace_on_next_line_is_excluded(self) -> None:
        output = production_text(
            """
            #[cfg(test)]
            mod tests
            {
                fn swallowed_in_tests() {
                    let _ = only_for_tests();
                }
            }
            pub fn production() { let _ = still_counted(); }
            """
        )

        self.assertNotIn("only_for_tests", output)
        self.assertIn("let _ = still_counted();", output)

    def test_closing_braces_inside_comments_and_strings_do_not_end_test_module(self) -> None:
        output = production_text(
            r'''
            #[cfg(test)]
            mod tests {
                const STRING_BRACE: &str = "}";
                // } comment brace must not close the module
                /* } block comment brace must not close the module */
                fn ignored() {
                    panic!("test-only panic after fake braces");
                }
            }
            pub fn production() { panic!("production panic"); }
            '''
        )

        self.assertNotIn("test-only panic after fake braces", output)
        self.assertIn("production panic", output)

    def test_raw_strings_do_not_end_test_module(self) -> None:
        output = production_text(
            r'''
            #[cfg(test)]
            mod tests {
                const RAW: &str = r#"}"#;
                fn ignored() {
                    let _ = only_for_tests();
                }
            }
            pub fn production() { let _ = still_counted(); }
            '''
        )

        self.assertNotIn("only_for_tests", output)
        self.assertIn("still_counted", output)

    def test_multiline_cfg_test_attribute_is_excluded(self) -> None:
        output = production_text(
            """
            #[cfg(
                test
            )]
            mod tests {
                fn ignored() { panic!("multiline cfg test panic"); }
            }
            pub fn production() { panic!("production panic"); }
            """
        )

        self.assertNotIn("multiline cfg test panic", output)
        self.assertIn("production panic", output)

    def test_nested_attributes_after_cfg_test_are_excluded_with_the_item(self) -> None:
        output = production_text(
            """
            #[cfg(test)]
            #[allow(dead_code)]
            mod tests {
                fn ignored() { let _ = test_only(); }
            }
            pub fn production() { let _ = still_counted(); }
            """
        )

        self.assertNotIn("test_only", output)
        self.assertIn("still_counted", output)

    def test_cfg_test_function_with_opening_brace_on_next_line_is_excluded(self) -> None:
        output = production_text(
            """
            #[cfg(test)]
            fn helper_only_in_tests()
            {
                panic!("test-only helper panic");
            }
            #[cfg(not(test))]
            fn cfg_not_test_is_not_silently_excluded() {
                panic!("cfg-not-test production panic");
            }
            """
        )

        self.assertNotIn("test-only helper panic", output)
        self.assertIn("cfg-not-test production panic", output)

    def test_cfg_all_requiring_test_is_excluded_but_cfg_any_production_path_is_retained(self) -> None:
        output = production_text(
            """
            #[cfg(all(test, unix))]
            mod unix_tests {
                fn ignored() { panic!("all-test panic"); }
            }
            #[cfg(any(test, target_os = "macos"))]
            fn can_compile_outside_tests() {
                panic!("platform production panic");
            }
            """
        )

        self.assertNotIn("all-test panic", output)
        self.assertIn("platform production panic", output)

    def test_cfg_test_direct_braced_items_are_excluded(self) -> None:
        output = production_text(
            """
            #[cfg(test)]
            impl Widget { fn helper() { panic!("impl panic"); } }
            #[cfg(test)]
            pub(crate) struct TestStruct { field: usize }
            #[cfg(test)]
            enum TestEnum { Variant }
            #[cfg(test)]
            trait TestTrait { fn helper(&self) { panic!("trait panic"); } }
            #[cfg(test)]
            extern "C" { fn test_symbol(); }
            pub fn production() { panic!("production panic"); }
            """
        )

        self.assertNotIn("impl panic", output)
        self.assertNotIn("TestStruct", output)
        self.assertNotIn("TestEnum", output)
        self.assertNotIn("trait panic", output)
        self.assertNotIn("test_symbol", output)
        self.assertIn("production panic", output)

    def test_cfg_test_direct_semicolon_items_are_excluded(self) -> None:
        output = production_text(
            """
            #[cfg(test)]
            type TestAlias = Result<(), String>;
            #[cfg(test)]
            const TEST_CONST: &[usize] = &[1; 3];
            #[cfg(test)]
            static TEST_STATIC: usize = 1;
            #[cfg(test)]
            pub use crate::test_helpers::helper;
            pub fn production() { let _ = still_counted(); }
            """
        )

        self.assertNotIn("TestAlias", output)
        self.assertNotIn("TEST_CONST", output)
        self.assertNotIn("TEST_STATIC", output)
        self.assertNotIn("test_helpers", output)
        self.assertIn("still_counted", output)

    def test_cfg_test_macro_definitions_are_excluded_but_macro_invocations_stay_conservative(self) -> None:
        output = production_text(
            """
            #[cfg(test)]
            macro_rules! test_macro { () => { panic!("macro_rules panic") }; }
            #[cfg(test)]
            macro test_macro2() { panic!("macro item panic"); }
            #[cfg(test)]
            thread_local! { static TEST_ONLY: usize = 1; }
            pub fn production() { panic!("production panic"); }
            """
        )

        self.assertNotIn("macro_rules panic", output)
        self.assertNotIn("macro item panic", output)
        self.assertIn("TEST_ONLY", output)
        self.assertIn("production panic", output)

    def test_leading_file_level_cfg_test_excludes_entire_file(self) -> None:
        output = production_text(
            """
            #![allow(dead_code)]
            #![cfg(test)]

            pub fn helper() { panic!("file-level test panic"); }
            """
        )

        self.assertNotIn("file-level test panic", output)

    def test_file_level_cfg_any_test_with_production_condition_is_retained(self) -> None:
        output = production_text(
            """
            #![cfg(any(test, target_os = "macos"))]

            pub fn can_compile_in_production() { panic!("retained panic"); }
            """
        )

        self.assertIn("retained panic", output)

    def test_rust_lifetimes_are_not_treated_as_character_literals(self) -> None:
        output = production_text(
            """
            pub fn production<'a, 'b>(left: &'a str, right: &'b str) {
                let _ = (left, right);
            }
            #[cfg(test)]
            mod tests {
                fn ignored() { panic!("test-only panic"); }
            }
            """
        )

        self.assertIn("let _ = (left, right);", output)
        self.assertNotIn("test-only panic", output)

    def test_test_rust_file_policy_is_preserved(self) -> None:
        cases = {
            "tests/integration.rs": True,
            "crates/example/src/tests.rs": True,
            "crates/example/src/parser_tests.rs": True,
            "crates/example/src/tests_helpers/mod.rs": True,
            "crates/example/src/lib.rs": False,
        }

        with tempfile.TemporaryDirectory() as temp_dir:
            repo = Path(temp_dir)
            for rel_path, expected in cases.items():
                path = repo / rel_path
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("", encoding="utf-8")
                self.assertIs(is_test_rust_file(path, repo), expected)


if __name__ == "__main__":
    unittest.main()
