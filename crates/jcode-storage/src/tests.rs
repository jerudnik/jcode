//! Unit tests for the ambient filesystem roots.
//!
//! These live in-crate rather than under `tests/` because they exercise the
//! private pure resolvers alongside the public entry points.

mod home_isolation_tests {
    use crate::*;

    /// The layouts cargo actually produces, captured from a real toolchain:
    /// a harness binary lands in `deps/` with a metadata hash, while `cargo
    /// run`, a direct execution, and an installed binary never do.
    #[test]
    fn classifies_cargo_layouts_from_observed_paths() {
        for path in [
            "/private/tmp/probe_t/target/debug/deps/probe_t-a39295051af45621",
            "/repo/target/debug/deps/jcode_app_core-0f868b93b8de0cac",
            "/repo/target/selfdev/deps/integration_test-0123456789abcdef",
        ] {
            assert!(
                is_cargo_test_binary_path(Path::new(path)),
                "expected test-harness layout: {path}"
            );
        }

        for path in [
            // `cargo run` and direct execution: sibling of `deps`, not inside.
            "/private/tmp/probe_t/target/debug/probe_t",
            "/repo/target/debug/jcode",
            "/repo/target/release/jcode",
            // Installed binaries.
            "/usr/local/bin/jcode",
            "/Users/someone/.jcode/current/jcode",
            "/nix/store/abcdef-jcode-0.1.0/bin/jcode",
            // A `deps` directory that is not cargo's: no metadata hash.
            "/home/someone/deps/jcode",
            "/home/someone/deps/jcode-1.2.3",
            // Hash-shaped but too short to be cargo metadata.
            "/repo/target/debug/deps/jcode-abc123",
            // Non-hex suffix.
            "/repo/target/debug/deps/jcode-zzzzzzzzzzzzzzzz",
            // Empty binary name.
            "/repo/target/debug/deps/-0123456789abcdef",
        ] {
            assert!(
                !is_cargo_test_binary_path(Path::new(path)),
                "expected NOT a test-harness layout: {path}"
            );
        }
    }

    /// The guard must not fire when the caller pinned `JCODE_HOME`, and this
    /// very test binary must be classified as a harness (self-referential
    /// check: if the classifier regresses, this fails in the suite it guards).
    #[test]
    fn this_test_binary_is_classified_as_a_harness() {
        let exe = std::env::current_exe().expect("current_exe");
        assert!(
            is_cargo_test_binary_path(&exe),
            "this test binary should match the harness layout: {}",
            exe.display()
        );
    }

    /// `jcode_dir()` must never resolve to the real `~/.jcode` from a test.
    ///
    /// Asserts against `test_harness_home()` rather than by clearing
    /// `JCODE_HOME`: mutating that variable would race every concurrently
    /// running test through the global config-cache fingerprint, which is the
    /// exact defect class `scripts/check_config_env_lease.py` gates. The
    /// redirect is what needs proving, and it is observable directly.
    #[test]
    fn test_harness_home_is_never_the_real_home() {
        let redirected = test_harness_home().expect("test binaries must redirect");
        let real_home = dirs::home_dir().map(|home| home.join(".jcode"));

        assert_ne!(
            Some(&redirected),
            real_home.as_ref(),
            "a test resolved the developer's real jcode home"
        );
        assert!(
            redirected.starts_with(std::env::temp_dir()),
            "the redirect must land under the temp dir, got {}",
            redirected.display()
        );
        assert!(
            redirected.is_dir(),
            "the redirect target must exist: {}",
            redirected.display()
        );
    }

