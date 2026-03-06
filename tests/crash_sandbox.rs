//! Crash reproduction test for sandbox.php.
//!
//! This test loads the sandbox.php file and runs it through the full LSP
//! pipeline (parse, diagnostics, hover at specific lines) to reproduce
//! crashes that occur when processing complex real-world PHP files.

mod common;

use common::create_test_backend;
use phpantom_lsp::Backend;
use tower_lsp::lsp_types::*;

const SANDBOX_CONTENT: &str = include_str!("../sandbox.php");

/// Helper: open a file on the backend (parse + update AST).
fn open_file(backend: &Backend, uri: &str, content: &str) {
    backend.update_ast(uri, content);
}

/// Helper: run all diagnostic collectors on the file.
fn collect_all_diagnostics(backend: &Backend, uri: &str, content: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    backend.collect_deprecated_diagnostics(uri, content, &mut out);
    backend.collect_unused_import_diagnostics(uri, content, &mut out);
    backend.collect_unknown_class_diagnostics(uri, content, &mut out);
    backend.collect_unknown_member_diagnostics(uri, content, &mut out);
    out
}

// ── Parsing ─────────────────────────────────────────────────────────────

#[test]
fn sandbox_parse_does_not_crash() {
    let backend = create_test_backend();
    let uri = "file:///sandbox.php";
    open_file(&backend, uri, SANDBOX_CONTENT);
}

// ── Diagnostics ─────────────────────────────────────────────────────────

#[test]
fn sandbox_diagnostics_do_not_crash() {
    let backend = create_test_backend();
    let uri = "file:///sandbox.php";
    open_file(&backend, uri, SANDBOX_CONTENT);
    let _diags = collect_all_diagnostics(&backend, uri, SANDBOX_CONTENT);
}

#[test]
fn sandbox_deprecated_diagnostics_do_not_crash() {
    let backend = create_test_backend();
    let uri = "file:///sandbox.php";
    open_file(&backend, uri, SANDBOX_CONTENT);
    let mut out = Vec::new();
    backend.collect_deprecated_diagnostics(uri, SANDBOX_CONTENT, &mut out);
}

#[test]
fn sandbox_unused_import_diagnostics_do_not_crash() {
    let backend = create_test_backend();
    let uri = "file:///sandbox.php";
    open_file(&backend, uri, SANDBOX_CONTENT);
    let mut out = Vec::new();
    backend.collect_unused_import_diagnostics(uri, SANDBOX_CONTENT, &mut out);
}

#[test]
fn sandbox_unknown_class_diagnostics_do_not_crash() {
    let backend = create_test_backend();
    let uri = "file:///sandbox.php";
    open_file(&backend, uri, SANDBOX_CONTENT);
    let mut out = Vec::new();
    backend.collect_unknown_class_diagnostics(uri, SANDBOX_CONTENT, &mut out);
}

#[test]
fn sandbox_unknown_member_diagnostics_do_not_crash() {
    let backend = create_test_backend();
    let uri = "file:///sandbox.php";
    open_file(&backend, uri, SANDBOX_CONTENT);
    let mut out = Vec::new();
    backend.collect_unknown_member_diagnostics(uri, SANDBOX_CONTENT, &mut out);
}

// ── Minimal reproduction: nested closure hover crash ────────────────────

/// Hovering on `$q` inside a nested closure that reuses the same variable
/// name as the outer closure used to cause infinite recursion in the hover
/// handler.  Fixed by a thread-local recursion depth guard in
/// `infer_callable_params_from_receiver`.
#[test]
fn sandbox_hover_nested_closure_reused_variable_does_not_crash() {
    let backend = create_test_backend();
    let uri = "file:///nested_closure.php";
    let content = r#"<?php
namespace App;

class QueryBuilder {
    public function where(string $col, mixed $val = null): static { return $this; }
    public function whereNull(string $col): static { return $this; }
    public function orWhere(mixed ...$args): static { return $this; }
    public function whereHas(string $rel, \Closure $cb): static { return $this; }
}

class Repo {
    public function list(): void {
        $query = new QueryBuilder();
        $query->where(function ($q) {
            $q->whereNull('user_id')
                ->orWhere('user_id', 1)
                ->orWhere(function ($q): void {
                    $q->where('is_public', 1)
                        ->where('is_verified', 1);
                });
        });
    }
}
"#;
    open_file(&backend, uri, content);

    // Line 17 is `->orWhere(function ($q): void {` — the crashing line.
    // Hover at the midpoint which lands on `function` or `($q)`.
    let line_text = content.lines().nth(17).unwrap();
    let col = (line_text.len() / 2) as u32;
    let position = Position {
        line: 17,
        character: col,
    };

    // This must not stack-overflow.
    backend.handle_hover(uri, content, position);
}

