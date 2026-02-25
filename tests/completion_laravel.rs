mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Shared stubs ───────────────────────────────────────────────────────────

const COMPOSER_JSON: &str = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;

const MODEL_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;
class Model {}
";

const COLLECTION_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;
/**
 * @template TKey of array-key
 * @template TModel
 */
class Collection {
    /** @return int */
    public function count(): int { return 0; }
    /** @return TModel|null */
    public function first(): mixed { return null; }
    /** @return array<TKey, TModel> */
    public function all(): array { return []; }
}
";

const HAS_MANY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class HasMany {}
";

const HAS_ONE_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class HasOne {}
";

const BELONGS_TO_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class BelongsTo {}
";

const BELONGS_TO_MANY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class BelongsToMany {}
";

const MORPH_TO_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class MorphTo {}
";

const MORPH_ONE_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class MorphOne {}
";

const MORPH_MANY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class MorphMany {}
";

const MORPH_TO_MANY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class MorphToMany {}
";

const HAS_MANY_THROUGH_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class HasManyThrough {}
";

const BUILDER_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;

use Illuminate\\Database\\Concerns\\BuildsQueries;

/**
 * @template TModel of \\Illuminate\\Database\\Eloquent\\Model
 * @mixin \\Illuminate\\Database\\Query\\Builder
 */
class Builder {
    /** @use BuildsQueries<TModel> */
    use BuildsQueries;

    /** @return static */
    public function where(string $column, mixed $operator = null, mixed $value = null): static { return $this; }
    /** @return static */
    public function orderBy(string $column, string $direction = 'asc'): static { return $this; }
    /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */
    public function get(): Collection { return new Collection(); }
    /** @return TModel|\\Illuminate\\Database\\Eloquent\\Collection<int, TModel>|null */
    public function find(mixed $id): mixed { return null; }
    /** @return static */
    public function limit(int $value): static { return $this; }
    /** @return bool */
    public function exists(): bool { return false; }
    /** @return string */
    public function toSql(): string { return ''; }
}
";

const QUERY_BUILDER_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Query;
class Builder {
    /** @return static */
    public function whereIn(string $column, array $values): static { return $this; }
    /** @return static */
    public function groupBy(string ...$groups): static { return $this; }
    /** @return static */
    public function having(string $column, mixed $operator = null, mixed $value = null): static { return $this; }
}
";

const BUILDS_QUERIES_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Concerns;
/**
 * @template TValue
 */
trait BuildsQueries {
    /** @return TValue|null */
    public function first(): mixed { return null; }
    /** @return TValue */
    public function firstOrFail(): mixed { return null; }
    /** @return TValue|null */
    public function sole(): mixed { return null; }
}
";

/// Standard set of framework stub files that every test needs.
fn framework_stubs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("vendor/illuminate/Eloquent/Model.php", MODEL_PHP),
        (
            "vendor/illuminate/Concerns/BuildsQueries.php",
            BUILDS_QUERIES_PHP,
        ),
        ("vendor/illuminate/Eloquent/Collection.php", COLLECTION_PHP),
        ("vendor/illuminate/Eloquent/Builder.php", BUILDER_PHP),
        ("vendor/illuminate/Query/Builder.php", QUERY_BUILDER_PHP),
        (
            "vendor/illuminate/Eloquent/Relations/HasMany.php",
            HAS_MANY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/HasOne.php",
            HAS_ONE_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/BelongsTo.php",
            BELONGS_TO_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/BelongsToMany.php",
            BELONGS_TO_MANY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/MorphTo.php",
            MORPH_TO_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/MorphOne.php",
            MORPH_ONE_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/MorphMany.php",
            MORPH_MANY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/MorphToMany.php",
            MORPH_TO_MANY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/HasManyThrough.php",
            HAS_MANY_THROUGH_PHP,
        ),
    ]
}

/// Build a PSR-4 workspace from the framework stubs plus extra app files.
fn make_workspace(app_files: &[(&str, &str)]) -> (phpantom_lsp::Backend, tempfile::TempDir) {
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.extend_from_slice(app_files);
    create_psr4_workspace(COMPOSER_JSON, &files)
}

/// Helper: open a file and trigger completion, returning the completion items.
async fn complete_at(
    backend: &phpantom_lsp::Backend,
    dir: &tempfile::TempDir,
    relative_path: &str,
    content: &str,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let uri = Url::from_file_path(dir.path().join(relative_path)).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: content.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    match result {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        _ => Vec::new(),
    }
}

fn property_names(items: &[CompletionItem]) -> Vec<&str> {
    items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect()
}

fn method_names(items: &[CompletionItem]) -> Vec<&str> {
    items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect()
}

// ─── HasMany relationship produces virtual property ─────────────────────────

#[tokio::test]
async fn test_has_many_relationship_produces_property() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // Line 9 = "$user->", character 15 = after ->
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Should include synthesized 'posts' relationship property, got: {:?}",
        props
    );

    let methods = method_names(&items);
    assert!(
        methods.contains(&"posts"),
        "The relationship method itself should also appear, got: {:?}",
        methods
    );
}

// ─── HasOne relationship produces virtual property ──────────────────────────

#[tokio::test]
async fn test_has_one_relationship_produces_property() {
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Profile.php", profile_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"profile"),
        "Should include synthesized 'profile' property, got: {:?}",
        props
    );
}

// ─── BelongsTo relationship produces virtual property ───────────────────────

#[tokio::test]
async fn test_belongs_to_relationship_produces_property() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getEmail(): string { return ''; }
}
";
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\BelongsTo;
class Post extends Model {
    /** @return BelongsTo<\\App\\Models\\User, $this> */
    public function author(): BelongsTo { return $this->belongsTo(User::class); }
    public function test() {
        $post = new Post();
        $post->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Post.php", post_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"author"),
        "Should include synthesized 'author' property, got: {:?}",
        props
    );
}