    /// Every ambient root must redirect, not just `~/.jcode`.
    ///
    /// `jcode_dir` was isolated first, which left two other roots resolving to
    /// real user state: the platform config dir (`app_config_dir`) and the home
    /// dir itself (`user_home_path`). The config dir holds
    /// `model_picker_usage.json`, whose contents feed the model picker's sort
    /// key, so five TUI tests passed or failed based on which models the
    /// developer had personally selected. Enumerated as one test so adding a
    /// fourth root without isolating it is visibly an omission.
    ///
    /// Asserted as "never under a real user root" rather than "always under
    /// the harness home": sibling tests in this binary set `JCODE_HOME` to
    /// their own temp dirs without restoring it, so the live wrappers may
    /// legitimately resolve there. Both destinations are isolated, which is
    /// the property that matters, and phrasing it this way makes the test
    /// independent of execution order. The exact redirect is pinned by
    /// `ambient_roots_keep_their_real_locations_outside_tests` against the
    /// pure resolvers, where no env can interfere.
    #[test]
    fn every_ambient_root_redirects_under_a_test_harness() {
        let real_home = dirs::home_dir().expect("home dir");
        let real_config = dirs::config_dir().expect("config dir");
        let real_cache = dirs::cache_dir().expect("cache dir");

        let roots: [(&str, PathBuf); 4] = [
            ("jcode_dir", jcode_dir().expect("jcode_dir")),
            ("app_config_dir", app_config_dir().expect("app_config_dir")),
            ("app_cache_dir", app_cache_dir().expect("app_cache_dir")),
            (
                "user_home_path",
                user_home_path(".aws/credentials").expect("user_home_path"),
            ),
        ];

        for (name, resolved) in roots {
            assert!(
                !resolved.starts_with(&real_cache),
                "{name} resolved into the developer's real cache dir: {}",
                resolved.display()
            );
            assert!(
                !resolved.starts_with(&real_config),
                "{name} resolved into the developer's real config dir: {}",
                resolved.display()
            );
            // Checked after the config dir because on some platforms the
            // config dir is itself under the home dir, and the more specific
            // message is the more useful one.
            assert!(
                !resolved.starts_with(&real_home),
                "{name} resolved into the developer's real home: {}",
                resolved.display()
            );
        }
    }

    /// The redirect must not change where real users' files live.
    ///
    /// Pins the non-test behaviour of each root, so isolating a root cannot
    /// silently relocate a shipped binary's state. Drives the pure resolvers
    /// with explicit inputs, which is the only way to exercise the
    /// non-harness branch from inside a harness.
    #[test]
    fn ambient_roots_keep_their_real_locations_outside_tests() {
        let real_home = PathBuf::from("/home/real");
        let real_config = PathBuf::from("/home/real/.config");

        // No JCODE_HOME, no harness: the real platform locations, unchanged.
        assert_eq!(
            resolve_app_config_dir(None, None, Some(real_config.clone())).expect("app config"),
            real_config.join("jcode"),
            "a shipped binary must still use the platform config dir"
        );
        assert_eq!(
            resolve_user_home_path(
                Path::new(".aws/credentials"),
                None,
                None,
                Some(real_home.clone())
            )
            .expect("user home path"),
            real_home.join(".aws/credentials"),
            "a shipped binary must still use the real home dir"
        );

        // Harness with no JCODE_HOME: redirected away from the real roots.
        let harness = PathBuf::from("/tmp/harness-home");
        assert_eq!(
            resolve_app_config_dir(None, Some(&harness), Some(real_config.clone()))
                .expect("app config"),
            harness.join("config").join("jcode")
        );
        assert_eq!(
            resolve_user_home_path(
                Path::new(".aws/credentials"),
                None,
                Some(&harness),
                Some(real_home.clone())
            )
            .expect("user home path"),
            harness.join("external").join(".aws/credentials")
        );

        // JCODE_HOME wins outright, including over the harness redirect, so an
        // explicitly sandboxed run lands exactly where it asked to.
        let pinned = PathBuf::from("/tmp/pinned-home");
        assert_eq!(
            resolve_app_config_dir(Some(&pinned), Some(&harness), Some(real_config))
                .expect("app config"),
            pinned.join("config").join("jcode")
        );
        assert_eq!(
            resolve_user_home_path(
                Path::new(".aws/credentials"),
                Some(&pinned),
                Some(&harness),
                Some(real_home)
            )
            .expect("user home path"),
            pinned.join("external").join(".aws/credentials")
        );

        // An absolute relative-path argument is still rejected.
        assert!(
            resolve_user_home_path(Path::new("/etc/passwd"), None, None, None).is_err(),
            "absolute paths must be rejected"
        );
    }

