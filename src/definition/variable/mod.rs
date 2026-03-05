/// Variable definition resolution.
///
/// This module handles go-to-definition for `$variable` references,
/// jumping from a variable usage to its most recent assignment or
/// declaration site.
///
/// The primary path parses the file into an AST and walks the enclosing
/// scope to find the variable's definition site with byte-accurate
/// offsets.  This correctly handles:
///   - Array destructuring: `[$a, $b] = explode(',', $str)`
///   - List destructuring:  `list($a, $b) = func()`
///   - Multi-line parameter lists
///   - Nested scopes (closures, arrow functions)
///
/// Supported definition sites (searched bottom-up from cursor):
///   - **Assignment**: `$var = …` (but not `==` / `===`)
///   - **Parameter**: `Type $var` in a function/method signature
///   - **Foreach**: `as $var` / `=> $var`
///   - **Catch**: `catch (…Exception $var)`
///   - **Static / global**: `static $var` / `global $var`
///   - **Array destructuring**: `[$a, $b] = …` / `list($a, $b) = …`
///
/// When the cursor is already at the definition site (e.g. on a
/// parameter or assignment LHS), GTD returns `None` — the user is
/// already at the definition.  Type hints next to the variable
/// (e.g. `Throwable` in `catch (Throwable $it)`) are separate
/// symbol spans that the user can click directly.
///
/// When the AST parse fails (malformed PHP, parser panic), the function
/// returns `None` rather than falling back to text heuristics.
///
/// ## Submodules
///
/// - [`var_definition`]: AST walk that finds variable definition sites
///   (assignments, parameters, foreach, catch, static/global,
///   destructuring).
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::parser::with_parsed_program;
use crate::util::{offset_to_position, position_to_offset};

mod var_definition;

use var_definition::find_variable_definition_in_program;

// ═══════════════════════════════════════════════════════════════════════
// AST-based variable definition search result
// ═══════════════════════════════════════════════════════════════════════

/// Result of searching for a variable definition in the AST.
#[derive(Default)]
pub(super) enum VarDefSearchResult {
    /// No definition site found for this variable in the current scope.
    #[default]
    NotFound,
    /// The cursor is already sitting on the definition site (e.g. on a
    /// parameter declaration).  The caller should return `None`.
    AtDefinition,
    /// Found a prior definition at the given byte offset.
    /// `offset` is the start of the `$var` token, `end_offset` is the end.
    FoundAt { offset: u32, end_offset: u32 },
}

impl Backend {
    // ──────────────────────────────────────────────────────────────────────
    // Variable go-to-definition helpers
    // ──────────────────────────────────────────────────────────────────────

    /// Find the most recent assignment or declaration of `$var_name` before
    /// `position` and return its location.
    ///
    /// Parses the file into an AST and walks the enclosing scope to find
    /// the definition site with exact byte offsets.  Returns `None` when
    /// the AST parse fails or no definition is found.
    pub(super) fn resolve_variable_definition(
        content: &str,
        uri: &str,
        position: Position,
        var_name: &str,
    ) -> Option<Location> {
        Self::resolve_variable_definition_ast(content, uri, position, var_name)?
    }

    /// AST-based variable definition resolution.
    ///
    /// Returns:
    /// - `Some(Some(location))` — found a prior definition, jump there
    /// - `Some(None)` — cursor is at a definition site or no definition
    ///   found in the AST
    /// - `None` — AST parse failed
    fn resolve_variable_definition_ast(
        content: &str,
        uri: &str,
        position: Position,
        var_name: &str,
    ) -> Option<Option<Location>> {
        let cursor_offset = position_to_offset(content, position);

        let result = with_parsed_program(
            content,
            "resolve_variable_definition_ast",
            |program, content| {
                find_variable_definition_in_program(program, content, var_name, cursor_offset)
            },
        );

        match result {
            VarDefSearchResult::NotFound => {
                // The AST parse succeeded but found no definition — return
                // Some(None) so the caller knows not to fall back to text.
                Some(None)
            }
            VarDefSearchResult::AtDefinition => {
                // Cursor is at the definition — return Some(None).
                Some(None)
            }
            VarDefSearchResult::FoundAt { offset, end_offset } => {
                let target_uri = Url::parse(uri).ok()?;
                let start_pos = offset_to_position(content, offset as usize);
                let end_pos = offset_to_position(content, end_offset as usize);
                Some(Some(Location {
                    uri: target_uri,
                    range: Range {
                        start: start_pos,
                        end: end_pos,
                    },
                }))
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests;
