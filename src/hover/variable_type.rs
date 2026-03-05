//! Variable type string resolution for hover.
//!
//! Unlike the completion pipeline (which resolves variables to
//! `Vec<ClassInfo>`), hover needs the **full type string** so that
//! generic parameters and scalar types are preserved.  For example,
//! a parameter typed as `\Generator<int, Pencil>` should display
//! exactly that, not just `Generator`.
//!
//! The entry point is [`resolve_variable_type_string`], which walks
//! the AST to find the variable's definition context and returns the
//! effective type string (docblock overriding native where applicable).

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::docblock;
use crate::parser::{extract_hint_string, with_parsed_program};
use crate::types::ClassInfo;

use crate::completion::resolver::FunctionLoaderFn;
use crate::completion::variable::raw_type_inference::resolve_variable_assignment_raw_type;

/// Resolve the type string for a variable at `cursor_offset` for hover.
///
/// Tries, in order:
/// 1. Inline `/** @var Type $var */` docblock override
/// 2. Parameter type (native + `@param` → effective)
/// 3. Foreach value/key binding (iterable element type from `@param`/`@var`)
/// 4. Catch variable (catch clause hint string)
/// 5. Assignment raw type via [`resolve_variable_assignment_raw_type`]
///
/// Returns `None` when no type information could be determined.
pub(crate) fn resolve_variable_type_string(
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    current_class: Option<&ClassInfo>,
    all_classes: &[ClassInfo],
    class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    function_loader: FunctionLoaderFn<'_>,
) -> Option<String> {
    // 1. Inline @var override: `/** @var Type $var */`
    if let Some(var_type) =
        docblock::find_var_raw_type_in_source(content, cursor_offset as usize, var_name)
    {
        return Some(var_type);
    }

    // 2–4. AST-based: parameter, foreach, catch
    let ast_result: Option<String> =
        with_parsed_program(content, "resolve_variable_type_string", |program, _| {
            find_variable_type_in_statements(
                program.statements.iter(),
                var_name,
                content,
                cursor_offset,
            )
        });
    if ast_result.is_some() {
        return ast_result;
    }

    // 5. Assignment raw type
    resolve_variable_assignment_raw_type(
        var_name,
        content,
        cursor_offset,
        current_class,
        all_classes,
        class_loader,
        function_loader,
    )
}

/// Walk top-level statements to find the scope containing the cursor
/// and extract the variable's type string from its definition site.
fn find_variable_type_in_statements<'a, I>(
    statements: I,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
) -> Option<String>
where
    I: Iterator<Item = &'a Statement<'a>>,
{
    let stmts: Vec<&Statement> = statements.collect();

    for &stmt in &stmts {
        match stmt {
            Statement::Class(class) => {
                let start = class.left_brace.start.offset;
                let end = class.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    return find_type_in_class_members(
                        class.members.iter(),
                        var_name,
                        content,
                        cursor_offset,
                    );
                }
            }
            Statement::Interface(iface) => {
                let start = iface.left_brace.start.offset;
                let end = iface.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    return find_type_in_class_members(
                        iface.members.iter(),
                        var_name,
                        content,
                        cursor_offset,
                    );
                }
            }
            Statement::Trait(trait_def) => {
                let start = trait_def.left_brace.start.offset;
                let end = trait_def.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    return find_type_in_class_members(
                        trait_def.members.iter(),
                        var_name,
                        content,
                        cursor_offset,
                    );
                }
            }
            Statement::Enum(enum_def) => {
                let start = enum_def.left_brace.start.offset;
                let end = enum_def.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    return find_type_in_class_members(
                        enum_def.members.iter(),
                        var_name,
                        content,
                        cursor_offset,
                    );
                }
            }
            Statement::Namespace(ns) => {
                if let Some(t) = find_variable_type_in_statements(
                    ns.statements().iter(),
                    var_name,
                    content,
                    cursor_offset,
                ) {
                    return Some(t);
                }
            }
            Statement::Function(func) => {
                let param_span = func.parameter_list.span();
                if cursor_offset >= param_span.start.offset
                    && cursor_offset <= param_span.end.offset
                {
                    return find_type_in_params(
                        &func.parameter_list,
                        var_name,
                        content,
                        cursor_offset,
                        func.span().start.offset as usize,
                    );
                }
                let body_start = func.body.left_brace.start.offset;
                let body_end = func.body.right_brace.end.offset;
                if cursor_offset >= body_start && cursor_offset <= body_end {
                    // Check body constructs (foreach, catch, closures)
                    if let Some(t) = find_type_in_body_stmts(
                        &func.body.statements.iter().collect::<Vec<_>>(),
                        var_name,
                        content,
                        cursor_offset,
                    ) {
                        return Some(t);
                    }
                    // Fall back to parameter type
                    return find_type_in_params(
                        &func.parameter_list,
                        var_name,
                        content,
                        cursor_offset,
                        func.span().start.offset as usize,
                    );
                }
            }
            Statement::Try(_) => {
                let stmt_span = stmt.span();
                if cursor_offset >= stmt_span.start.offset
                    && cursor_offset <= stmt_span.end.offset
                    && let Some(t) = find_type_in_catch(stmt, var_name, cursor_offset)
                {
                    return Some(t);
                }
            }
            Statement::Foreach(foreach) => {
                let stmt_span = stmt.span();
                if cursor_offset >= stmt_span.start.offset
                    && cursor_offset <= stmt_span.end.offset
                    && let Some(t) = find_type_in_foreach(foreach, var_name, content, cursor_offset)
                {
                    return Some(t);
                }
            }
            _ => {}
        }
    }
    None
}