    /// The `Result` and `Option` forms must not drift apart.
    ///
    /// `user_home_path_opt` exists so ~20 call sites stop writing
    /// `user_home_path(..).ok()`, which is only safe while the two agree on
    /// every input except the absolute-path contract that the `_opt` form
    /// asserts before it ever reaches the resolver. Pin that here, since a
    /// divergence would silently redirect real files.
    #[test]
    fn result_and_option_resolvers_agree_on_every_ambient_combination() {
        let real_home = PathBuf::from("/Users/real");
        let harness = PathBuf::from("/tmp/harness-home");
        let pinned = PathBuf::from("/tmp/pinned-home");
        let relative = Path::new(".aws/credentials");

        for jcode_home in [None, Some(pinned.as_path())] {
            for harness_home in [None, Some(harness.as_path())] {
                for real in [None, Some(real_home.clone())] {
                    let via_result =
                        resolve_user_home_path(relative, jcode_home, harness_home, real.clone())
                            .ok();
                    let via_option = resolve_user_home_path_opt(
                        relative,
                        jcode_home,
                        harness_home,
                        real.clone(),
                    );
                    assert_eq!(
                        via_result, via_option,
                        "resolvers disagreed for jcode_home={jcode_home:?} \
                         harness={harness_home:?} real_home={real:?}"
                    );
                }
            }
        }
    }

    /// A missing home is the *only* way the `Option` form yields `None`.
    #[test]
    fn option_resolver_returns_none_only_when_the_home_is_missing() {
        let relative = Path::new(".aws/credentials");
        assert_eq!(
            resolve_user_home_path_opt(relative, None, None, None),
            None,
            "no home anywhere must be None"
        );
        assert!(
            resolve_user_home_path_opt(relative, None, None, Some(PathBuf::from("/Users/real")))
                .is_some(),
            "a present home must always resolve"
        );
    }

    /// A blank `JCODE_HOME` must not be trusted as a real path, at *any* root.
    ///
    /// Taken literally, `JCODE_HOME="\t"` is a *relative* path, so every
    /// consumer wrote real state (telemetry ids, sessions, selfdev build
    /// requests) into a directory named `"\t"` under the current working
    /// directory. That is exactly the ambient-root escape F29 exists to close,
    /// and it was reachable from a shipped binary, not just tests: the repo
    /// root accumulated one from the suite itself.
    ///
    /// Drives the *public* entry points through the real environment rather
    /// than the pure resolvers, because the defect was in how each root read
    /// the variable, which a resolver taking pre-parsed arguments cannot see.
    /// Covers all four because three had the identical defect; testing only the
    /// one that happened to leak would leave the rest live.
    ///
    /// Reverting `jcode_home_override` to a bare `var_os` fails this.
    #[test]
    fn blank_jcode_home_falls_back_at_every_ambient_root() {
        let _lease = crate::lock_test_env_write();
        let previous = std::env::var_os("JCODE_HOME");

        for blank in ["", " ", "\t", "\n", "  \t "] {
            // SAFETY: mutation is serialized by the test-environment lease.
            unsafe { std::env::set_var("JCODE_HOME", blank) };

            let roots = [
                ("jcode_dir", jcode_dir().expect("jcode dir")),
                ("app_config_dir", app_config_dir().expect("config dir")),
                ("app_cache_dir", app_cache_dir().expect("cache dir")),
                (
                    "user_home_path",
                    user_home_path(".aws/credentials").expect("user home path"),
                ),
            ];

            for (name, resolved) in roots {
                assert!(
                    resolved.is_absolute(),
                    "{name} resolved to a relative path for JCODE_HOME={blank:?}, \
                     which lands under the current working directory: {}",
                    resolved.display()
                );
            }
        }

        // A real override is still honored, at the root that leaked.
        // SAFETY: same lease.
        unsafe { std::env::set_var("JCODE_HOME", "/tmp/pinned") };
        assert_eq!(
            jcode_dir().expect("explicit override"),
            PathBuf::from("/tmp/pinned")
        );

        // SAFETY: same lease; restore what the process had.
        unsafe {
            match previous {
                Some(value) => std::env::set_var("JCODE_HOME", value),
                None => std::env::remove_var("JCODE_HOME"),
            }
        }
    }

