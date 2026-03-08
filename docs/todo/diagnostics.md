# PHPantom — Diagnostics

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label | Scale |
|---|---|
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low** |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## 2. Resolution-failure diagnostics
**Impact: Medium · Effort: Medium**

Unresolved class/interface, unresolved member access, and unresolvable
subject type diagnostics are already implemented. Two diagnostic types
remain:

### Diagnostics to emit

| Diagnostic | Trigger | Severity | Example |
|---|---|---|---|
| Unresolved function | A function call that `find_or_load_function` cannot resolve (global functions, namespaced functions, stubs) | Warning | `Function 'do_thing' not found` |
| Unresolved type in PHPDoc | A `@return`, `@param`, `@var`, `@throws`, `@mixin`, or `@extends` tag references a class that cannot be resolved | Information | `Type 'SomeAlias' in @return could not be resolved` |


---

## 3. Diagnostic suppression intelligence
**Impact: Medium · Effort: Medium**

When PHPantom proxies diagnostics from external tools (PHPStan, Psalm,
PHPMD, PHP_CodeSniffer), users need a way to suppress specific warnings.
Rather than forcing them to install a separate extension or memorise each
tool's suppression syntax, PHPantom can offer **code actions to insert
the correct suppression comment** for the tool that produced the
diagnostic.

### Behaviour

- When the cursor is on a diagnostic that originated from a proxied
  tool, offer a code action: `Suppress [TOOL] [RULE] for this line` /
  `…for this function` / `…for this file`.
- Insert the correct comment syntax for the originating tool:
  - PHPStan: `// @phpstan-ignore [identifier]` (line-level), or
    `@phpstan-ignore-next-line` above the line.
  - Psalm: `/** @psalm-suppress [IssueType] */` on the line or above
    the function/class.
  - PHPCS: `// phpcs:ignore [Sniff.Name]` or `// phpcs:disable` /
    `// phpcs:enable` blocks.
  - PHPMD: `// @SuppressWarnings(PHPMD.[RuleName])` in a docblock.
- For PHPantom's own diagnostics (§1, §2): support `@suppress PHPxxxx`
  in docblocks (matching PHP Tools' convention) and a config flag
  `phpantom.diagnostics.enabled: bool` (default `true`).

**Prerequisites:** Diagnostics infrastructure (§1 or §2 must ship
first so there are diagnostics to suppress). External tool integration
is a later phase — start with suppression for our own diagnostics.

**Why this matters:** This is the kind of feature that makes users
choose to configure PHPantom as their primary PHP language server
rather than running a separate linter extension alongside it. Generic
PHPMD/PHPStan extensions can show errors but can't offer contextual
suppression code actions because they don't understand PHP scope.

---

## 5. Warn when composer.json is missing or classmap is not optimized
**Impact: High · Effort: Medium**

PHPantom relies on Composer artifacts (`vendor/composer/autoload_classmap.php`,
`autoload_psr4.php`, `autoload_files.php`) for class discovery. When these
are missing or incomplete, completions silently degrade. The user should be
told what's wrong and offered help fixing it.

### Detection (during `initialized`)

| Condition | Severity | Message |
|---|---|---|
| No `composer.json` in workspace root | Warning | "No composer.json found. Class completions will be limited to open files and stubs." |
| `composer.json` exists but `vendor/` directory is missing | Warning | "No vendor directory found. Run `composer install` to enable full completions." |
| PSR-4 prefixes exist but no user classes in classmap | Info | "Composer classmap does not contain your project classes. Run `composer dump-autoload -o` for full class completions." |

For the no-composer.json case, offer to generate a minimal one via
`window/showMessageRequest`:

1. **"Generate composer.json"** — create a `composer.json` that maps
   the entire project root as a classmap (`"autoload": {"classmap": ["./"]}`).
   Then run `composer dump-autoload -o` to build the classmap. This
   covers legacy projects and single-directory setups that don't follow
   PSR-4 conventions.
2. **"Dismiss"** — do nothing.

The third condition needs care. The classmap is rarely empty because
vendor packages like PHPUnit use `classmap` autoloading (not PSR-4), so
there will be vendor entries even without `-o`. The real signal is:
the project's `composer.json` declares PSR-4 prefixes (e.g. `App\`,
`Tests\`), but none of the classmap FQNs start with any of those
project prefixes. This means the user's own classes were not dumped
into the classmap, which is exactly what `-o` fixes.

Detection logic:
1. Collect non-vendor PSR-4 prefixes from `psr4_mappings` (already
   tagged with `is_vendor`).
2. After loading the classmap, check whether any classmap FQN starts
   with one of those prefixes.
3. If there are project PSR-4 prefixes but zero matching classmap
   entries, the autoloader is not optimized.

### Actions (via `window/showMessageRequest`)

For the non-optimized classmap case, offer action buttons:

1. **"Run composer dump-autoload -o"** — spawn the command in the
   workspace root, reload the classmap on success, show a progress
   notification.
2. **"Add to composer.json & run"** — add
   `"config": {"optimize-autoloader": true}` to `composer.json` so
   future `composer install` / `composer update` always produce an
   optimized classmap, then run `composer dump-autoload`.
3. **"Dismiss"** — do nothing.

### UX guidelines

- The no-composer.json and no-vendor warnings are safe to show via
  `window/showMessage` (informational, no action taken).
- The classmap warning should use `window/showMessageRequest` with
  action buttons so the user explicitly opts in before we touch files
  or run commands.
- Only show once per session. Do not re-trigger on every `didOpen`.
- Never modify `composer.json` or run commands without explicit user
  confirmation via an action button.
- If the spawned `composer` command fails (e.g. PHP not installed
  locally, Docker-only setup), catch the error gracefully and show
  "Composer command failed. You may need to run it manually."
- Log the detection result to the output panel regardless (already done
  for the "Loaded N classmap entries" message, just add context when
  zero user classes are found).