// ── Hover — test individual lines to find stack overflow ────────────────
//
// The hover handler has an infinite recursion bug triggered by something
// in sandbox.php.  Testing one line at a time in separate test functions
// lets us identify exactly which line causes the stack overflow.

/// Hover over a range of lines.  Uses stacker to grow the stack on
/// demand so we can catch deep recursion as a panic instead of a
/// process-killing SIGABRT.
fn hover_line_range(start: u32, end: u32) {
    let backend = create_test_backend();
    let uri = "file:///sandbox.php";
    open_file(&backend, uri, SANDBOX_CONTENT);

    let mut crashed: Vec<(u32, String)> = Vec::new();

    for line in start..end {
        let line_text = SANDBOX_CONTENT
            .lines()
            .nth(line as usize)
            .unwrap_or("")
            .to_string();
        let col = (line_text.len() / 2) as u32;
        let position = Position {
            line,
            character: col,
        };

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            backend.handle_hover(uri, SANDBOX_CONTENT, position);
        }));
        if result.is_err() {
            crashed.push((line, line_text));
        }
    }

    assert!(
        crashed.is_empty(),
        "Hover crashed at {} lines: {:?}",
        crashed.len(),
        crashed,
    );
}

// Split hover testing into chunks of ~50 lines so that if one chunk
// crashes the test runner, the test name tells us which range to
// investigate.  Each test uses a thread with a 128 MB stack.

macro_rules! hover_chunk_test {
    ($name:ident, $start:expr, $end:expr) => {
        #[test]
        fn $name() {
            let result = std::thread::Builder::new()
                .name(stringify!($name).into())
                .stack_size(128 * 1024 * 1024)
                .spawn(move || {
                    hover_line_range($start, $end);
                })
                .expect("spawn thread")
                .join();

            assert!(
                result.is_ok(),
                "Stack overflow in hover for lines {}..{} — this is the range with infinite recursion",
                $start, $end,
            );
        }
    };
}

hover_chunk_test!(sandbox_hover_lines_000_050, 0, 50);
hover_chunk_test!(sandbox_hover_lines_050_100, 50, 100);
hover_chunk_test!(sandbox_hover_lines_100_105, 100, 105);
hover_chunk_test!(sandbox_hover_lines_105_110, 105, 110);
hover_chunk_test!(sandbox_hover_lines_110_111, 110, 111);
hover_chunk_test!(sandbox_hover_lines_111_112, 111, 112);
hover_chunk_test!(sandbox_hover_lines_112_113, 112, 113);
hover_chunk_test!(sandbox_hover_lines_113_114, 113, 114);
hover_chunk_test!(sandbox_hover_lines_114_115, 114, 115);
hover_chunk_test!(sandbox_hover_lines_115_120, 115, 120);
hover_chunk_test!(sandbox_hover_lines_120_125, 120, 125);
hover_chunk_test!(sandbox_hover_lines_125_130, 125, 130);
hover_chunk_test!(sandbox_hover_lines_130_135, 130, 135);
hover_chunk_test!(sandbox_hover_lines_135_140, 135, 140);
hover_chunk_test!(sandbox_hover_lines_140_145, 140, 145);
hover_chunk_test!(sandbox_hover_lines_145_150, 145, 150);
hover_chunk_test!(sandbox_hover_lines_150_200, 150, 200);
hover_chunk_test!(sandbox_hover_lines_200_300, 200, 300);
hover_chunk_test!(sandbox_hover_lines_300_400, 300, 400);
hover_chunk_test!(sandbox_hover_lines_400_500, 400, 500);
hover_chunk_test!(sandbox_hover_lines_500_610, 500, 610);