/// Search class members for a method containing the cursor, then
/// extract the variable's type string.
fn find_type_in_class_members<'a, I>(
    members: I,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
) -> Option<String>
where
    I: Iterator<Item = &'a ClassLikeMember<'a>>,
{
    for member in members {
        if let ClassLikeMember::Method(method) = member {
            // Check parameter list span
            let param_span = method.parameter_list.span();
            if cursor_offset >= param_span.start.offset && cursor_offset <= param_span.end.offset {
                return find_type_in_params(
                    &method.parameter_list,
                    var_name,
                    content,
                    cursor_offset,
                    method.span().start.offset as usize,
                );
            }

            if let MethodBody::Concrete(body) = &method.body {
                let body_start = body.left_brace.start.offset;
                let body_end = body.right_brace.end.offset;
                if cursor_offset >= body_start && cursor_offset <= body_end {
                    // Check body constructs first
                    if let Some(t) = find_type_in_body_stmts(
                        &body.statements.iter().collect::<Vec<_>>(),
                        var_name,
                        content,
                        cursor_offset,
                    ) {
                        return Some(t);
                    }
                    // Fall back to parameter type
                    return find_type_in_params(
                        &method.parameter_list,
                        var_name,
                        content,
                        cursor_offset,
                        method.span().start.offset as usize,
                    );
                }
            }
        }
    }
    None
}

/// Extract the effective type string for a parameter matching `var_name`.
///
/// Uses the native type hint and `@param` docblock type, preferring the
/// docblock when it is more specific (via `resolve_effective_type`).
fn find_type_in_params(
    parameter_list: &FunctionLikeParameterList<'_>,
    var_name: &str,
    content: &str,
    _cursor_offset: u32,
    method_start_offset: usize,
) -> Option<String> {
    for param in parameter_list.parameters.iter() {
        let pname = param.variable.name.to_string();
        if pname != var_name {
            continue;
        }

        let native_type = param.hint.as_ref().map(|h| extract_hint_string(h));

        // Try @param docblock type
        let docblock_type =
            docblock::find_iterable_raw_type_in_source(content, method_start_offset, var_name)
                .or_else(|| {
                    // find_iterable_raw_type_in_source looks for @var/@param
                    // with the variable name. Try extract_param_raw_type on
                    // the raw docblock text as well.
                    find_method_docblock(content, method_start_offset)
                        .and_then(|doc| docblock::extract_param_raw_type(&doc, &pname))
                });

        let effective =
            docblock::resolve_effective_type(native_type.as_deref(), docblock_type.as_deref());

        if effective.is_some() {
            return effective;
        }

        // Return native type if no effective type
        return native_type;
    }
    None
}

