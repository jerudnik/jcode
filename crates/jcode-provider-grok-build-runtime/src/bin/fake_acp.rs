// Recompile the shared scriptable fake as this package's test binary so Cargo
// exposes a stable CARGO_BIN_EXE path to the wrapper integration tests.
include!("../../../jcode-provider-acp-runtime/src/bin/fake_acp.rs");
