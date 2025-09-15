# Semantic Tagging System - Developer Usage Guide

## Quick Start

The semantic tagging system is now production-ready! Here's how to use it in your code.

### Basic Setup

```rust
use spacedrive_core::{
    service::{
        semantic_tag_service::SemanticTagService,
        user_metadata_service::UserMetadataService,
        semantic_tagging_facade::SemanticTaggingFacade,
    },
    domain::semantic_tag::{TagType, PrivacyLevel, TagSource},
};

// In your service/component:
let db = library.db();
let facade = SemanticTaggingFacade::new(db.clone());
let device_id = library.device_id();
```

## Common Use Cases

### 1. User Manually Tags a File

```rust
// User selects a photo and adds tags: "vacation", "family", "beach"
let entry_id = 12345; // From user selection
let tag_names = vec!["vacation".to_string(), "family".to_string(), "beach".to_string()];

let applied_tag_ids = facade.tag_entry(entry_id, tag_names, device_id).await?;

println!("Applied {} tags to entry", applied_tag_ids.len());
```

The system will:
- Find existing tags or create new ones
- Apply them to the file's metadata
- Track usage patterns for future suggestions
- Enable immediate search by these tags

### 2. AI Analyzes Content and Suggests Tags

```rust
// AI analyzes an image and detects objects
let ai_suggestions = vec![
    ("dog".to_string(), 0.95, "object_detection".to_string()),
    ("beach".to_string(), 0.87, "scene_analysis".to_string()),
    ("sunset".to_string(), 0.82, "lighting_analysis".to_string()),
];

let applied_tags = facade.apply_ai_tags(entry_id, ai_suggestions, device_id).await?;

// User can review AI suggestions in UI and approve/reject them
```

### 3. Create Organizational Hierarchy

```rust
// Build: Technology → Programming → Web Development → Frontend → React
let hierarchy = vec![
    ("Technology".to_string(), None),
    ("Programming".to_string(), Some("Technology".to_string())),
    ("Web Development".to_string(), Some("Technology".to_string())),
    ("Frontend".to_string(), Some("Technology".to_string())),
    ("React".to_string(), Some("Technology".to_string())),
];

let tags = facade.create_tag_hierarchy(hierarchy, device_id).await?;

// Now tagging a file with "React" automatically inherits the hierarchy
```

### 4. Handle Ambiguous Tag Names (Polymorphic Naming)

```rust
// Create disambiguated "Phoenix" tags
let phoenix_city = facade.create_namespaced_tag(
    "Phoenix".to_string(),
    "Geography".to_string(),
    Some("#FF6B35".to_string()), // Orange for cities
    device_id,
).await?;

let phoenix_framework = facade.create_namespaced_tag(
    "Phoenix".to_string(), 
    "Technology".to_string(),
    Some("#9D4EDD".to_string()), // Purple for tech
    device_id,
).await?;

// When user types "Phoenix", system uses context to pick the right one
```

### 5. Search Files by Tags (Hierarchical)

```rust
// Find all "Technology" files (includes React, JavaScript, etc.)
let tech_files = facade.find_files_by_tags(
    vec!["Technology".to_string()],
    true  // include_descendants - searches entire hierarchy
).await?;

// Find specific combination
let web_files = facade.find_files_by_tags(
    vec!["Web Development".to_string(), "React".to_string()],
    false // exact match only
).await?;
```

### 6. Smart Tag Suggestions

```rust
// Get suggestions based on existing tags
let suggestions = facade.suggest_tags_for_entry(entry_id, 5).await?;

for (suggested_tag, confidence) in suggestions {
    println!("Suggest '{}' with {:.1}% confidence", 
             suggested_tag.canonical_name, 
             confidence * 100.0);
}

// UI can show these as one-click applications
```

## Action System Integration

### CLI Integration

```rust
// In CLI command handler:
use spacedrive_core::ops::tags::{CreateTagAction, CreateTagInput, ApplyTagsAction, ApplyTagsInput};

// Create tag via action system
let create_input = CreateTagInput::simple("Important".to_string());
let action = CreateTagAction::from_input(create_input)?;
let result = action_manager.dispatch_library(library_id, action).await?;

// Apply tags via action system  
let apply_input = ApplyTagsInput::user_tags(vec![entry_id], vec![tag_id]);
let action = ApplyTagsAction::from_input(apply_input)?;
let result = action_manager.dispatch_library(library_id, action).await?;
```

### GraphQL Integration (Future)

```graphql
# Create a semantic tag
mutation CreateTag($input: CreateTagInput!) {
  createTag(input: $input) {
    tagId
    canonicalName
    namespace
    message
  }
}

# Apply tags to files
mutation ApplyTags($input: ApplyTagsInput!) {
  applyTags(input: $input) {
    entriesAffected
    tagsApplied
    warnings
  }
}

# Search tags with context
query SearchTags($query: String!, $context: [ID!]) {
  searchTags(query: $query, contextTagIds: $context) {
    tags {
      tag { canonicalName namespace }
      relevance
      contextScore
    }
    disambiguated
  }
}
```

## Advanced Features

### Context Resolution (Smart Disambiguation)

```rust
// User has geographic context and types "Phoenix"
let context_tags = vec![arizona_tag, usa_tag, city_tag];
let resolved = tag_service.resolve_ambiguous_tag("Phoenix", &context_tags).await?;

// System returns "Geography::Phoenix" (city) instead of "Mythology::Phoenix" (bird)
// Based on namespace compatibility, usage patterns, and hierarchical relationships
```

### Semantic Variants (Multiple Access Points)

