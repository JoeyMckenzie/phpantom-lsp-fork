//! PHPStan conditional return type parsing.
//!
//! This submodule handles annotations like:
//! ```text
//! @return ($abstract is class-string<TClass> ? TClass
//!           : ($abstract is null ? \Illuminate\Foundation\Application : mixed))
//! ```
//!
//! The main entry point is [`extract_conditional_return_type`], which
//! returns a [`ConditionalReturnType`] tree that downstream code can
//! evaluate at call-sites by matching the actual argument types against
//! the declared conditions.

use mago_docblock::document::TagKind;

use crate::types::{ConditionalReturnType, ParamCondition};

use super::parser::{collapse_newlines, parse_docblock_for_tags, DocblockInfo};
use super::types::clean_type;

// ─── Public API ─────────────────────────────────────────────────────────────

/// Extract a PHPStan conditional return type from a `@return` tag.
///
/// Handles annotations like:
/// ```text
/// @return ($abstract is class-string<TClass> ? TClass
///           : ($abstract is null ? \Illuminate\Foundation\Application : mixed))
/// ```
///
/// Returns `None` if the `@return` tag is missing or is not a conditional
/// (i.e. does not start with `(`).
pub fn extract_conditional_return_type(docblock: &str) -> Option<ConditionalReturnType> {
    extract_conditional_return_type_from_info(&parse_docblock_for_tags(docblock)?)
}

/// Like [`extract_conditional_return_type`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_conditional_return_type_from_info(info: &DocblockInfo) -> Option<ConditionalReturnType> {
    let raw = extract_raw_return_content_from_info(info)?;
    let trimmed = raw.trim();
    if !trimmed.starts_with('(') {
        return None;
    }
    parse_conditional_expr(trimmed)
}

// ─── Internals ──────────────────────────────────────────────────────────────

/// Extract the raw content after `@return` from a pre-parsed docblock.
///
/// mago-docblock already handles `/** */` stripping, leading `*`
/// removal, and multi-line tag continuation.  The description is
/// returned with internal `\n` normalised to spaces so that downstream
/// parsers see a single-line string (matching the old behaviour of
/// joining continuation lines with spaces).
fn extract_raw_return_content_from_info(info: &DocblockInfo) -> Option<String> {
    let tag = info
        .first_tag_by_kind(TagKind::PhpstanReturn)
        .or_else(|| info.first_tag_by_kind(TagKind::Return))?;

    let desc = tag.description.trim();
    if desc.is_empty() {
        return None;
    }

    Some(collapse_newlines(desc))
}

/// Parse a conditional expression string into a [`ConditionalReturnType`].
///
/// Expected format (recursive):
/// ```text
/// ($paramName is ConditionType ? ThenType : ElseType)
/// ```
///
/// Where `ThenType` and `ElseType` can each be either a concrete type name
/// or another parenthesised conditional.
fn parse_conditional_expr(input: &str) -> Option<ConditionalReturnType> {
    let s = input.trim();

    // Must be wrapped in parens
    if !s.starts_with('(') || !s.ends_with(')') {
        // It's a concrete type
        let cleaned = clean_type(s);
        if cleaned.is_empty() {
            return None;
        }
        return Some(ConditionalReturnType::Concrete(cleaned));
    }

    // Strip outer parens
    let inner = &s[1..s.len() - 1];
    let inner = inner.trim();

    // Parse: $paramName is ConditionType ? ThenType : ElseType

    // 1. Extract $paramName
    let rest = inner.strip_prefix('$')?;
    let space_idx = rest.find(|c: char| c.is_whitespace())?;
    let param_name = rest[..space_idx].to_string();
    let rest = rest[space_idx..].trim_start();

    // 2. Expect "is"
    let rest = rest.strip_prefix("is")?.trim_start();

    // 3. Extract condition type (everything up to ` ? `)
    // We need to find ` ? ` but be careful about nested parens
    let question_pos = find_token_at_depth(rest, '?')?;
    let condition_str = rest[..question_pos].trim();
    let rest = rest[question_pos + 1..].trim_start();

    // 4. Parse condition
    let condition = parse_condition(condition_str);

    // 5. Parse then-type and else-type split by ` : `
    // We need to find `:` at depth 0
    let colon_pos = find_token_at_depth(rest, ':')?;
    let then_str = rest[..colon_pos].trim();
    let else_str = rest[colon_pos + 1..].trim();

    let then_type = parse_type_or_conditional(then_str)?;
    let else_type = parse_type_or_conditional(else_str)?;

    Some(ConditionalReturnType::Conditional {
        param_name,
        condition,
        then_type: Box::new(then_type),
        else_type: Box::new(else_type),
    })
}

/// Parse a string that is either a `(...)` conditional or a concrete type.
fn parse_type_or_conditional(s: &str) -> Option<ConditionalReturnType> {
    let s = s.trim();
    if s.starts_with('(') {
        parse_conditional_expr(s)
    } else {
        let cleaned = clean_type(s);
        if cleaned.is_empty() {
            return None;
        }
        Some(ConditionalReturnType::Concrete(cleaned))
    }
}

/// Find the position of `token` (e.g. `?` or `:`) at parenthesis depth 0.
///
/// Skips over `<…>` generics and `(…)` nested conditionals.
fn find_token_at_depth(s: &str, token: char) -> Option<usize> {
    let mut paren_depth = 0i32;
    let mut angle_depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '<' => angle_depth += 1,
            '>' => angle_depth -= 1,
            c if c == token && paren_depth == 0 && angle_depth == 0 => {
                return Some(i);
            }
            _ => {}
        }
    }
    None
}

/// Parse a condition string like `class-string<TClass>`, `null`, `"foo"`, or `\Closure`.
fn parse_condition(s: &str) -> ParamCondition {
    let s = s.trim();
    if s.starts_with("class-string") {
        ParamCondition::ClassString
    } else if s.eq_ignore_ascii_case("null") {
        ParamCondition::IsNull
    } else if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
    {
        // Literal string condition: `$param is "foo"` or `$param is 'foo'`
        let inner = &s[1..s.len() - 1];
        ParamCondition::LiteralString(inner.to_string())
    } else {
        let cleaned = s.strip_prefix('\\').unwrap_or(s);
        ParamCondition::IsType(cleaned.to_string())
    }
}