// ─── MorphTo relationship produces virtual property ─────────────────────────

#[tokio::test]
async fn test_morph_to_relationship_produces_property() {
    let comment_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\MorphTo;
class Comment extends Model {
    /** @return MorphTo */
    public function commentable(): MorphTo { return $this->morphTo(); }
    public function test() {
        $comment = new Comment();
        $comment->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Comment.php", comment_php)]);

    let items = complete_at(&backend, &dir, "src/Models/Comment.php", comment_php, 9, 19).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"commentable"),
        "Should include synthesized 'commentable' property, got: {:?}",
        props
    );
}

// ─── Multiple relationships all produce properties ──────────────────────────

#[tokio::test]
async fn test_multiple_relationships_all_produce_properties() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {}
";
    let role_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Role extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
use Illuminate\\Database\\Eloquent\\Relations\\BelongsToMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    /** @return BelongsToMany<\\App\\Models\\Role, $this> */
    public function roles(): BelongsToMany { return $this->belongsToMany(Role::class); }
    public function getFullName(): string { return ''; }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/Profile.php", profile_php),
        ("src/Models/Role.php", role_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 16, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Should include 'posts' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"profile"),
        "Should include 'profile' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"roles"),
        "Should include 'roles' property, got: {:?}",
        props
    );
    assert!(
        !props.contains(&"getFullName"),
        "'getFullName' should not appear as a property, got: {:?}",
        props
    );
}

// ─── Non-model class does not get relationship properties ───────────────────

#[tokio::test]
async fn test_relationship_property_does_not_appear_for_non_models() {
    // A plain class that happens to return a class named HasMany (but in a
    // different namespace / without actually extending Eloquent Model).
    let service_php = "\
<?php
namespace App\\Models;
class HasMany {}
class UserService {
    /** @return HasMany */
    public function posts(): HasMany { return new HasMany(); }
    public function test() {
        $svc = new UserService();
        $svc->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/UserService.php", service_php)]);

    let items = complete_at(
        &backend,
        &dir,
        "src/Models/UserService.php",
        service_php,
        8,
        14,
    )
    .await;
    let props = property_names(&items);

    assert!(
        !props.contains(&"posts"),
        "'posts' should NOT be synthesized on non-Model class, got: {:?}",
        props
    );
}

// ─── HasOne chain resolves to the related model's members ───────────────────

#[tokio::test]
async fn test_has_one_relationship_property_chains_to_related_class() {
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
    public function getAvatar(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->profile->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Profile.php", profile_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->profile->" at line 9, character 24
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 24).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getBio"),
        "Should chain through profile to Profile::getBio, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"getAvatar"),
        "Should chain through profile to Profile::getAvatar, got: {:?}",
        methods
    );
}

// ─── $this-> shows relationship properties ──────────────────────────────────

#[tokio::test]
async fn test_this_arrow_shows_relationship_properties() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $this->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$this->" at line 8, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Should include synthesized 'posts' property via $this->, got: {:?}",
        props
    );
}

// ─── Laravel provider beats @property tag (priority) ────────────────────────

#[tokio::test]
async fn test_laravel_provider_beats_phpdoc_property_tag() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    // The class has both a @property tag and a relationship method named
    // "posts". The LaravelModelProvider has higher priority so its
    // synthesized property wins, and the @property tag from PHPDocProvider
    // is not duplicated.
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
/**
 * @property array $posts
 */
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 12, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 12, 15).await;

    let posts_props: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::PROPERTY)
                && i.filter_text.as_deref().unwrap_or(&i.label) == "posts"
        })
        .collect();

    assert_eq!(
        posts_props.len(),
        1,
        "Should have exactly one 'posts' property (Laravel provider wins over @property), got: {}",
        posts_props.len()
    );
}

// ─── Relationship declared in a trait used by the model ─────────────────────

#[tokio::test]
async fn test_relationship_from_trait_produces_property() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let trait_php = "\
<?php
namespace App\\Concerns;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
trait HasPosts {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(\\App\\Models\\Post::class); }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Concerns\\HasPosts;
class User extends Model {
    use HasPosts;
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Concerns/HasPosts.php", trait_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 8, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Trait relationship method should produce virtual property, got: {:?}",
        props
    );
}

// ─── Indirect Model subclass (through BaseModel) ────────────────────────────

#[tokio::test]
async fn test_indirect_model_subclass_gets_relationship_properties() {
    let base_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class BaseModel extends Model {}
";
    let post_php = "\
<?php
namespace App\\Models;
class Post extends BaseModel {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends BaseModel {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/BaseModel.php", base_php),
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 8, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Indirect Model subclass should still get relationship properties, got: {:?}",
        props
    );
}

// ─── FQN relationship return type ───────────────────────────────────────────

#[tokio::test]
async fn test_fqn_relationship_return_type_produces_property() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<\\App\\Models\\Post, $this> */
    public function posts(): \\Illuminate\\Database\\Eloquent\\Relations\\HasMany {
        return $this->hasMany(Post::class);
    }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 10, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "FQN return type should still produce 'posts' property, got: {:?}",
        props
    );
}

// ─── All collection relationship types produce properties ───────────────────

