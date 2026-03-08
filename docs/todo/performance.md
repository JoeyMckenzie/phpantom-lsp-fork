# PHPantom — Performance

Internal performance improvements that reduce latency, memory usage,
and lock contention on the hot paths. These items are sequenced so
that structural fixes land before features that would amplify the
underlying costs (parallel file processing, full background indexing).

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label | Scale |
|---|---|
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low** |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## 2. Reference-counted `ClassInfo` (`Arc<ClassInfo>`)
**Impact: High · Effort: Medium**

`ClassInfo` is a large struct: 30+ fields including `Vec<MethodInfo>`,
`Vec<PropertyInfo>`, `Vec<ConstantInfo>`, multiple `HashMap`s, and
many `Vec<String>` fields. It is deep-cloned constantly:

- `find_class_in_ast_map` returns `Some(cls.clone())`
- `find_or_load_class` clones the result from the ast_map
- `resolve_class_with_inheritance` starts with `class.clone()` and
  clones every parent method/property during merging
- `resolve_class_fully_inner` calls `resolve_class_with_inheritance`
  (more clones), then caches the result with `.clone()`
- `resolve_target_classes` returns `Vec<ClassInfo>` (each a full clone)
- Cache retrieval clones on read: `map.get(&key) → cached.clone()`

A single completion on `$this->` in a class with a deep inheritance
chain can produce dozens of full `ClassInfo` clones, each involving
deep copies of all method signatures, parameter lists, and docblock
strings.

Under full background indexing (indexing.md Phase 5), the ast_map
holds thousands of `ClassInfo` values. Cloning them out on every
lookup produces significant allocation pressure.

### Fix

Store `Arc<ClassInfo>` in `ast_map` instead of owned `ClassInfo`.
Retrieval becomes a cheap reference-count increment instead of a
deep copy. The `resolved_class_cache` should also store
`Arc<ClassInfo>` so that cache hits are free.

### Mutation

Inheritance merging (`resolve_class_with_inheritance`) mutates the
merged class. Use `Arc::make_mut` (copy-on-write) at the start of
merging: the first mutation clones the inner value, but subsequent
mutations on the same `Arc` are free. Code that only reads a
`ClassInfo` (the majority of call sites) never pays for a clone.

### Scope

This is a pervasive change that touches every function returning or
accepting `ClassInfo`. It can be done incrementally:

1. Change `ast_map` to store `Arc<ClassInfo>`. Update
   `find_class_in_ast_map` and `parse_and_cache_content_versioned`.
2. Change `resolved_class_cache` to store `Arc<ClassInfo>`. Update
   `resolve_class_fully_inner`.
3. Update `resolve_target_classes` and downstream consumers to accept
   `Arc<ClassInfo>` where possible.

Each step compiles and passes tests independently.

---

## 7. Recursive string substitution in `apply_substitution`
**Impact: Medium · Effort: High**

Generic type substitution (`apply_substitution`) does recursive
string parsing and re-building for every type string. It handles
nullable, union, intersection, generic, callable, and array types
by splitting, recursing, and re-joining strings. Each recursion
level allocates new `String` values.

This runs on every inherited method's return type, every parameter's
type hint, and every property's type hint when template substitution
is active. In a deeply-generic framework like Laravel (where
`Collection<TKey, TValue>` flows through multiple inheritance
levels), this function is called hundreds of times per resolution,
each time allocating new strings.

The resolved-class cache (type-inference.md §31) mitigates this by
caching the result, so substitution only runs on cache misses. But
cache misses still happen: first access, after edits that trigger
invalidation, and for generic classes with different type arguments.

### Fix (long-term)

Replace the string-based type representation with a parsed type AST
(an enum of `TypeNode` variants: `Named`, `Union`, `Intersection`,
`Generic`, `Nullable`, `Array`, `Callable`, etc.). Parse the type
string once during class extraction. Substitution becomes a tree
walk that swaps `Named` leaf nodes, avoiding all string allocation
and re-parsing.

This is a significant refactor that touches the parser, docblock
extraction, type resolution, and inheritance merging. It should be
evaluated after the lower-effort items are done and profiling
confirms that substitution remains a measurable cost.

### Fix (short-term)

Two targeted optimisations that reduce allocation without the full
refactor:

1. **Early exit.** Before recursing, check whether the type string
   contains any of the substitution map's keys. If no key appears
   as a substring, return the input unchanged (no allocation). This
   skips the majority of type strings that don't reference template
   parameters.

2. **Cow return type.** Change `apply_substitution` to return
   `Cow<'_, str>` instead of `String`. When no substitution occurs
   (the common case), return the borrowed input. Only allocate a new
   `String` when a replacement actually happens.

---

## 8. Incremental text sync
**Impact: Low-Medium · Effort: Medium**

PHPantom uses `TextDocumentSyncKind::FULL`, meaning every
`textDocument/didChange` notification sends the entire file content.
For large files (5000+ lines, common in legacy PHP), sending 200 KB
on every keystroke adds measurable IPC overhead.

The practical benefit is bounded: Mago requires a full re-parse
regardless of how the change was received. The saving is purely in
the data transferred over the IPC channel. For files under ~1000
lines this is negligible.

This item is already tracked in [lsp-features.md §17](lsp-features.md#17-incremental-text-sync)
and is included here for completeness. The effort and implementation
plan are unchanged. It is the lowest-priority performance item
because full-file sync is rarely the bottleneck in practice.