/// Walk body statements looking for foreach/catch/closure type strings.
fn find_type_in_body_stmts(
    stmts: &[&Statement<'_>],
    var_name: &str,
    content: &str,
    cursor_offset: u32,
) -> Option<String> {
    for &stmt in stmts {
        let stmt_span = stmt.span();
        if cursor_offset < stmt_span.start.offset || cursor_offset > stmt_span.end.offset {
            continue;
        }

        // Catch variable type hints
        if let Some(t) = find_type_in_catch(stmt, var_name, cursor_offset) {
            return Some(t);
        }

        // Foreach bindings
        if let Statement::Foreach(foreach) = stmt
            && let Some(t) = find_type_in_foreach(foreach, var_name, content, cursor_offset)
        {
            return Some(t);
        }

        // Closure/arrow function parameters
        if let Some(t) = find_type_in_closure_stmt(stmt, var_name, content, cursor_offset) {
            return Some(t);
        }
    }
    None
}

/// Extract the catch clause type hint string for a matching variable.
fn find_type_in_catch(stmt: &Statement<'_>, var_name: &str, cursor_offset: u32) -> Option<String> {
    match stmt {
        Statement::Try(try_stmt) => {
            for catch in try_stmt.catch_clauses.iter() {
                if let Some(ref var) = catch.variable
                    && var.name == var_name
                {
                    let var_start = var.span.start.offset;
                    let var_end = var.span.end.offset;
                    if cursor_offset >= var_start && cursor_offset < var_end {
                        return Some(extract_hint_string(&catch.hint));
                    }
                }
                // Recurse into catch block
                let catch_span = catch.block.span();
                if cursor_offset >= catch_span.start.offset
                    && cursor_offset <= catch_span.end.offset
                {
                    for inner in catch.block.statements.iter() {
                        if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                            return Some(t);
                        }
                    }
                }
            }
            // Recurse into try block
            let try_span = try_stmt.block.span();
            if cursor_offset >= try_span.start.offset && cursor_offset <= try_span.end.offset {
                for inner in try_stmt.block.statements.iter() {
                    if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                        return Some(t);
                    }
                }
            }
            // Recurse into finally block
            if let Some(ref finally) = try_stmt.finally_clause {
                let finally_span = finally.block.span();
                if cursor_offset >= finally_span.start.offset
                    && cursor_offset <= finally_span.end.offset
                {
                    for inner in finally.block.statements.iter() {
                        if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                            return Some(t);
                        }
                    }
                }
            }
            None
        }
        Statement::If(if_stmt) => {
            for inner in if_stmt.body.statements() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::Foreach(foreach) => {
            for inner in foreach.body.statements() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::While(while_stmt) => {
            for inner in while_stmt.body.statements() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::DoWhile(do_while) => {
            find_type_in_catch(do_while.statement, var_name, cursor_offset)
        }
        Statement::For(for_stmt) => {
            for inner in for_stmt.body.statements() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::Block(block) => {
            for inner in block.statements.iter() {
                if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                    return Some(t);
                }
            }
            None
        }
        Statement::Switch(switch) => {
            for case in switch.body.cases() {
                for inner in case.statements().iter() {
                    if let Some(t) = find_type_in_catch(inner, var_name, cursor_offset) {
                        return Some(t);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the type of a foreach value/key binding from the iterable's
/// docblock annotation.
///
/// For `foreach ($pencils as $pencil)` where `$pencils` has
/// `@param \Generator<int, Pencil>`, this returns `Pencil`.
fn find_type_in_foreach(
    foreach: &Foreach<'_>,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
) -> Option<String> {
    // Check if the cursor is on the foreach value or key variable, or inside the body
    let foreach_start = foreach.foreach.span().start.offset;
    let body_end = foreach.body.span().end.offset;
    if cursor_offset < foreach_start || cursor_offset > body_end {
        return None;
    }

    // Determine if this foreach's value or key variable matches var_name
    let is_value_var = match &foreach.target {
        ForeachTarget::Value(val) => {
            if let Expression::Variable(Variable::Direct(dv)) = val.value {
                dv.name == var_name
            } else {
                false
            }
        }
        ForeachTarget::KeyValue(kv) => {
            if let Expression::Variable(Variable::Direct(dv)) = kv.value {
                dv.name == var_name
            } else {
                false
            }
        }
    };

    let is_key_var = match &foreach.target {
        ForeachTarget::Value(_) => false,
        ForeachTarget::KeyValue(kv) => {
            if let Expression::Variable(Variable::Direct(dv)) = kv.key {
                dv.name == var_name
            } else {
                false
            }
        }
    };

    if !is_value_var && !is_key_var {
        // Recurse into body for nested foreach/catch
        match &foreach.body {
            ForeachBody::Statement(inner) => {
                return find_type_in_body_stmts(&[inner], var_name, content, cursor_offset);
            }
            ForeachBody::ColonDelimited(body) => {
                let stmts: Vec<&Statement> = body.statements.iter().collect();
                return find_type_in_body_stmts(&stmts, var_name, content, cursor_offset);
            }
        }
    }

    // Get the iterable expression's raw type from docblock annotations
    let foreach_offset = foreach.foreach.span().start.offset as usize;
    let raw_type = extract_foreach_expression_raw_type(foreach, content, foreach_offset);

    if let Some(ref rt) = raw_type {
        if is_value_var {
            // Extract value type: `list<User>` → `User`,
            // `Generator<int, Pencil>` → `Pencil`,
            // `User[]` → `User`
            if let Some(element_type) = docblock::types::extract_generic_value_type(rt) {
                return Some(element_type);
            }
        } else if is_key_var && let Some(key_type) = docblock::types::extract_generic_key_type(rt) {
            return Some(key_type);
        }
    }

    None
}

/// Extract the raw iterable type string from the foreach expression's
/// surrounding docblock annotations.
fn extract_foreach_expression_raw_type(
    foreach: &Foreach<'_>,
    content: &str,
    foreach_offset: usize,
) -> Option<String> {
    let expr_span = foreach.expression.span();
    let expr_start = expr_span.start.offset as usize;
    let expr_end = expr_span.end.offset as usize;
    let expr_text = content.get(expr_start..expr_end)?.trim();

    if !expr_text.starts_with('$') || expr_text.contains("->") || expr_text.contains("::") {
        return None;
    }

    docblock::find_iterable_raw_type_in_source(content, foreach_offset, expr_text)
}

/// Recursively search a statement for closure/arrow function parameters.
fn find_type_in_closure_stmt(
    stmt: &Statement<'_>,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
) -> Option<String> {
    match stmt {
        Statement::Expression(expr_stmt) => {
            find_type_in_closure_expr(expr_stmt.expression, var_name, content, cursor_offset)
        }
        Statement::Return(ret) => ret
            .value
            .and_then(|expr| find_type_in_closure_expr(expr, var_name, content, cursor_offset)),
        Statement::If(if_stmt) => {
            for inner in if_stmt.body.statements() {
                if let Some(t) = find_type_in_closure_stmt(inner, var_name, content, cursor_offset)
                {
                    return Some(t);
                }
            }
            None
        }
        Statement::Foreach(foreach) => {
            for inner in foreach.body.statements() {
                if let Some(t) = find_type_in_closure_stmt(inner, var_name, content, cursor_offset)
                {
                    return Some(t);
                }
            }
            None
        }
        Statement::Block(block) => {
            for inner in block.statements.iter() {
                if let Some(t) = find_type_in_closure_stmt(inner, var_name, content, cursor_offset)
                {
                    return Some(t);
                }
            }
            None
        }
        Statement::Try(try_stmt) => {
            for inner in try_stmt.block.statements.iter() {
                if let Some(t) = find_type_in_closure_stmt(inner, var_name, content, cursor_offset)
                {
                    return Some(t);
                }
            }
            for catch in try_stmt.catch_clauses.iter() {
                for inner in catch.block.statements.iter() {
                    if let Some(t) =
                        find_type_in_closure_stmt(inner, var_name, content, cursor_offset)
                    {
                        return Some(t);
                    }
                }
            }
            if let Some(ref finally) = try_stmt.finally_clause {
                for inner in finally.block.statements.iter() {
                    if let Some(t) =
                        find_type_in_closure_stmt(inner, var_name, content, cursor_offset)
                    {
                        return Some(t);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Check if an expression contains a closure/arrow function whose
/// parameter matches the target variable.
fn find_type_in_closure_expr(
    expr: &Expression<'_>,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
) -> Option<String> {
    match expr {
        Expression::Closure(closure) => {
            let closure_span = closure.span();
            if cursor_offset >= closure_span.start.offset
                && cursor_offset <= closure_span.end.offset
            {
                // Check closure parameters
                let result = find_type_in_params(
                    &closure.parameter_list,
                    var_name,
                    content,
                    cursor_offset,
                    closure_span.start.offset as usize,
                );
                if result.is_some() {
                    return result;
                }
                // Recurse into body
                let stmts: Vec<&Statement> = closure.body.statements.iter().collect();
                return find_type_in_body_stmts(&stmts, var_name, content, cursor_offset);
            }
            None
        }
        Expression::ArrowFunction(arrow) => {
            let arrow_span = arrow.span();
            if cursor_offset >= arrow_span.start.offset && cursor_offset <= arrow_span.end.offset {
                return find_type_in_params(
                    &arrow.parameter_list,
                    var_name,
                    content,
                    cursor_offset,
                    arrow_span.start.offset as usize,
                );
            }
            None
        }
        Expression::Call(call) => {
            let arg_list = call.get_argument_list();
            for arg in arg_list.arguments.iter() {
                let arg_expr: &Expression<'_> = arg.value();
                if let Some(t) =
                    find_type_in_closure_expr(arg_expr, var_name, content, cursor_offset)
                {
                    return Some(t);
                }
            }
            None
        }
        Expression::Parenthesized(paren) => {
            find_type_in_closure_expr(paren.expression, var_name, content, cursor_offset)
        }
        Expression::Assignment(assign) => {
            find_type_in_closure_expr(assign.rhs, var_name, content, cursor_offset)
        }
        _ => None,
    }
}

/// Extract the raw docblock text for a method/function at the given
/// source offset by scanning backwards for `/** ... */`.
fn find_method_docblock(content: &str, method_start: usize) -> Option<String> {
    let before = content.get(..method_start)?;
    let trimmed = before.trim_end();
    if !trimmed.ends_with("*/") {
        return None;
    }
    let doc_end = trimmed.len();
    let doc_start = trimmed.rfind("/**")?;
    Some(trimmed[doc_start..doc_end].to_string())
}