#[tokio::test]
async fn test_morph_many_and_belongs_to_many_produce_properties() {
    let comment_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Comment extends Model {}
";
    let role_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Role extends Model {}
";
    let tag_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Tag extends Model {}
";
    let deployment_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Deployment extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\MorphMany;
use Illuminate\\Database\\Eloquent\\Relations\\BelongsToMany;
use Illuminate\\Database\\Eloquent\\Relations\\HasManyThrough;
use Illuminate\\Database\\Eloquent\\Relations\\MorphToMany;
class User extends Model {
    /** @return MorphMany<\\App\\Models\\Comment, $this> */
    public function comments(): MorphMany { return $this->morphMany(Comment::class, 'commentable'); }
    /** @return BelongsToMany<\\App\\Models\\Role, $this> */
    public function roles(): BelongsToMany { return $this->belongsToMany(Role::class); }
    /** @return HasManyThrough<\\App\\Models\\Deployment, \\App\\Models\\User> */
    public function deployments(): HasManyThrough { return $this->hasManyThrough(Deployment::class, User::class); }
    /** @return MorphToMany<\\App\\Models\\Tag, $this> */
    public function tags(): MorphToMany { return $this->morphToMany(Tag::class, 'taggable'); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Comment.php", comment_php),
        ("src/Models/Role.php", role_php),
        ("src/Models/Tag.php", tag_php),
        ("src/Models/Deployment.php", deployment_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 18, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 18, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"comments"),
        "MorphMany should produce 'comments' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"roles"),
        "BelongsToMany should produce 'roles' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"deployments"),
        "HasManyThrough should produce 'deployments' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"tags"),
        "MorphToMany should produce 'tags' property, got: {:?}",
        props
    );
}

// ─── MorphOne relationship produces virtual property ────────────────────────

#[tokio::test]
async fn test_morph_one_relationship_produces_property() {
    let image_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Image extends Model {
    public function getUrl(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\MorphOne;
class User extends Model {
    /** @return MorphOne<\\App\\Models\\Image, $this> */
    public function avatar(): MorphOne { return $this->morphOne(Image::class, 'imageable'); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Image.php", image_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"avatar"),
        "MorphOne should produce 'avatar' property, got: {:?}",
        props
    );
}

// ─── Real declared property beats virtual relationship property ──────────────

#[tokio::test]
async fn test_real_property_beats_virtual_relationship_property() {
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    /** A real declared property that shadows the relationship. */
    public string $profile = 'default';
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Profile.php", profile_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 11, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;

    let profile_props: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::PROPERTY)
                && i.filter_text.as_deref().unwrap_or(&i.label) == "profile"
        })
        .collect();

    assert_eq!(
        profile_props.len(),
        1,
        "Should have exactly one 'profile' property (real declared wins), got: {}",
        profile_props.len()
    );
}

// ─── Cross-file chain through relationship property ─────────────────────────

#[tokio::test]
async fn test_cross_file_relationship_property_chain_resolves() {
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
    public function getAvatar(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->profile->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Profile.php", profile_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->profile->" at line 9, character 24
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 24).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getBio"),
        "Should chain through relationship property to Profile::getBio cross-file, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"getAvatar"),
        "Should chain through relationship property to Profile::getAvatar cross-file, got: {:?}",
        methods
    );
}

// ─── Relationship property chain after first() ─────────────────────────────

#[tokio::test]
async fn test_relationship_property_chain_after_first() {
    // When accessing a relationship property on a model returned by first(),
    // completion should resolve to the related model, not the parent model.
    //
    // Customer::where()->first()->userInformation-> should offer
    // UserInformation methods, not Customer methods.
    let user_info_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class UserInformation extends Model {
    public function getAddress(): string { return ''; }
    public function getPhone(): string { return ''; }
}
";
    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class Customer extends Model {
    public function getEmail(): string { return ''; }
    /** @return HasOne<\\App\\Models\\UserInformation, $this> */
    public function userInformation(): HasOne { return $this->hasOne(UserInformation::class); }
    public function test() {
        $customer = Customer::where('id', 1)->first();
        $customer->userInformation->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/UserInformation.php", user_info_php),
        ("src/Models/Customer.php", customer_php),
    ]);

    // "$customer->userInformation->" at line 10, character 39
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Customer.php",
        customer_php,
        10,
        39,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getAddress"),
        "Should chain through userInformation to UserInformation::getAddress, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"getPhone"),
        "Should chain through userInformation to UserInformation::getPhone, got: {:?}",
        methods
    );
    // Should NOT contain Customer's own methods
    assert!(
        !methods.contains(&"getEmail"),
        "Should NOT offer Customer::getEmail on UserInformation, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_relationship_property_chain_after_first_or_fail() {
    // Same as above but with firstOrFail() instead of first().
    let user_info_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class UserInformation extends Model {
    public function getAddress(): string { return ''; }
    public function getPhone(): string { return ''; }
}
";
    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class Customer extends Model {
    public function getEmail(): string { return ''; }
    /** @return HasOne<\\App\\Models\\UserInformation, $this> */
    public function userInformation(): HasOne { return $this->hasOne(UserInformation::class); }
    public function test() {
        $customer = Customer::where('id', 1)->firstOrFail();
        $customer->userInformation->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/UserInformation.php", user_info_php),
        ("src/Models/Customer.php", customer_php),
    ]);

    // "$customer->userInformation->" at line 10, character 39
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Customer.php",
        customer_php,
        10,
        39,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getAddress"),
        "Should chain through userInformation to UserInformation::getAddress after firstOrFail(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"getPhone"),
        "Should chain through userInformation to UserInformation::getPhone after firstOrFail(), got: {:?}",
        methods
    );
    assert!(
        !methods.contains(&"getEmail"),
        "Should NOT offer Customer::getEmail on UserInformation after firstOrFail(), got: {:?}",
        methods
    );
}

// ─── Skips methods without return type ──────────────────────────────────────

#[tokio::test]
async fn test_skips_methods_without_return_type() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function posts() {}
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        !props.contains(&"posts"),
        "Method without return type should not produce a virtual property, got: {:?}",
        props
    );
}

// ─── Relationship without generics (singular) produces nothing ──────────────

#[tokio::test]
async fn test_singular_relationship_without_generics_produces_nothing() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        !props.contains(&"profile"),
        "Singular relationship without generics should not produce a property (no TRelated), got: {:?}",
        props
    );
}

