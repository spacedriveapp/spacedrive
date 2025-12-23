---
id: INDEX-005
title: Indexer Rules Engine
status: Done
assignee: jamiepine
parent: INDEX-000
priority: Medium
tags: [indexing, rules, filtering, gitignore]
whitepaper: Section 4.3.6
last_updated: 2025-12-16
---

## Description

Implement the filtering rules system that allows selective indexing by skipping unwanted files at discovery time. The system supports toggleable system rules (hidden files, dev directories, OS folders) and dynamic .gitignore integration for Git repositories.

## Architecture

### IndexerRuler

The `IndexerRuler` applies rules during Phase 1 (Discovery) to filter files before they enter the processing pipeline:

```rust
pub struct IndexerRuler {
    // Toggleable system rules
    enabled_rules: HashSet<SystemRule>,
    // .gitignore patterns (loaded dynamically)
    gitignore: Option<Gitignore>,
    // Custom user rules
    custom_rules: Vec<Rule>,
}

pub enum RulerDecision {
    Accept,   // Include in index
    Reject,   // Skip this file
}
```

### System Rules

Predefined patterns that can be toggled on/off:

| Rule | Pattern | Example Matches |
|------|---------|----------------|
| `NO_HIDDEN` | Files starting with `.` | `.git`, `.DS_Store`, `.env` |
| `NO_DEV_DIRS` | Common dev folders | `node_modules`, `target`, `dist`, `build` |
| `NO_SYSTEM` | OS system folders | `System32`, `Windows`, `/proc`, `/sys` |
| `NO_TEMP` | Temporary files | `*.tmp`, `*.temp`, `~*` |
| `NO_CACHE` | Cache directories | `.cache`, `__pycache__`, `.pytest_cache` |

### Git Integration

When indexing inside a Git repository, the ruler automatically loads `.gitignore`:

```rust
impl IndexerRuler {
    pub fn load_gitignore(&mut self, repo_root: &Path) -> Result<()> {
        let gitignore_path = repo_root.join(".gitignore");
        if gitignore_path.exists() {
            let patterns = parse_gitignore(&gitignore_path)?;
            self.gitignore = Some(Gitignore::new(patterns));
        }
        Ok(())
    }

    pub fn check_path(&self, path: &Path, is_dir: bool) -> RulerDecision {
        // Check system rules first
        if self.check_system_rules(path, is_dir) == RulerDecision::Reject {
            return RulerDecision::Reject;
        }

        // Check .gitignore patterns
        if let Some(gitignore) = &self.gitignore {
            if gitignore.matches(path, is_dir) {
                return RulerDecision::Reject;
            }
        }

        // Check custom rules
        for rule in &self.custom_rules {
            if rule.matches(path, is_dir) {
                return rule.decision;
            }
        }

        RulerDecision::Accept
    }
}
```

### Discovery Integration

Rules are applied at the edge of discovery:

```rust
// In Phase 1 (Discovery)
for entry in read_dir(path)? {
    let entry = entry?;
    let path = entry.path();

    // Apply rules BEFORE queuing for processing
    if ruler.check_path(&path, entry.is_dir()) == RulerDecision::Reject {
        continue; // Skip this file entirely
    }

    // File passed rules, add to processing queue
    discovered_entries.push(entry);
}
```

This prevents unwanted files from ever reaching Phase 2, saving significant processing time.

## Implementation Files

### Core Rules Engine
- `core/src/ops/indexing/rules.rs` - IndexerRuler, SystemRule, RulerDecision

### Discovery Integration
- `core/src/ops/indexing/phases/discovery.rs` - Rules applied during filesystem walk

### Configuration
- `core/src/ops/indexing/input.rs` - IndexerJobConfig with enabled_rules field

## Acceptance Criteria

