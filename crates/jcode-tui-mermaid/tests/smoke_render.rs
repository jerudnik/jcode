//! End-to-end regression guard for the Mermaid renderer pipeline.
//!
//! `mermaid-rs-renderer` v0.2.1 emits CSS font stacks with unescaped nested
//! double quotes in the SVG `font-family` attribute, which made `usvg` reject
//! every diagram ("expected a whitespace not 'N'"). The wrapper now sanitizes
//! the SVG before parsing; this test renders a real flowchart through the
//! public API and asserts a valid PNG comes out.
#![cfg(feature = "renderer")]

#[test]
fn renders_flowchart_to_valid_png() {
    let content = "flowchart TD\n    A[Start] --> B[Done]\n";
    match jcode_tui_mermaid::render_mermaid_untracked(content, Some(48)) {
        jcode_tui_mermaid::RenderResult::Image {
            path,
            width,
            height,
            ..
        } => {
            assert!(width > 0 && height > 0, "render produced zero dimensions");
            assert!(path.exists(), "expected PNG written to {path:?}");
            let bytes = std::fs::read(&path).expect("read rendered PNG");
            assert!(
                bytes.starts_with(&[0x89, b'P', b'N', b'G']),
                "output is not a valid PNG"
            );
        }
        jcode_tui_mermaid::RenderResult::Error(e) => {
            panic!("mermaid render failed: {e}");
        }
    }
}
