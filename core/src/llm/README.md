# AI Engine

The AI Engine is a core module of Spacedrive that provides intelligent file system operations and context-aware assistance through configurable LLM-powered agents. It uses a flexible RON-based configuration system to define agent behaviors, tools, and workflows.

## Overview

The AI Engine module enables Spacedrive to perform intelligent operations on the file system by:

- Processing natural language queries about files and directories
- Understanding file context and relationships
- Executing complex file operations through LLM reasoning
- Maintaining conversational context about file operations

## Architecture

### Core Components

```
ai-engine/
├── agents/
│   ├── directory_agent.rs     # File system navigation agent
│   ├── context_agent.rs       # Content understanding agent
│   └── mod.rs
├── tools/
│   ├── spacedrive_fs.rs       # File system operations
│   ├── content_analyzer.rs    # File content analysis
│   └── mod.rs
├── config/
│   ├── parser.rs             # RON configuration parser
│   ├── validator.rs          # Configuration validation
│   └── templates/
│       └── directory_agent.ron
├── memory/
│   ├── conversation.rs       # Conversation state management
│   └── storage.rs           # Memory backend implementations
└── mod.rs
```

### Agent Configuration

Agents are configured using RON (Rust Object Notation) files that define:

- Model parameters and provider settings
- Available tools and their parameters
- Workflow execution strategies
- Memory management
- Prompt templates
- Validation rules

Example configuration for the Directory Agent:

```ron
(
    agent: (
        name: "DirectoryAgent",
        description: "File system navigation and context evaluation agent",
        model: (
            provider: "ollama",
            name: "llama3.1-70b-instruct",
            // ... other model settings
        ),
        // ... tool definitions, workflow config, etc.
    )
)
```

## Usage

### Basic Integration

```rust
use spacedrive_core::ai_engine::{Agent, AgentConfig};

async fn create_directory_agent() -> Result<Agent> {
    // Load configuration from RON file
    let config = AgentConfig::from_file("config/directory_agent.ron")?;

    // Initialize agent
    let agent = Agent::new(config).await?;

    // Execute queries
    let response = agent.execute("Find all images in the Downloads folder").await?;

    Ok(agent)
}
```

### Custom Tool Implementation

```rust
use spacedrive_core::ai_engine::tools::{Tool, ToolResult};

#[derive(Debug)]
struct SpacedriveFs;

#[async_trait]
impl Tool for SpacedriveFs {
    async fn execute(&self, params: HashMap<String, Value>) -> ToolResult {
        // Implement file system operations
        // ...
    }
}
```

## Memory Management

The AI Engine supports different memory backends:

- In-Memory (default): Temporary storage for the session
- Redis: Distributed memory storage
- PostgreSQL: Persistent conversation history

Configure memory settings in the agent's RON file:

```ron
memory: (
    type: "Conversational",
    storage: (
        type: "Redis",
        connection_string: Some("redis://localhost:6379"),
        ttl_seconds: Some(3600),
    ),
)
```

## Error Handling

The module implements comprehensive error handling:

- Automatic retries with exponential backoff
- Fallback responses for failed operations
- Detailed error logging and reporting

Configuration example:

```ron
error_strategy: (
    max_retries: 3,
    backoff_seconds: 2,
    fallback_response: "Operation failed, please try again.",
)
```

## Development

### Adding New Tools

1. Create a new tool implementation in `tools/`:

```rust
#[derive(Debug)]
pub struct NewTool;

#[async_trait]
impl Tool for NewTool {
    async fn execute(&self, params: HashMap<String, Value>) -> ToolResult {
        // Tool implementation
    }
}
```

2. Add tool configuration to agent RON file:

```ron
tools: [
    (
        name: "new_tool",
        description: "Description of the new tool",
        required_params: [
            // Parameter definitions
        ],
    ),
]
```

### Testing

Run the test suite:

```bash
cargo test -p spacedrive-core ai_engine
```

Run specific agent tests:

```bash
cargo test -p spacedrive-core ai_engine::agents::directory_agent
```

## Contributing

When contributing to the AI Engine:

1. Ensure all new tools implement the `Tool` trait
2. Add appropriate tests for new functionality
3. Update RON schema documentation
4. Follow Rust best practices and Spacedrive's coding style

## Future Developments

Planned features:

- [ ] Additional LLM provider integrations
- [ ] Enhanced file content understanding
- [ ] Improved memory management systems
- [ ] Multi-agent collaboration
- [ ] Custom tool development framework

## License

This module is part of Spacedrive and is licensed under the same terms as the main project.
