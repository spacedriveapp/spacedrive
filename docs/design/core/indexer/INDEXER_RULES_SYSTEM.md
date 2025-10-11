<!--CREATED: 2025-06-19-->
# Indexer Rules System Design

## Overview

This document outlines the design for implementing an indexer rules system in Spacedrive's new core architecture. The system allows users to define flexible rules that control which files and directories are included or excluded during indexing operations.

## Goals

1. **Flexibility**: Support multiple rule types (glob patterns, regex, file attributes, git integration)
2. **Performance**: Minimal impact on indexing speed through efficient rule evaluation
3. **Persistence**: Store rules in the database with proper relationships to locations
4. **Extensibility**: Easy to add new rule types without major refactoring
5. **User Control**: Allow users to create, modify, and delete rules per location
6. **System Defaults**: Provide sensible default rules that can be overridden

## Architecture

### Domain Model

```rust
// core/src/domain/indexer_rule.rs
pub struct IndexerRule {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,  // System rules cannot be deleted
    pub is_enabled: bool,
    pub priority: i32,    // Higher priority rules evaluated first
    pub rule_type: IndexerRuleType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum IndexerRuleType {
    // Path-based rules
    AcceptGlob { patterns: Vec<String> },
    RejectGlob { patterns: Vec<String> },
    AcceptRegex { patterns: Vec<String> },
    RejectRegex { patterns: Vec<String> },

    // Directory rules
    AcceptIfChildExists { children: Vec<String> },
    RejectIfChildExists { children: Vec<String> },

    // File attribute rules
    RejectLargerThan { size_bytes: u64 },
    RejectOlderThan { days: u32 },
    AcceptExtensions { extensions: Vec<String> },
    RejectExtensions { extensions: Vec<String> },

    // Integration rules
    RespectGitignore,
    RejectSystemFiles,
    RejectHiddenFiles,
}

pub struct LocationRules {
    pub location_id: Uuid,
    pub rules: Vec<IndexerRule>,
    pub inherit_system_rules: bool,
}
```

### Database Schema

```sql
-- Rules definition table
CREATE TABLE indexer_rules (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    priority INTEGER NOT NULL DEFAULT 0,
    rule_type_discriminator VARCHAR(50) NOT NULL,
    rule_data JSONB NOT NULL, -- Stores type-specific data
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- Location-rule relationships
CREATE TABLE location_indexer_rules (
    location_id UUID NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    rule_id UUID NOT NULL REFERENCES indexer_rules(id) ON DELETE CASCADE,
    rule_order INTEGER NOT NULL, -- Override default priority for this location
    PRIMARY KEY (location_id, rule_id)
);

-- Rule application history (optional, for debugging)
CREATE TABLE indexer_rule_applications (
    id UUID PRIMARY KEY,
    location_id UUID NOT NULL REFERENCES locations(id),
    rule_id UUID NOT NULL REFERENCES indexer_rules(id),
    path TEXT NOT NULL,
    action VARCHAR(20) NOT NULL, -- 'accepted' or 'rejected'
    applied_at TIMESTAMPTZ NOT NULL
);
```

### Rule Evaluation Engine

```rust
// core/src/services/indexer_rules/engine.rs
pub struct IndexerRuleEngine {
    compiled_rules: Vec<CompiledRule>,
    gitignore_cache: Option<GitignoreCache>,
}

pub struct CompiledRule {
    rule: IndexerRule,
    matcher: RuleMatcher,
}

pub enum RuleMatcher {
    Glob(GlobSet),
    Regex(RegexSet),
    ChildExists(HashSet<String>),
    FileAttribute(Box<dyn Fn(&EntryMetadata) -> bool>),
    Gitignore(Gitignore),
}

impl IndexerRuleEngine {
    pub fn new(rules: Vec<IndexerRule>) -> Result<Self> {
        // Compile rules for efficient matching
        // Sort by priority
        // Initialize gitignore if needed
    }

    pub fn should_index(&self, path: &Path, metadata: &EntryMetadata) -> RuleDecision {
        // Evaluate rules in priority order
        // Short-circuit on first definitive decision
        // Return decision with matching rule for debugging
    }
}

pub struct RuleDecision {
    pub should_index: bool,
    pub matching_rule: Option<Uuid>,
    pub reason: String,
}
```

### Integration Points

#### 1. Indexer Job Integration

```rust
// Modify core/src/operations/indexing/indexer_job.rs
impl IndexerJob {
    async fn setup_rule_engine(&self, location: &Location) -> Result<IndexerRuleEngine> {
        // Load rules for location from database
        // Merge with system rules if enabled
        // Compile and cache rule engine
    }

    async fn read_directory(&self, path: &Path, rule_engine: &IndexerRuleEngine) -> Result<Vec<Entry>> {
        // Apply rules during directory traversal
        // Skip rejected paths early
        // Track rule applications if debugging enabled
    }
}
```

#### 2. Location Manager Integration

