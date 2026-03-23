# PHPantom — Bug Fixes

Known bugs and incorrect behaviour. These are distinct from feature
requests — they represent cases where existing functionality produces
wrong results. Bugs should generally be fixed before new features at
the same impact tier.

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

#### B9. `assert($x instanceof self)` narrowing ignores `self`/`static`/`parent`

| | |
|---|---|
| **Impact** | Medium-High |
| **Effort** | Low |

`try_extract_instanceof` in `completion/types/narrowing.rs` only matches
`Expression::Identifier` for the RHS of an `instanceof` check. When the
RHS is `self`, `static`, or `parent`, the Mago AST produces
`Expression::Self_`, `Expression::Static`, or `Expression::Parent`
instead, which fall through to `None`. This means
`assert($feature instanceof self)` never narrows the variable.

The fix is to add three arms to the `match bin.rhs` block in
`try_extract_instanceof` that map `Self_` → `"self"`, `Static` →
`"static"`, `Parent` → `"parent"`. The downstream
`apply_instanceof_inclusion` already resolves `"self"` etc. to the
current class via `type_hint_to_classes`.

**Reproduce:** any method with a `BaseCatalogFeature` parameter that
calls `assert($feature instanceof self)` then accesses subclass-only
methods on `$feature`. Also affects Mockery test patterns:
`$mock = $this->mock(X::class); assert($mock instanceof X);`.

**Triage count:** ~30 diagnostics in luxplus/shared (18 BaseCatalogFeature,
11 MockInterface, 1 Elasticsearch).

---

#### B10. Negative narrowing after early return not applied

| | |
|---|---|
| **Impact** | Low-Medium |
| **Effort** | Medium |

After `if ($x instanceof Y) { return; }`, the variable `$x` should be
narrowed to exclude `Y` for all subsequent code in the same scope.
PHPantom does not apply this negative narrowing via early return.

**Reproduce:**

```php
public static function toString(mixed $value): string
{
    if ($value instanceof Stringable) {
        return $value->__toString();
    }
    if ($value instanceof BackedEnum) {
        $value = $value->value; // PHPantom resolves $value as Stringable here
    }
}
```

The diagnostic reports `Property 'value' not found on class 'Stringable'`
because `$value` is still resolved as `Stringable` inside the
`BackedEnum` branch, even though that branch is only reachable when
`$value` is NOT `Stringable`.

Guard-clause narrowing already works for `if (!$x instanceof Y) { return; }`
(positive narrowing after negated check). This is the inverse: positive
check with exit should produce negative narrowing for subsequent code.

**Triage count:** ~2 diagnostics directly, but a general correctness
issue that affects any code using the early-return-after-instanceof
pattern.

---

#### B11. Variables initialized to `null` and conditionally reassigned lose their type

| | |
|---|---|
| **Impact** | Medium |
| **Effort** | Medium |

When a variable is initialized as `$x = null` and then conditionally
reassigned inside a loop or branch (e.g. `$x = $transaction`), PHPantom
resolves `$x` to `null` at the usage site even after a truthiness guard
like `if (!$x) { continue; }` or `if ($x !== null)`.

Three common patterns:

1. **Null-init + loop assignment + guard:**
   `$x = null; foreach (...) { if (cond) { $x = $expr; } } if ($x) { $x->method(); }`

2. **Null coalesce + guard:**
   `$x = $arr[$key] ?? null; if (!$x) { continue; } $x->property;`

3. **assertNotNull:**
   `$day = $arr['key'] ?? null; self::assertNotNull($day); $day->from;`

The root cause is that PHPantom's variable resolution picks the first
(or dominant) assignment and does not consider all assignment sites to
build a union type. For pattern 1, only the `= null` is seen. For
patterns 2 and 3, the `?? null` makes the type nullable but the
subsequent guard or assertion is not recognized as narrowing.

Pattern 3 overlaps with custom assert narrowing (`@phpstan-assert`) which
requires recognizing `assertNotNull` as a type guard.

**Triage count:** ~8 scalar_member_access diagnostics in luxplus/shared
(AltapayGateway, PCNService, CustomerService, CoolrunnerClientTest).