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

#### B15. Inline `@var` cast overrides variable type in same-line RHS

| | |
|---|---|
| **Impact** | Low |
| **Effort** | Low |

When an inline `/** @var T */` annotation precedes a reassignment
like `$data = $data->toArray()`, PHPantom resolves `$data` on the
RHS as type `T` instead of its previous type. The `@var` cast
should only apply to the variable after the assignment completes.

**Reproducer:**

```php
class Data {
    public function toArray(): array { return []; }
}
class Test {
    public function run(Data $data): array {
        /** @var array<string, mixed> */
        $data = $data->toArray();
        //      ^^^^^ resolved as array<string, mixed> instead of Data
        return $data;
    }
}
```

Produces: `Cannot access method 'toArray' on type 'array<string, mixed>'`

**Root cause:** This is a variant of B13 (variable type resolved from
reassignment target inside RHS). B13 was fixed for the case where the
assignment target name matches the variable in the RHS expression,
but the `@var` inline annotation applies the type override before the
RHS is evaluated, so the check does not catch it.

**Impact in shared codebase:** 2 false positives (Klarna.php and
GoogleTagManagerClient.php, both using `/** @var array<string, mixed> */
$data = $data->toArray()`).