    /// The blank-rejecting rule itself, which every root now shares.
    #[test]
    fn jcode_home_override_rejects_blank_values_and_keeps_real_ones() {
        let _lease = crate::lock_test_env_write();
        let previous = std::env::var_os("JCODE_HOME");

        for blank in ["", " ", "\t", "\n", "  \t "] {
            // SAFETY: mutation is serialized by the test-environment lease.
            unsafe { std::env::set_var("JCODE_HOME", blank) };
            assert_eq!(
                jcode_home_override(),
                None,
                "JCODE_HOME={blank:?} must be treated as unset"
            );
            assert!(
                !home_is_redirected() || test_harness_home().is_some(),
                "JCODE_HOME={blank:?} must not report the home as redirected"
            );
        }

        // SAFETY: same lease.
        unsafe { std::env::set_var("JCODE_HOME", "/tmp/pinned") };
        assert_eq!(
            jcode_home_override(),
            Some(OsString::from("/tmp/pinned")),
            "a real override must survive"
        );

        // SAFETY: same lease; restore what the process had.
        unsafe {
            match previous {
                Some(value) => std::env::set_var("JCODE_HOME", value),
                None => std::env::remove_var("JCODE_HOME"),
            }
        }
    }
}

mod empty_relative_tests {
    #[test]
    fn empty_relative_yields_the_home_root_itself() {
        // `resolve_target_dir` uses `user_home_path("")` to mean "$HOME". Assert
        // that spelling actually yields a directory root and not something with
        // a stray trailing component, on both the redirected and real paths.
        let redirected = crate::user_home_path("").expect("home under harness");
        crate::assert_redirected_away_from_real_home(&redirected, "empty-relative home");
        assert!(
            redirected.is_absolute(),
            "expected an absolute home root, got {}",
            redirected.display()
        );
    }
}

mod sanitize_override_tests {
    use crate::*;

    /// The whole point of the filter: an override aimed inside the real home is
    /// dropped while the home is redirected (the Linux CI case, where
    /// `XDG_CONFIG_HOME=/home/runner/.config`), so callers fall through to
    /// their sandboxed default.
    #[test]
    fn drops_an_override_pointing_into_the_real_home() {
        let real_home = dirs::home_dir().expect("home");
        assert!(home_is_redirected(), "tests run under the harness redirect");
        assert_eq!(
            sanitize_ambient_dir_override(Some(real_home.join(".config").into_os_string())),
            None
        );
    }

    /// A neutral override is honored: tests point `$XDG_CONFIG_HOME` at fixture
    /// dirs on purpose, and users have non-default layouts. Dropping those too
    /// would trade one bug for another.
    #[test]
    fn honors_a_neutral_override() {
        let temp = std::env::temp_dir().join("jcode-neutral-override");
        assert_eq!(
            sanitize_ambient_dir_override(Some(temp.clone().into_os_string())),
            Some(temp)
        );
    }

    #[test]
    fn absent_override_stays_absent() {
        assert_eq!(sanitize_ambient_dir_override(None), None);
    }
}