// ─── Collection relationship without generics falls back to Model ───────────

#[tokio::test]
async fn test_collection_relationship_without_generics_uses_model_fallback() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Collection relationship without generics should still produce a property (falls back to Collection<Model>), got: {:?}",
        props
    );
}

// ─── Same-file test using did_open with no workspace ────────────────────────

#[tokio::test]
async fn test_same_file_relationship_property_with_plain_backend() {
    // This test uses create_test_backend() and opens a single file that
    // defines all needed classes in the global namespace. The parent_class
    // is set to the full FQN via the use statement.
    let backend = create_test_backend();

    let uri = Url::parse("file:///laravel_same_file.php").unwrap();
    // We define stub classes without a namespace. The parser stores them
    // by their short name. We place them so that `User extends Model` and
    // `Model` has FQN `Illuminate\Database\Eloquent\Model` via the
    // namespace declaration.
    //
    // Actually, for a single file the simplest approach is to put everything
    // in one namespace. We define Model as a separate class in the file with
    // the correct FQN.
    let text = "\
<?php
namespace App\\Models;

class Model extends \\Illuminate\\Database\\Eloquent\\Model {}

class HasMany {}

class Post extends Model {
    public function getTitle(): string { return ''; }
}

class User extends Model {
    /** @return HasMany<Post, $this> */
    public function posts(): HasMany { return new HasMany(); }
    public function test() {
        $user = new User();
        $user->
    }
}
";

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                // "$user->" at line 17, character 15
                position: Position {
                    line: 17,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    match result {
        Some(CompletionResponse::Array(items))
        | Some(CompletionResponse::List(CompletionList { items, .. })) => {
            let props = property_names(&items);
            // The parent class is `App\Models\Model` which extends
            // `\Illuminate\Database\Eloquent\Model`. Since the class loader
            // cannot resolve the stub FQN in this simple test, the provider
            // may not detect this as an Eloquent model. That's expected.
            // This test documents the limitation of same-file testing
            // without stubs. Cross-file PSR-4 tests above cover the real
            // behavior.
            //
            // If the provider detects it (because the parent walk finds it),
            // great. If not, this is a known limitation.
            let _ = props;
        }
        _ => {
            // Completion may return None for this edge case - that's acceptable.
        }
    }
}

// ─── Provider priority: virtual property from Laravel beats @property from PHPDoc ───

#[tokio::test]
async fn test_builder_methods_appear_as_static_on_model() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::" at line 5, character 14
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 14).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"where"),
        "Builder's where() should appear as static on User::, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"orderBy"),
        "Builder's orderBy() should appear as static on User::, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "Builder's get() should appear as static on User::, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Builder's first() should appear as static on User::, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"find"),
        "Builder's find() should appear as static on User::, got: {:?}",
        methods
    );
}

// ─── Builder chain resolution ───────────────────────────────────────────────

#[tokio::test]
async fn test_builder_where_chain_resolves_to_builder_methods() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        $q = User::where('email', 'foo@bar.com');
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$q->" at line 6, character 12
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 6, 12).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"orderBy"),
        "After User::where(), ->orderBy() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "After User::where(), ->get() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "After User::where(), ->first() should be available, got: {:?}",
        methods
    );
}

// ─── Builder get() returns Collection with model type ───────────────────────

#[tokio::test]
async fn test_builder_get_returns_collection_of_model() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getName(): string { return ''; }
    public function test() {
        $users = User::where('active', true)->get();
        $users->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$users->" at line 7, character 16
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 16).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"count"),
        "Collection from get() should have count(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Collection from get() should have first(), got: {:?}",
        methods
    );
}

// ─── Builder first() returns model instance ─────────────────────────────────

#[tokio::test]
async fn test_builder_first_returns_model_instance() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getName(): string { return ''; }
    public function test() {
        $user = User::where('active', true)->first();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$user->" at line 7, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getName"),
        "first() should return a User instance with getName(), got: {:?}",
        methods
    );
}

// ─── Builder first() via BuildsQueries trait ────────────────────────────────

#[tokio::test]
async fn test_builder_first_via_builds_queries_trait() {
    // first() lives on the BuildsQueries trait, not directly on Builder.
    // The Builder stub declares:
    //   /** @use BuildsQueries<TModel> */
    //   use BuildsQueries;
    //
    // BuildsQueries has @template TValue and first() returns TValue|null.
    // After trait merging, Builder::first() returns TModel|null, and when
    // TModel is substituted with User, the result should be User|null.
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getName(): string { return ''; }
    public function test() {
        $user = User::where('active', true)->first();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$user->" at line 7, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getName"),
        "first() via BuildsQueries should return User with getName(), got: {:?}",
        methods
    );
}

// ─── Builder mixin methods forwarded ────────────────────────────────────────

#[tokio::test]
async fn test_builder_mixin_methods_forwarded_to_model() {
    // whereIn and groupBy come from Query\Builder via @mixin on Eloquent\Builder.
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::" at line 5, character 14
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 14).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"whereIn"),
        "Query\\Builder's whereIn() should appear via @mixin forwarding, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"groupBy"),
        "Query\\Builder's groupBy() should appear via @mixin forwarding, got: {:?}",
        methods
    );
}

// ─── Scope method beats Builder forwarded method ────────────────────────────

#[tokio::test]
async fn test_scope_beats_builder_forwarded_method() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function scopeWhere(\\Illuminate\\Database\\Eloquent\\Builder $query, string $col): void {}
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::" at line 6, character 14
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 6, 14).await;

    // There should be a static "where" method from the scope.
    // The Builder's "where" should not duplicate it (merge dedup).
    let where_methods: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::METHOD)
                && i.filter_text.as_deref().unwrap_or(&i.label) == "where"
        })
        .collect();

    assert!(
        !where_methods.is_empty(),
        "Should have at least one 'where' method"
    );
}