```rust
// Extend core/src/location/manager.rs
impl LocationManager {
    pub async fn create_location_with_rules(
        &self,
        path: PathBuf,
        rule_ids: Vec<Uuid>,
    ) -> Result<Location> {
        // Create location
        // Attach rules
        // Validate rule compatibility
    }

    pub async fn update_location_rules(
        &self,
        location_id: Uuid,
        rule_ids: Vec<Uuid>,
    ) -> Result<()> {
        // Update rules
        // Trigger re-indexing if needed
    }
}
```

#### 3. File Watcher Integration

```rust
// Extend core/src/services/location_watcher/event_handler.rs
impl EventHandler {
    async fn should_process_event(&self, path: &Path) -> bool {
        // Get cached rule engine for location
        // Apply rules to determine if event should be processed
        // Cache decision for performance
    }
}
```

### System Default Rules

```rust
pub fn create_system_rules() -> Vec<IndexerRule> {
    vec![
        // OS-specific system files
        IndexerRule {
            name: "Ignore System Files".to_string(),
            rule_type: IndexerRuleType::RejectGlob {
                patterns: vec![
                    "*.DS_Store".to_string(),
                    "Thumbs.db".to_string(),
                    "desktop.ini".to_string(),
                    "$RECYCLE.BIN".to_string(),
                ],
            },
            priority: 100,
            is_system: true,
            ..Default::default()
        },

        // Hidden files
        IndexerRule {
            name: "Ignore Hidden Files".to_string(),
            rule_type: IndexerRuleType::RejectHiddenFiles,
            priority: 90,
            is_system: true,
            ..Default::default()
        },

        // Development artifacts
        IndexerRule {
            name: "Ignore Development Folders".to_string(),
            rule_type: IndexerRuleType::RejectGlob {
                patterns: vec![
                    "node_modules".to_string(),
                    "__pycache__".to_string(),
                    ".git".to_string(),
                    "target".to_string(),
                    "dist".to_string(),
                ],
            },
            priority: 80,
            is_system: true,
            ..Default::default()
        },
    ]
}
```

### Performance Optimizations

1. **Compiled Rules**: Rules are compiled once during initialization
2. **Early Directory Pruning**: Skip entire directory trees when possible
3. **Rule Caching**: Cache compiled rules per location
4. **Batch Evaluation**: Evaluate multiple paths in batch when possible
5. **Priority Short-Circuit**: Stop evaluation on first definitive match

### GraphQL API

```graphql
type IndexerRule {
	id: ID!
	name: String!
	description: String
	isSystem: Boolean!
	isEnabled: Boolean!
	priority: Int!
	ruleType: IndexerRuleType!
	createdAt: DateTime!
	updatedAt: DateTime!
}

type IndexerRuleType {
	type: String!
	config: JSON!
}

type Query {
	indexerRules(locationId: ID): [IndexerRule!]!
	systemRules: [IndexerRule!]!
}

type Mutation {
	createIndexerRule(input: CreateIndexerRuleInput!): IndexerRule!
	updateIndexerRule(id: ID!, input: UpdateIndexerRuleInput!): IndexerRule!
	deleteIndexerRule(id: ID!): Boolean!

	attachRuleToLocation(locationId: ID!, ruleId: ID!): Location!
	detachRuleFromLocation(locationId: ID!, ruleId: ID!): Location!
}
```

## Implementation Plan

### Phase 1: Core Infrastructure

1. Create domain models and database schema
2. Implement rule compilation and matching logic
3. Create system default rules

### Phase 2: Indexer Integration

1. Integrate rule engine into indexer job
2. Add rule evaluation during directory traversal
3. Update database entities to track excluded paths

### Phase 3: Location Integration

1. Add rule management to location manager
2. Update location creation to support rules
3. Implement rule inheritance logic

### Phase 4: API and UI

1. Add GraphQL types and resolvers
2. Create rule management UI
3. Add rule testing/preview functionality

### Phase 5: Advanced Features

1. Git integration (.gitignore support)
2. Rule templates and presets
3. Rule import/export
4. Performance monitoring and optimization

## Migration Strategy

1. **Preserve Existing Behavior**: Map current `ignore_patterns` to new rule system
2. **Automatic Migration**: Convert existing patterns to rules during upgrade
3. **Backward Compatibility**: Support old API temporarily with deprecation warnings

## Testing Strategy

1. **Unit Tests**: Test individual rule matchers and compilation
2. **Integration Tests**: Test rule application during indexing
3. **Performance Tests**: Ensure minimal impact on indexing speed
4. **Edge Cases**: Test complex rule combinations and conflicts

## Future Enhancements

1. **Machine Learning Rules**: Auto-suggest rules based on usage patterns
2. **Cloud Rule Sharing**: Share rule sets between users
3. **Rule Analytics**: Track which rules are most effective
4. **Dynamic Rules**: Rules that adapt based on system resources
5. **Content-Based Rules**: Rules based on file content, not just metadata