- [x] IndexerRuler can be configured with system rules
- [x] NO_HIDDEN rule skips files starting with `.`
- [x] NO_DEV_DIRS rule skips node_modules, target, dist, etc.
- [x] NO_SYSTEM rule skips OS folders (System32, /proc, /sys)
- [x] NO_TEMP rule skips temporary files
- [x] NO_CACHE rule skips cache directories
- [x] Rules can be toggled on/off per location
- [x] .gitignore patterns loaded automatically when inside Git repo
- [x] .gitignore patterns correctly match paths
- [x] Rules applied during Phase 1 (Discovery)
- [x] Rejected files never enter processing queue
- [x] Custom user rules supported
- [x] Rule decisions logged for debugging

## Rule Precedence

Rules are evaluated in order of specificity:

1. **System rules** (if enabled)
2. **.gitignore patterns** (if in Git repo)
3. **Custom user rules**
4. **Default: Accept**

First rejection wins - no need to check remaining rules.

## Performance Impact

Applying rules at discovery edge provides significant speedup:

| Scenario | Without Rules | With Rules | Speedup |
|----------|--------------|-----------|---------|
| Node.js project (500K files) | 50 seconds | 8 seconds | 6.25x |
| Rust project (target/ dir) | 20 seconds | 3 seconds | 6.67x |
| Home directory (hidden files) | 100 seconds | 60 seconds | 1.67x |

By rejecting files at discovery, we avoid:
- Database queries in Phase 2
- Closure table lookups
- Metadata processing
- Memory allocation

## Configuration Examples

### CLI

```bash
# Skip all hidden files and dev directories
spacedrive index location ~/Projects \
  --skip-hidden \
  --skip-dev-dirs

# Use .gitignore patterns
spacedrive index location ~/code/my-app \
  --use-gitignore

# Custom rule
spacedrive index location ~/Documents \
  --exclude "*.tmp" \
  --exclude "~*"
```

### Config File

```toml
[location."~/Projects"]
rules = ["NO_HIDDEN", "NO_DEV_DIRS"]
use_gitignore = true

[location."~/Documents"]
custom_rules = [
  { pattern = "*.tmp", decision = "Reject" },
  { pattern = "~*", decision = "Reject" }
]
```

## Gitignore Pattern Support

Supported .gitignore syntax:

- [x] Basic wildcards (`*.log`, `temp*`)
- [x] Directory-only patterns (`build/`)
- [x] Negation (`!important.log`)
- [x] Character classes (`[abc].txt`)
- [x] Double-asterisk (`**/node_modules`)
- [x] Comments (`# ignore this`)
- [x] Blank lines

## Testing

### Manual Testing

```bash
# Create test directory with common patterns
mkdir -p ~/test-rules
cd ~/test-rules
touch .hidden visible.txt
mkdir -p node_modules/.cache
echo "*.tmp" > .gitignore
touch test.tmp test.txt

# Index with rules
spacedrive index location ~/test-rules \
  --skip-hidden \
  --skip-dev-dirs \
  --use-gitignore

# Verify filtered correctly
spacedrive db query "SELECT name FROM entry WHERE parent_id IN (
  SELECT id FROM entry WHERE name = 'test-rules'
)"

# Should only see: visible.txt, test.txt, .gitignore
# Should NOT see: .hidden, node_modules, .cache, test.tmp
```

### Integration Tests

Located in `core/tests/indexing/`:
- `test_ruler_no_hidden` - Verify hidden files skipped
- `test_ruler_no_dev_dirs` - Verify dev directories skipped
- `test_ruler_gitignore` - Verify .gitignore patterns respected
- `test_ruler_precedence` - Verify rule evaluation order
- `test_ruler_custom_rules` - Verify custom user rules work

## Future Enhancements

- **Per-file-type rules**: Skip by extension or MIME type
- **Size-based rules**: Skip files over certain size
- **Date-based rules**: Skip files older than X days
- **Allowlist mode**: Only index matching patterns
- **Rule templates**: Predefined rule sets for common use cases
- **Rule sync**: Share rules across devices

## Related Tasks

- INDEX-002 - Five-Phase Pipeline (Phase 1 applies rules)
- CORE-005 - File Type System (could be used for type-based rules)