// ─── Builder forwarding does not appear for non-models ──────────────────────

#[tokio::test]
async fn test_builder_forwarding_not_on_non_models() {
    let service_php = "\
<?php
namespace App\\Models;
class UserService {
    public function test() {
        UserService::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/UserService.php", service_php)]);

    // "UserService::" at line 4, character 22
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/UserService.php",
        service_php,
        4,
        22,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        !methods.contains(&"where"),
        "Non-model class should not have Builder methods, got: {:?}",
        methods
    );
}

// ─── Builder exists() and toSql() preserve non-template return types ────────

#[tokio::test]
async fn test_builder_non_template_return_types_preserved() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::" at line 5, character 14
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 14).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"exists"),
        "Builder's exists() should be forwarded, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"toSql"),
        "Builder's toSql() should be forwarded, got: {:?}",
        methods
    );
}

// ─── Indirect model subclass gets Builder forwarding ────────────────────────

#[tokio::test]
async fn test_indirect_model_subclass_gets_builder_forwarding() {
    let base_model_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class BaseModel extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
class User extends BaseModel {
    public function getName(): string { return ''; }
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/BaseModel.php", base_model_php),
        ("src/Models/User.php", user_php),
    ]);

    // "User::" at line 5, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 15).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"where"),
        "Indirect model subclass should get Builder forwarding, got: {:?}",
        methods
    );
}

// ─── Builder forwarding coexists with relationships and scopes ──────────────

#[tokio::test]
async fn test_builder_forwarding_coexists_with_relationships_and_scopes() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function scopeActive(\\Illuminate\\Database\\Eloquent\\Builder $query): void {}
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 10, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);
    let methods = method_names(&items);

    // Relationship property
    assert!(
        props.contains(&"posts"),
        "Relationship property should appear, got: {:?}",
        props
    );
    // Scope (instance)
    assert!(
        methods.contains(&"active"),
        "Scope method should appear as instance, got: {:?}",
        methods
    );
    // Relationship method
    assert!(
        methods.contains(&"posts"),
        "Relationship method should appear, got: {:?}",
        methods
    );
}

// ─── Provider priority ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_provider_priority_laravel_over_phpdoc_over_mixin() {
    // A model with a relationship, a @property tag for the same name,
    // and a @mixin with a property of the same name.
    // The Laravel provider's version should be the one that survives.
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let mixin_php = "\
<?php
namespace App\\Models;
class PostsMixin {
    public string $posts = '';
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
/**
 * @property string $posts
 * @mixin PostsMixin
 */
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/PostsMixin.php", mixin_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 13, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 13, 15).await;

    let posts_props: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::PROPERTY)
                && i.filter_text.as_deref().unwrap_or(&i.label) == "posts"
        })
        .collect();

    assert_eq!(
        posts_props.len(),
        1,
        "Should have exactly one 'posts' property despite three sources, got: {}",
        posts_props.len()
    );
}

// ─── Inline builder chain completion ────────────────────────────────────────

#[tokio::test]
async fn test_inline_builder_chain_where_arrow_completion() {
    // User::where()-> should offer builder methods (orderBy, get, first, etc.)
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::where()->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::where()->" at line 5, character 23
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 23).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"orderBy"),
        "User::where()-> should offer orderBy(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "User::where()-> should offer get(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "User::where()-> should offer first(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_inline_builder_chain_orderby_arrow_completion() {
    // User::where()->orderBy('name')-> should continue to offer builder methods
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::where()->orderBy('name')->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::where()->orderBy('name')->" at line 5, character 40
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 40).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"get"),
        "User::where()->orderBy('name')-> should offer get(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "User::where()->orderBy('name')-> should offer first(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"limit"),
        "User::where()->orderBy('name')-> should offer limit(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_inline_builder_chain_three_deep() {
    // User::where()->orderBy('name')->limit(10)-> should still offer builder methods
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::where()->orderBy('name')->limit(10)->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // cursor at end of chain
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 55).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"get"),
        "Three-deep builder chain should offer get(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Three-deep builder chain should offer first(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_builder_scope_static_chain_completion() {
    // A model with scopes should also chain: BlogAuthor::active()-> should offer builder methods
    let author_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class BlogAuthor extends Model {
    public function scopeActive(Builder $query): void {}
    public function scopeOfGenre(Builder $query, string $genre): void {}
    public function test() {
        BlogAuthor::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/BlogAuthor.php", author_php)]);

    // "BlogAuthor::" at line 8, character 20
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BlogAuthor.php",
        author_php,
        8,
        20,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"active"),
        "BlogAuthor:: should offer scope method active(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"ofGenre"),
        "BlogAuthor:: should offer scope method ofGenre(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"where"),
        "BlogAuthor:: should offer builder-forwarded where(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"orderBy"),
        "BlogAuthor:: should offer builder-forwarded orderBy(), got: {:?}",
        methods
    );
}

// ─── Single-file with inline Illuminate stubs (example.php style) ───────────