```rust
// Create tag with multiple access points
let js_tag = facade.create_tag_with_variants(
    "JavaScript".to_string(),
    Some("JS".to_string()),              // Abbreviation
    vec!["ECMAScript".to_string()],       // Aliases
    Some("Technology".to_string()),       // Namespace
    device_id,
).await?;

// All of these find the same tag:
// - "JavaScript"  
// - "JS"
// - "ECMAScript"
// - "JavaScript Programming Language" (if set as formal_name)
```

### Privacy Controls

```rust
// Create archive tag (hidden from normal search)
let mut personal_tag = tag_service.create_tag(
    "Personal".to_string(),
    None,
    device_id
).await?;

personal_tag.tag_type = TagType::Privacy;
personal_tag.privacy_level = PrivacyLevel::Archive;

// Files tagged with this won't appear in normal searches
// But can be found with: search_tags("", None, None, true) // include_archived = true
```

### AI Integration with Confidence

```rust
// AI analyzes code file
let ai_applications = vec![
    TagApplication::ai_applied(javascript_tag_id, 0.98, device_id),
    TagApplication::ai_applied(react_tag_id, 0.85, device_id),
    TagApplication::ai_applied(typescript_tag_id, 0.72, device_id), // Lower confidence
];

// Set context and attributes
for app in &mut ai_applications {
    app.applied_context = Some("code_analysis".to_string());
    app.set_instance_attribute("model_version", "v2.1")?;
}

metadata_service.apply_semantic_tags(entry_id, ai_applications, device_id).await?;

// UI can show low-confidence tags for user review
```

## Performance Considerations

### Efficient Hierarchy Queries

```rust
// ✅ FAST: Uses closure table - O(1) complexity
let descendants = tag_service.get_descendants(technology_tag_id).await?;

// ✅ FAST: Direct database query with indexes
let tech_files = metadata_service.find_entries_by_semantic_tags(
    &[technology_tag_id], 
    true  // include_descendants
).await?;
```

### Bulk Operations

```rust
// ✅ EFFICIENT: Apply multiple tags in one operation
let tag_applications = vec![
    TagApplication::user_applied(tag1_id, device_id),
    TagApplication::user_applied(tag2_id, device_id),
    TagApplication::user_applied(tag3_id, device_id),
];

metadata_service.apply_semantic_tags(entry_id, tag_applications, device_id).await?;

// ✅ EFFICIENT: Batch tag creation
let tag_ids = facade.tag_entry(
    entry_id,
    vec!["project".to_string(), "urgent".to_string(), "2024".to_string()],
    device_id
).await?;
```

### Search Performance

```rust
// ✅ FAST: Uses FTS5 full-text search
let results = tag_service.search_tags(
    "javascript react web",
    Some("Technology"),  // Namespace filter
    None,               // No type filter
    false              // Exclude archived
).await?;

// Returns ranked results across all name variants
```

## Error Handling

```rust
use spacedrive_core::domain::semantic_tag::TagError;

match facade.create_simple_tag("".to_string(), None, device_id).await {
    Ok(tag) => println!("Created tag: {}", tag.canonical_name),
    Err(TagError::NameConflict(msg)) => println!("Name conflict: {}", msg),
    Err(TagError::InvalidCompositionRule(msg)) => println!("Validation error: {}", msg),
    Err(TagError::DatabaseError(msg)) => println!("Database error: {}", msg),
    Err(e) => println!("Other error: {}", e),
}
```

## Integration Points

### With Indexing System
```rust
// During file indexing, automatically apply content-based tags
if entry.kind == EntryKind::File {
    match detect_file_type(&entry) {
        FileType::Image => {
            let ai_tags = analyze_image_content(&entry_path).await?;
            facade.apply_ai_tags(entry.id, ai_tags, device_id).await?;
        }
        FileType::Code => {
            let language_tag = detect_programming_language(&entry_path).await?;
            facade.apply_ai_tags(entry.id, vec![language_tag], device_id).await?;
        }
        _ => {}
    }
}
```

### With Search System
```rust
// Enhanced search using semantic tags
let search_results = SearchAction::new(SearchInput {
    query: "React components".to_string(),
    use_semantic_tags: true,
    include_tag_hierarchy: true,
}).execute(library, context).await?;
```

### With Sync System (Future)
```rust
// When Library Sync is implemented, conflicts resolve automatically:
let merged_result = tag_service.merge_tag_applications(
    local_tag_applications,
    remote_tag_applications,
).await?;

// Union merge: "vacation" + "family" = "vacation, family" (no data loss)
```

## Database Schema Integration

The semantic tagging system integrates seamlessly with existing Spacedrive tables:

```
entries
  ↓ metadata_id
user_metadata ←→ user_metadata_semantic_tags ←→ semantic_tags
                                                       ↓
                                                 tag_relationships
                                                       ↓  
                                                   tag_closure
```

This preserves the existing "every Entry has immediate metadata" architecture while adding sophisticated semantic capabilities.

## Migration Path

Since this is a development codebase:

1. **Deploy migration**: `m20250115_000001_semantic_tags.rs` creates all tables
2. **Start using semantic tags**: Existing simple tags continue working  
3. **UI enhancement**: Gradually expose semantic features to users
4. **Feature rollout**: Enable advanced features (hierarchy, AI, etc.) progressively

No user data migration required - this is a clean, additive enhancement.

## What's Production Ready ✅

- Complete database schema with optimal performance
- Full service layer with all operations implemented  
- Action system integration for CLI/API usage
- Comprehensive validation and error handling
- Union merge conflict resolution (interface ready for sync)
- Usage pattern tracking for AI suggestions
- Privacy controls and organizational features
- Full Unicode support for international users

The semantic tagging system transforms Spacedrive from having simple labels to providing the sophisticated semantic fabric described in the whitepaper - enabling true content-aware organization at scale.