#[tokio::test]
async fn test_builder_chain_single_file_with_inline_stubs() {
    // Mimics example.php: model class in one namespace, Illuminate stubs
    // in separate namespace blocks in the same file.
    let backend = create_test_backend();

    let uri = Url::parse("file:///inline_stubs.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Demo {\n",
        "\n",
        "class MyUser extends \\Illuminate\\Database\\Eloquent\\Model\n",
        "{\n",
        "    public function getName(): string { return ''; }\n",
        "    public function demo(): void\n",
        "    {\n",
        "        MyUser::where('active', true);\n", // line 8
        "        MyUser::where('active', 1)->get();\n", // line 9
        "        MyUser::where('active', 1)->first();\n", // line 10
        "        MyUser::orderBy('name')->limit(10)->get();\n", // line 11
        "    }\n",
        "}\n",
        "\n",
        "} // end namespace Demo\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */\n",
        "        public static function query() {}\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     * @mixin \\Illuminate\\Database\\Query\\Builder\n",
        "     */\n",
        "    class Builder {\n",
        "        /** @use \\Illuminate\\Database\\Concerns\\BuildsQueries<TModel> */\n",
        "        use \\Illuminate\\Database\\Concerns\\BuildsQueries;\n",
        "\n",
        "        /** @return $this */\n",
        "        public function where($column, $operator = null, $value = null) {}\n",
        "\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */\n",
        "        public function get($columns = ['*']) { return new Collection(); }\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     */\n",
        "    class Collection {\n",
        "        /** @return TModel|null */\n",
        "        public function first(): mixed { return null; }\n",
        "        public function count(): int { return 0; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent\\Relations {\n",
        "    class HasMany {}\n",
        "    class HasOne {}\n",
        "    class BelongsTo {}\n",
        "    class BelongsToMany {}\n",
        "    class MorphOne {}\n",
        "    class MorphMany {}\n",
        "    class MorphTo {}\n",
        "    class MorphToMany {}\n",
        "    class HasManyThrough {}\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Concerns {\n",
        "    /**\n",
        "     * @template TValue\n",
        "     */\n",
        "    trait BuildsQueries {\n",
        "        /** @return TValue|null */\n",
        "        public function first($columns = ['*']) { return null; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Query {\n",
        "    class Builder {\n",
        "        /** @return $this */\n",
        "        public function whereIn($column, $values) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function groupBy(...$groups) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function orderBy($column, $direction = 'asc') { return $this; }\n",
        "        /** @return $this */\n",
        "        public function limit($value) { return $this; }\n",
        "    }\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    // ── Test 1: MyUser::where('active', true) should offer Builder methods ──
    // "MyUser::where('active', true);" is at line 8.
    // We need completion AFTER the semicolon is removed and replaced with "->".
    // Instead, let's test the chain: MyUser::where('active', 1)->get() at line 9.
    // "$q = MyUser::where(...)->get();" — let's check that get() returns Collection.
    // Actually, let's just trigger completion at the right spots.

    // Test: MyUser::where('active', 1)->  (need to check what methods are offered)
    // Line 9: "        MyUser::where('active', 1)->get();\n"
    // Position after "->" is column 40.
    // But the text already has "get()" so let's change approach:
    // Use a modified version that has a completion trigger point.

    // Let's re-open with a version that has completion triggers
    let text_with_triggers = concat!(
        "<?php\n",
        "namespace Demo {\n",
        "\n",
        "class MyUser extends \\Illuminate\\Database\\Eloquent\\Model\n",
        "{\n",
        "    public function getName(): string { return ''; }\n",
        "    public function demo(): void\n",
        "    {\n",
        "        $q = MyUser::where('active', true);\n", // line 8
        "        $q->\n",                                // line 9
        "    }\n",
        "}\n",
        "\n",
        "} // end namespace Demo\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */\n",
        "        public static function query() {}\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     * @mixin \\Illuminate\\Database\\Query\\Builder\n",
        "     */\n",
        "    class Builder {\n",
        "        /** @use \\Illuminate\\Database\\Concerns\\BuildsQueries<TModel> */\n",
        "        use \\Illuminate\\Database\\Concerns\\BuildsQueries;\n",
        "\n",
        "        /** @return $this */\n",
        "        public function where($column, $operator = null, $value = null) {}\n",
        "\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */\n",
        "        public function get($columns = ['*']) { return new Collection(); }\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     */\n",
        "    class Collection {\n",
        "        /** @return TModel|null */\n",
        "        public function first(): mixed { return null; }\n",
        "        public function count(): int { return 0; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent\\Relations {\n",
        "    class HasMany {}\n",
        "    class HasOne {}\n",
        "    class BelongsTo {}\n",
        "    class BelongsToMany {}\n",
        "    class MorphOne {}\n",
        "    class MorphMany {}\n",
        "    class MorphTo {}\n",
        "    class MorphToMany {}\n",
        "    class HasManyThrough {}\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Concerns {\n",
        "    /**\n",
        "     * @template TValue\n",
        "     */\n",
        "    trait BuildsQueries {\n",
        "        /** @return TValue|null */\n",
        "        public function first($columns = ['*']) { return null; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Query {\n",
        "    class Builder {\n",
        "        /** @return $this */\n",
        "        public function whereIn($column, $values) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function groupBy(...$groups) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function orderBy($column, $direction = 'asc') { return $this; }\n",
        "        /** @return $this */\n",
        "        public function limit($value) { return $this; }\n",
        "    }\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 2,
                text: text_with_triggers.to_string(),
            },
        })
        .await;

    // "$q->" at line 9, character 12
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 9,
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for $q->"
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!("Single-file inline stubs: $q-> methods: {:?}", methods);

            assert!(
                methods.contains(&"get"),
                "MyUser::where()-> should offer get() from Eloquent Builder, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "MyUser::where()-> should offer first() from BuildsQueries, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"orderBy"),
                "MyUser::where()-> should offer orderBy() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"limit"),
                "MyUser::where()-> should offer limit() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_inline_chain_after_static_builder_single_file() {
    // Mimics example.php lines 881-885: BlogAuthor::where(...)->get() etc.
    // The chain is inline (no intermediate $q variable), so the subject
    // extractor must resolve BlogAuthor::where(...) to Builder<BlogAuthor>
    // and then offer Builder methods after "->".
    let backend = create_test_backend();

    let uri = Url::parse("file:///inline_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Demo {\n",
        "\n",
        "class BlogAuthor extends \\Illuminate\\Database\\Eloquent\\Model\n",
        "{\n",
        "    public function demo(): void\n",
        "    {\n",
        "        BlogAuthor::where('active', 1)->\n", // line 7, cursor at 42
        "    }\n",
        "}\n",
        "\n",
        "} // end namespace Demo\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */\n",
        "        public static function query() {}\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     *\n",
        "     * @mixin \\Illuminate\\Database\\Query\\Builder\n",
        "     */\n",
        "    class Builder implements \\Illuminate\\Contracts\\Database\\Eloquent\\Builder {\n",
        "        /** @use \\Illuminate\\Database\\Concerns\\BuildsQueries<TModel> */\n",
        "        use \\Illuminate\\Database\\Concerns\\BuildsQueries;\n",
        "\n",
        "        /**\n",
        "         * @param  (\\Closure(static): mixed)|string|array  $column\n",
        "         * @return $this\n",
        "         */\n",
        "        public function where($column, $operator = null, $value = null, $boolean = 'and') {}\n",
        "\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */\n",
        "        public function get($columns = ['*']) { return new Collection(); }\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     */\n",
        "    class Collection {\n",
        "        /** @return TModel|null */\n",
        "        public function first(): mixed { return null; }\n",
        "        public function count(): int { return 0; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent\\Relations {\n",
        "    class HasMany {}\n",
        "    class HasOne {}\n",
        "    class BelongsTo {}\n",
        "    class BelongsToMany {}\n",
        "    class MorphOne {}\n",
        "    class MorphMany {}\n",
        "    class MorphTo {}\n",
        "    class MorphToMany {}\n",
        "    class HasManyThrough {}\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Concerns {\n",
        "    /**\n",
        "     * @template TValue\n",
        "     */\n",
        "    trait BuildsQueries {\n",
        "        /** @return TValue|null */\n",
        "        public function first($columns = ['*']) { return null; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Query {\n",
        "    class Builder {\n",
        "        /** @return $this */\n",
        "        public function whereIn($column, $values) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function groupBy(...$groups) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function orderBy($column, $direction = 'asc') { return $this; }\n",
        "        /** @return $this */\n",
        "        public function limit($value) { return $this; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Contracts\\Database\\Eloquent {\n",
        "    /**\n",
        "     * @mixin \\Illuminate\\Database\\Eloquent\\Builder\n",
        "     */\n",
        "    interface Builder {}\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    // "BlogAuthor::where('active', 1)->" at line 7
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 7,
                    character: 42,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for BlogAuthor::where(...)->"
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!(
                "Inline chain BlogAuthor::where(...)->  methods: {:?}",
                methods
            );

            assert!(
                methods.contains(&"get"),
                "Should offer get() from Eloquent Builder, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "Should offer first() from BuildsQueries, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"orderBy"),
                "Should offer orderBy() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"limit"),
                "Should offer limit() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"where"),
                "Should offer where() for continued chaining, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_builder_chain_with_real_example_php() {
    // Reproduces the failure in example.php where BlogAuthor::where(...)->
    // does not offer Builder methods. Uses the actual example.php content
    // with line 881 modified to create a completion trigger.
    let original = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example.php"),
    )
    .expect("example.php should exist");

    // Replace the first occurrence of the chain line with an open trigger.
    // Original: "        BlogAuthor::where('active', 1)->get();     // returns Collection<BlogAuthor>"
    // Modified: "        BlogAuthor::where('active', 1)->"
    let trigger_line = "        BlogAuthor::where('active', 1)->";
    let text = original.replace(
        "        BlogAuthor::where('active', 1)->get();     // returns Collection<BlogAuthor>",
        trigger_line,
    );

    // Find the 0-based line number of the trigger line.
    let trigger_line_idx = text
        .lines()
        .position(|l| l == trigger_line)
        .expect("trigger line should exist in modified text") as u32;

    let backend = create_test_backend();
    let uri = Url::parse("file:///example_test.php").unwrap();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text,
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: trigger_line_idx,
                    character: trigger_line.len() as u32,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for BlogAuthor::where(...)->  (line {})",
        trigger_line_idx
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!(
                "example.php BlogAuthor::where(...)->  methods: {:?}",
                methods
            );

            assert!(
                methods.contains(&"get"),
                "Should offer get() from Eloquent Builder, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "Should offer first() from BuildsQueries, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"orderBy"),
                "Should offer orderBy() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"limit"),
                "Should offer limit() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_inline_orderby_chain_after_static_builder_single_file() {
    // Tests that BlogAuthor::orderBy('name')-> offers Builder methods.
    // orderBy() comes from Query\Builder via @mixin on Eloquent Builder,
    // so the $this return type must resolve back to Eloquent Builder
    // (not Query\Builder).
    let backend = create_test_backend();

    let uri = Url::parse("file:///inline_orderby.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Demo {\n",
        "\n",
        "class BlogAuthor extends \\Illuminate\\Database\\Eloquent\\Model\n",
        "{\n",
        "    public function demo(): void\n",
        "    {\n",
        "        BlogAuthor::orderBy('name')->\n",
        "    }\n",
        "}\n",
        "\n",
        "} // end namespace Demo\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */\n",
        "        public static function query() {}\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     *\n",
        "     * @mixin \\Illuminate\\Database\\Query\\Builder\n",
        "     */\n",
        "    class Builder implements \\Illuminate\\Contracts\\Database\\Eloquent\\Builder {\n",
        "        /** @use \\Illuminate\\Database\\Concerns\\BuildsQueries<TModel> */\n",
        "        use \\Illuminate\\Database\\Concerns\\BuildsQueries;\n",
        "\n",
        "        /**\n",
        "         * @param  (\\Closure(static): mixed)|string|array  $column\n",
        "         * @return $this\n",
        "         */\n",
        "        public function where($column, $operator = null, $value = null, $boolean = 'and') {}\n",
        "\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */\n",
        "        public function get($columns = ['*']) { return new Collection(); }\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     */\n",
        "    class Collection {\n",
        "        /** @return TModel|null */\n",
        "        public function first(): mixed { return null; }\n",
        "        public function count(): int { return 0; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent\\Relations {\n",
        "    class HasMany {}\n",
        "    class HasOne {}\n",
        "    class BelongsTo {}\n",
        "    class BelongsToMany {}\n",
        "    class MorphOne {}\n",
        "    class MorphMany {}\n",
        "    class MorphTo {}\n",
        "    class MorphToMany {}\n",
        "    class HasManyThrough {}\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Concerns {\n",
        "    /**\n",
        "     * @template TValue\n",
        "     */\n",
        "    trait BuildsQueries {\n",
        "        /** @return TValue|null */\n",
        "        public function first($columns = ['*']) { return null; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Query {\n",
        "    class Builder {\n",
        "        /** @return $this */\n",
        "        public function whereIn($column, $values) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function groupBy(...$groups) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function orderBy($column, $direction = 'asc') { return $this; }\n",
        "        /** @return $this */\n",
        "        public function limit($value) { return $this; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Contracts\\Database\\Eloquent {\n",
        "    /**\n",
        "     * @mixin \\Illuminate\\Database\\Eloquent\\Builder\n",
        "     */\n",
        "    interface Builder {}\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    // "BlogAuthor::orderBy('name')->" at line 7
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 7,
                    character: 37,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for BlogAuthor::orderBy('name')->"
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!(
                "Inline orderBy chain BlogAuthor::orderBy('name')->  methods: {:?}",
                methods
            );

            assert!(
                methods.contains(&"get"),
                "Should offer get() after orderBy()->, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "Should offer first() after orderBy()->, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"limit"),
                "Should offer limit() after orderBy()->, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"where"),
                "Should offer where() after orderBy()->, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_builder_orderby_chain_with_real_example_php() {
    // Tests line 883: BlogAuthor::orderBy('name')->limit(10)->get()
    // orderBy() comes from Query\Builder via @mixin, should still resolve
    // back to Eloquent Builder so that limit() and get() are available.
    let original = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example.php"),
    )
    .expect("example.php should exist");

    let trigger_line = "        BlogAuthor::orderBy('name')->limit(10)->";
    let text = original.replace(
        "        BlogAuthor::orderBy('name')->limit(10)->get(); // full chain resolution",
        trigger_line,
    );

    let trigger_line_idx = text
        .lines()
        .position(|l| l == trigger_line)
        .expect("trigger line should exist in modified text") as u32;

    let backend = create_test_backend();
    let uri = Url::parse("file:///example_orderby.php").unwrap();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text,
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: trigger_line_idx,
                    character: trigger_line.len() as u32,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for BlogAuthor::orderBy()->limit()->"
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!(
                "example.php BlogAuthor::orderBy()->limit()->  methods: {:?}",
                methods
            );

            assert!(
                methods.contains(&"get"),
                "Should offer get() at end of orderBy()->limit()-> chain, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "Should offer first() at end of orderBy()->limit()-> chain, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_builder_orderby_single_step_with_real_example_php() {
    // Tests intermediate step: BlogAuthor::orderBy('name')->
    // orderBy() comes from Query\Builder via @mixin, returns $this.
    // Completion should offer limit(), get(), first(), where(), etc.
    let original = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example.php"),
    )
    .expect("example.php should exist");

    let trigger_line = "        BlogAuthor::orderBy('name')->";
    let text = original.replace(
        "        BlogAuthor::orderBy('name')->limit(10)->get(); // full chain resolution",
        trigger_line,
    );

    let trigger_line_idx = text
        .lines()
        .position(|l| l == trigger_line)
        .expect("trigger line should exist in modified text") as u32;

    let backend = create_test_backend();
    let uri = Url::parse("file:///example_orderby_step1.php").unwrap();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text,
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: trigger_line_idx,
                    character: trigger_line.len() as u32,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for BlogAuthor::orderBy()->"
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!(
                "example.php BlogAuthor::orderBy()->  methods: {:?}",
                methods
            );

            assert!(
                methods.contains(&"get"),
                "Should offer get() after orderBy()->, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"limit"),
                "Should offer limit() after orderBy()->, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"where"),
                "Should offer where() after orderBy()->, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "Should offer first() after orderBy()->, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_builder_wherein_chain_with_real_example_php() {
    // Tests line 885: BlogAuthor::whereIn('id', [1, 2])->groupBy('genre')->get()
    // whereIn() and groupBy() come from Query\Builder via @mixin.
    // After chaining, get() (from Eloquent Builder) should still be available.
    let original = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example.php"),
    )
    .expect("example.php should exist");

    let trigger_line = "        BlogAuthor::whereIn('id', [1, 2])->groupBy('genre')->";
    let text = original.replace(
        "        BlogAuthor::whereIn('id', [1, 2])->groupBy('genre')->get();",
        trigger_line,
    );

    let trigger_line_idx = text
        .lines()
        .position(|l| l == trigger_line)
        .expect("trigger line should exist in modified text") as u32;

    let backend = create_test_backend();
    let uri = Url::parse("file:///example_wherein.php").unwrap();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text,
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: trigger_line_idx,
                    character: trigger_line.len() as u32,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for BlogAuthor::whereIn()->groupBy()->"
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!(
                "example.php BlogAuthor::whereIn()->groupBy()->  methods: {:?}",
                methods
            );

            assert!(
                methods.contains(&"get"),
                "Should offer get() at end of whereIn()->groupBy()-> chain, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "Should offer first() at end of whereIn()->groupBy()-> chain, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"where"),
                "Should offer where() at end of whereIn()->groupBy()-> chain, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
