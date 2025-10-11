<!--CREATED: 2025-10-11-->
# Context Window Management - Research from Production Rust Agent Projects

**Research Date:** October 2025
**Projects Analyzed:** ccswarm, rust-agentai, rust-deep-agents-sdk
**Purpose:** Inform Spacedrive's agent context window management strategy

---

## Executive Summary

Context window management is critical for long-running AI agents to prevent:
1. **Token limit exceeded errors** (models have finite context windows)
2. **Degraded performance** (larger contexts = slower inference, higher costs)
3. **Loss of focus** (too much history dilutes current task relevance)

The three projects employ different strategies ranging from simple truncation to sophisticated multi-memory architectures. This research identifies patterns suitable for Spacedrive's extension agent system.

---

## Strategy 1: Summarization Middleware (rust-deep-agents-sdk)

### Approach: Simple Truncation with Summary Note

**Implementation:**
```rust
pub struct SummarizationMiddleware {
    pub messages_to_keep: usize,
    pub summary_note: String,
}

impl AgentMiddleware for SummarizationMiddleware {
    async fn modify_model_request(&self, ctx: &mut MiddlewareContext<'_>) -> anyhow::Result<()> {
        if ctx.request.messages.len() > self.messages_to_keep {
            let dropped = ctx.request.messages.len() - self.messages_to_keep;

            // Keep only the most recent N messages
            let mut truncated = ctx.request.messages
                .split_off(ctx.request.messages.len() - self.messages_to_keep);

            // Insert summary note at the beginning
            truncated.insert(0, AgentMessage {
                role: MessageRole::System,
                content: MessageContent::Text(format!(
                    "{} ({} earlier messages summarized)",
                    self.summary_note, dropped
                )),
                metadata: None,
            });

            ctx.request.messages = truncated;
        }
        Ok(())
    }
}
```

**Usage:**
```rust
let agent = ConfigurableAgentBuilder::new("You are a helpful assistant")
    .with_model(model)
    .with_summarization(SummarizationConfig::new(
        10,  // Keep last 10 messages
        "Earlier conversation history has been summarized for brevity."
    ))
    .build()?;
```

**Characteristics:**
- **Simple** - Only ~30 lines of code
- **Predictable** - Always keeps exactly N messages
- **Fast** - No LLM calls needed
- ️ **Lossy** - Dropped messages gone forever
- ️ **No semantic awareness** - Might drop important context

**When to Use:**
- Short-lived agents with limited interaction depth
- Agents with simple, linear conversation flows
- When speed is more important than perfect context

---

## Strategy 2: Anthropic Prompt Caching (rust-deep-agents-sdk)

### Approach: Cache Static Prompts to Reduce Processing

**Implementation:**
```rust
pub struct AnthropicPromptCachingMiddleware {
    pub ttl: String,  // e.g., "5m" for 5 minutes
    pub unsupported_model_behavior: String,
}

impl AgentMiddleware for AnthropicPromptCachingMiddleware {
    async fn modify_model_request(&self, ctx: &mut MiddlewareContext<'_>) -> anyhow::Result<()> {
        // Skip if TTL is zero
        if self.ttl == "0" {
            return Ok(());
        }

        // Convert system prompt to a cached system message
        if !ctx.request.system_prompt.is_empty() {
            let system_message = AgentMessage {
                role: MessageRole::System,
                content: MessageContent::Text(ctx.request.system_prompt.clone()),
                metadata: Some(MessageMetadata {
                    tool_call_id: None,
                    cache_control: Some(CacheControl {
                        cache_type: "ephemeral".to_string(),
                    }),
                }),
            };

            // Insert at beginning, clear original system prompt
            ctx.request.messages.insert(0, system_message);
            ctx.request.system_prompt.clear();
        }

        Ok(())
    }
}
```

**Cache Control in Messages:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub tool_call_id: Option<String>,
    pub cache_control: Option<CacheControl>,  // ← Caching directive
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: String,  // "ephemeral" for Anthropic
}
```

**Usage:**
```rust
let agent = ConfigurableAgentBuilder::new(instructions)
    .with_model(anthropic_model)
    .with_prompt_caching(true)  // Enable caching
    .build()?;
```

**Characteristics:**
- **Performance Boost** - Anthropic caches large prompts (system, tools)
- **Cost Reduction** - Cached tokens are 90% cheaper
- **No Code Changes** - Transparent to agent logic
- ️ **Provider-Specific** - Only works with Anthropic Claude
- ️ **TTL Limited** - Cache expires (typically 5 minutes)

**When to Use:**
- Using Anthropic Claude models
- Large system prompts (tools, instructions)
- Repeated calls with same base prompt
- Cost optimization is important

---

## Strategy 3: Multi-Memory System (ccswarm)

### Approach: Cognitive Architecture with Memory Consolidation

**Memory Types:**
```rust
pub struct SessionMemory {
    pub working_memory: WorkingMemory,      // Immediate (7±2 items)
    pub episodic_memory: EpisodicMemory,    // Experiences (1000 episodes)
    pub semantic_memory: SemanticMemory,    // Concepts & knowledge
    pub procedural_memory: ProceduralMemory, // Skills & procedures
}
```

#### Working Memory (Immediate Context)

Based on Miller's Law: 7±2 items

```rust
const WORKING_MEMORY_CAPACITY: usize = 7;

pub struct WorkingMemory {
    pub current_items: VecDeque<WorkingMemoryItem>,
    pub capacity: usize,
    pub active_task_context: Option<TaskContext>,
    pub attention_focus: Vec<String>,
    pub cognitive_load: f32,  // 0.0-1.0
}

impl WorkingMemory {
    fn add_item(&mut self, item: WorkingMemoryItem) {
        // FIFO eviction when at capacity
        if self.current_items.len() >= self.capacity {
            self.current_items.pop_front();
        }
        self.current_items.push_back(item);
    }

    fn cleanup_expired_items(&mut self) {
        let now = Utc::now();
        self.current_items.retain(|item| {
            let age = now - item.created_at;
            let decay_threshold = item.decay_rate * age.num_minutes() as f32;
            decay_threshold < 1.0  // Keep if not fully decayed
        });
    }
}

pub struct WorkingMemoryItem {
    pub content: String,
    pub item_type: WorkingMemoryType,
    pub priority: f32,
    pub decay_rate: f32,  // How quickly item becomes irrelevant
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
}
```

#### Memory Consolidation

```rust
impl SessionMemory {
    pub fn consolidate_memories(&mut self) {
        // Transfer important items from working → long-term
        let items_to_consolidate: Vec<_> = self.working_memory
            .current_items
            .iter()
            .filter(|item| self.should_consolidate_item(item))
            .cloned()
            .collect();

        for item in items_to_consolidate {
            match &item.item_type {
                WorkingMemoryType::TaskInstructions => {
                    // → Procedural memory (patterns)
                    self.procedural_memory.skill_patterns.push(/* ... */);
                }
                WorkingMemoryType::IntermediateResult => {
                    // → Semantic memory (facts)
                    self.semantic_memory.fact_base.push(/* ... */);
                }
                WorkingMemoryType::ErrorMessage => {
                    // → Episodic memory (experiences)
                    self.episodic_memory.add_episode(/* ... */);
                }
            }
        }

        // Clean up working memory
        self.working_memory.cleanup_expired_items();
    }

    fn should_consolidate_item(&self, item: &WorkingMemoryItem) -> bool {
        let age = Utc::now() - item.created_at;
        let significance = item.priority;

        // Consolidate if old enough AND significant
        age.num_minutes() > 30 && significance > 0.7
    }
}
```

#### Memory Retrieval

```rust
pub fn retrieve_relevant_memories(&self, query: &str) -> RetrievalResult {
    let mut result = RetrievalResult::new();

    // Search working memory (immediate context)
    for item in &self.working_memory.current_items {
        if item.content.to_lowercase().contains(&query.to_lowercase()) {
            result.working_memory_items.push(item.clone());
        }
    }

    // Search episodic memory (past experiences)
    for episode in &self.episodic_memory.episodes {
        if episode.description.to_lowercase().contains(&query.to_lowercase()) {
            result.relevant_episodes.push(episode.clone());
        }
    }

    // Search semantic memory (concepts)
    for concept in self.semantic_memory.concepts.values() {
        if concept.name.to_lowercase().contains(&query.to_lowercase()) {
            result.relevant_concepts.push(concept.clone());
        }
    }

    // Search procedural memory (skills)
    for procedure in self.procedural_memory.procedures.values() {
        if procedure.name.to_lowercase().contains(&query.to_lowercase()) {
            result.relevant_procedures.push(procedure.clone());
        }
    }

    return result;
}
```

**Characteristics:**
- **Cognitively Grounded** - Based on human memory research
- **Sophisticated** - Different memory types for different purposes
- **Semantic Retrieval** - Can query past experiences
- ️ **Complex** - Requires significant implementation
- ️ **Storage Overhead** - Maintains large historical dataset

**When to Use:**
- Long-running agents with extended lifespans
- Agents that learn from experience
- Multi-session continuity required
- Rich historical context needed

---

## Strategy 4: Session Compression & Reuse (ccswarm)

### Approach: Intelligent Session Management

**Configuration:**
```rust
pub struct OptimizedSessionConfig {
    pub max_sessions_per_role: usize,      // Default: 5
    pub idle_timeout: Duration,             // Default: 300s
    pub enable_compression: bool,           // Default: true
    pub compression_threshold: f64,         // Default: 0.8
    pub reuse_strategy: ReuseStrategy,
    pub performance_settings: PerformanceSettings,
}

pub enum ReuseStrategy {
    Aggressive,           // Always reuse
    LoadBased { threshold: f64 },
    TimeBased { max_age: Duration },
    Hybrid { load_threshold: f64, max_age: Duration },
}

pub struct PerformanceSettings {
    pub batch_operations: bool,
    pub batch_size: usize,
    pub context_caching: bool,  // ← Context caching flag
}
```

**Session Metrics:**
```rust
pub struct SessionMetadata {
    pub created_at: DateTime<Utc>,
    pub last_used: DateTime<Utc>,
    pub compression_enabled: bool,
    pub compression_ratio: f64,  // How much context was compressed
    pub context_size: usize,     // Current context window size
}

pub struct SessionMetrics {
    pub total_operations: usize,
    pub token_savings: usize,  // From compression
    pub average_response_time: Duration,
}
```

**Session Reuse Logic:**
```rust
async fn is_session_reusable(&self, session: &OptimizedSession) -> Result<bool> {
    match &self.config.reuse_strategy {
        ReuseStrategy::Aggressive => Ok(true),

        ReuseStrategy::LoadBased { threshold } => {
            let load = calculate_session_load(&session.metrics);
            Ok(load < *threshold)  // Reuse if under load threshold
        }

        ReuseStrategy::TimeBased { max_age } => {
            let age = Utc::now() - session.metadata.created_at;
            Ok(age < *max_age)  // Reuse if not too old
        }

        ReuseStrategy::Hybrid { load_threshold, max_age } => {
            let load = calculate_session_load(&session.metrics);
            let age = Utc::now() - session.metadata.created_at;
            Ok(load < *load_threshold && age < *max_age)
        }
    }
}
```

**Characteristics:**
- **Resource Efficient** - Reuses sessions instead of creating new
- **Adaptive** - Different strategies for different use cases
- **Metrics-Driven** - Tracks compression savings and performance
- ️ **Session-Focused** - About session lifecycle, not message history
- ️ **Compression Stub** - Compression flag exists but implementation unclear

**When to Use:**
- Multi-agent systems with many concurrent agents
- Resource-constrained environments
- Cost optimization important
- Session pooling beneficial

---

## Strategy 5: No Management (rust-agentai)

### Approach: Unbounded History

**Implementation:**
```rust
pub struct Agent {
    client: Client,
    history: Vec<ChatMessage>,  // ← Grows unbounded!
}

impl Agent {
    pub async fn run(&mut self, model: &str, prompt: &str, toolbox: Option<&dyn ToolBox>) -> Result<D> {
        // Add to history
        self.history.push(ChatMessage::user(prompt));

        // Send ENTIRE history to LLM every time
        let chat_req = ChatRequest::new(self.history.clone());
        let chat_resp = self.client.exec_chat(model, chat_req, Some(&chat_opts)).await?;

        // Add response to history
        self.history.push(ChatMessage::assistant(response));

        Ok(response)
    }
}
```

**Characteristics:**
- **Simple** - Zero complexity
- **Perfect Memory** - Never loses context
- ️ **Will Crash** - Eventually hits context limit
- ️ **Expensive** - Sends entire history every request
- ️ **Slow** - Large contexts increase latency

**Current Status:**
```rust
// TODO comments in rust-agentai source:
// TODO: Create new history trait
// This will allow configuring behaviour of messages. When doing multi-agent
// approach we could decide what history is being used, should we save all messages etc.
```

**When to Use:**
- Prototyping and development
- Short-lived agents (single task)
- Small expected message counts (< 50)
- **NOT for production**

---

## Comparative Analysis

| Strategy | Complexity | Memory | Performance | Semantic Awareness | Best For |
|----------|------------|--------|-------------|-------------------|----------|
| **Summarization** | Low | O(N) fixed | Fast | None | Simple agents |
| **Prompt Caching** | Low | O(N) | Very Fast | None | Cost optimization |
| **Multi-Memory** | High | O(N log N) | Medium | High | Long-running agents |
| **Session Reuse** | Medium | O(1) per session | Fast | None | Multi-agent systems |
| **Unbounded** | None | O(N) unbounded | Slow | Perfect | Development only |

---

## Additional Techniques Found

### 1. State Offloading (rust-deep-agents-sdk)

Instead of keeping state in context, store in structured state:

```rust
pub struct AgentStateSnapshot {
    pub todos: Vec<TodoItem>,
    pub files: BTreeMap<String, String>,
    pub scratchpad: BTreeMap<String, serde_json::Value>,
}

// Instead of:
// "I created file1.txt, file2.txt, and file3.txt..."
// (in message history - takes tokens)

// Do this:
// state.files.insert("file1.txt", content);
// (in structured state - no tokens)
```

**Benefit:** Reduces context bloat from accumulated state

### 2. SubAgent Delegation (rust-deep-agents-sdk)

Offload complex sub-tasks to ephemeral agents:

```rust
// Instead of:
// Main agent: "Let me research X..."
// Main agent: "Now let me research Y..."
// Main agent: "Now analyzing..."
// (100+ messages in main agent's history)

// Do this:
// Main agent calls task tool → SubAgent researches X → Returns result
// Main agent calls task tool → SubAgent researches Y → Returns result
// Main agent analyzes (only 2 messages in history)

const TASK_TOOL_DESCRIPTION: &str = r#"
Launch an ephemeral subagent to handle complex, multi-step independent tasks
with isolated context windows.

When to use:
- Complex and multi-step tasks that can be fully delegated
- Tasks requiring heavy token/context usage that would bloat the main thread
- Parallel execution of independent work
"#;
```

**Benefit:** Each subagent has fresh context window, main agent stays lean

### 3. Attention Focus (ccswarm)

Track what's currently important:

```rust
pub struct WorkingMemory {
    pub attention_focus: Vec<String>,  // What agent is focusing on
    pub cognitive_load: f32,           // 0.0-1.0 utilization
}

// Update cognitive load based on working memory
fn update_cognitive_load(&mut self) {
    let utilization = self.current_items.len() as f32 / WORKING_MEMORY_CAPACITY as f32;
    self.cognitive_load = utilization.min(1.0);
}
```

**Benefit:** Can prioritize what stays in context based on current focus

### 4. Decay-Based Eviction (ccswarm)

Items become less relevant over time:

```rust
pub struct WorkingMemoryItem {
    pub decay_rate: f32,        // How fast item loses relevance
    pub created_at: DateTime<Utc>,
}

fn cleanup_expired_items(&mut self) {
    let now = Utc::now();
    self.current_items.retain(|item| {
        let age = now - item.created_at;
        let decay_threshold = item.decay_rate * age.num_minutes() as f32;
        decay_threshold < 1.0  // Keep if not fully decayed
    });
}
```

**Benefit:** Natural context pruning based on relevance decay

---

## Recommendations for Spacedrive

### Recommended Hybrid Strategy

Combine multiple approaches for optimal results:

```rust
pub struct SpacedriveAgentContext {
    // Strategy 1: State Offloading
    memory: AgentMemory,  // Don't put state in messages

    // Strategy 2: Prompt Caching
    enable_prompt_caching: bool,  // For Anthropic users

    // Strategy 3: Message Limit with Smart Summarization
    message_window: MessageWindow,

    // Strategy 4: SubAgent Delegation
    subagent_registry: SubAgentRegistry,
}

pub struct MessageWindow {
    max_messages: usize,          // Default: 20
    always_keep: usize,           // Always keep N most recent (default: 5)
    consolidation_trigger: usize, // Consolidate when exceeds (default: 30)
}
```

### Implementation Design

#### Phase 1: Simple Truncation (Week 1)

```rust
impl AgentContext<M> {
    async fn prepare_llm_request(&self, new_message: &str) -> ModelRequest {
        let mut messages = self.history.read().await.clone();
        messages.push(AgentMessage::user(new_message));

        // Simple truncation - keep last 20
        if messages.len() > 20 {
            let kept = messages.split_off(messages.len() - 20);
            messages = vec![
                AgentMessage::system("Prior conversation summarized for brevity"),
            ];
            messages.extend(kept);
        }

        ModelRequest {
            system_prompt: self.base_instructions.clone(),
            messages,
        }
    }
}
```

**Pros:** Ship fast, avoid crashes
**Cons:** Loses context

#### Phase 2: State-Based Memory (Weeks 2-3)

```rust
// Don't put state in messages - use structured memory
impl AgentContext<M> {
    async fn handle_event(&self, event: VdfsEvent) {
        // Instead of adding "I detected 5 faces" to message history...
        // Store in structured memory:
        self.memory().update(|mut m| {
            m.history.append(PhotoEvent::PhotoAnalyzed {
                photo_id: event.entry.id(),
                faces_detected: 5,
                timestamp: Utc::now(),
            })?;
            Ok(m)
        }).await?;
    }
}
```

**Benefit:** Messages stay concise, state in structured storage

#### Phase 3: Anthropic Caching (Week 4)

```rust
// If using Anthropic, enable caching
let agent = AgentBuilder::new(instructions)
    .with_model(anthropic_model)
    .with_cache_control(CacheControl::ephemeral())
    .build()?;

// System prompt, tool schemas get cached
// 90% cost reduction on repeated requests
```

**Benefit:** Massive cost savings for Anthropic users

#### Phase 4: Intelligent Consolidation (Weeks 5-6)

```rust
impl AgentMemory {
    async fn consolidate_if_needed(&mut self) -> Result<()> {
        if self.should_consolidate() {
            // Extract key facts from working memory
            let facts = self.extract_important_facts().await?;

            // Move to associative memory
            for fact in facts {
                self.knowledge.add(fact).await?;
            }

            // Clear working memory
            self.plan.reset().await?;
        }
        Ok(())
    }

    fn should_consolidate(&self) -> bool {
        // Consolidate if:
        // - Working memory is large (> 100 items)
        // - Last consolidation was > 1 hour ago
        // - Cognitive load is high
        self.plan.size() > 100
            || self.last_consolidation.elapsed() > Duration::from_secs(3600)
    }
}
```

**Benefit:** Agents can "remember" important facts without bloating context

---

## Context Window Sizes by Provider

| Provider | Model | Context Window | Notes |
|----------|-------|----------------|-------|
| **Anthropic** | Claude 3.5 Sonnet | 200k tokens | Prompt caching available |
| **Anthropic** | Claude 3.5 Haiku | 200k tokens | Fastest, cheapest |
| **OpenAI** | GPT-4o | 128k tokens | No built-in caching |
| **OpenAI** | GPT-4o-mini | 128k tokens | Cheaper but less capable |
| **Gemini** | Gemini 1.5 Pro | 2M tokens | Largest context (overkill?) |
| **Local** | Llama 3.1 8B | 128k tokens | Via Ollama |
| **Local** | Qwen 2.5 | 32k tokens | Smaller but fast |

**Implications for Spacedrive:**
- **200k tokens** ≈ 150,000 words ≈ 300 pages of text
- Most agents won't need aggressive management
- BUT: Long-running agents processing thousands of files could hit limits
- **Strategy:** Optimize for local models (32k-128k), bonus for cloud (200k+)

---

## Recommended Implementation for Spacedrive

### Core Design Principles

1. **State Not in Context** - Use memory systems, not message history
2. **Event-Driven** - Agents don't "remember" every event, they process and store facts
3. **Lazy Loading** - Load relevant memories when needed, not all at once
4. **Configurable** - Per-extension settings for message limits

### Proposed Architecture

```rust
pub struct AgentContextManager {
    // Message history for LLM (limited)
    message_history: MessageHistory,

    // Structured memory (unlimited)
    memory: AgentMemory,

    // Caching strategy
    cache_strategy: CacheStrategy,
}

pub struct MessageHistory {
    messages: VecDeque<AgentMessage>,
    max_messages: usize,  // Default: 20
    always_keep_recent: usize,  // Always keep last N (default: 5)
}

impl MessageHistory {
    async fn prepare_for_llm(&self, memory: &AgentMemory) -> Vec<AgentMessage> {
        let mut messages = Vec::new();

        // Always include system prompt
        messages.push(AgentMessage::system(self.base_prompt.clone()));

        // Add memory summary if relevant
        if let Some(context) = memory.retrieve_relevant_context().await? {
            messages.push(AgentMessage::system(format!(
                "Relevant context from memory:\n{}",
                context
            )));
        }

        // Add recent message history
        messages.extend(self.messages.iter()
            .rev()
            .take(self.max_messages)
            .rev()
            .cloned());

        messages
    }
}

pub enum CacheStrategy {
    None,
    Anthropic { ttl: String },
    Custom { implementation: Box<dyn CacheProvider> },
}
```

### Extension Configuration

```toml
[extension.agent.context]
max_messages = 20           # Message history limit
always_keep_recent = 5      # Never truncate last N
consolidation_interval = 3600  # Seconds between memory consolidation
enable_prompt_caching = true   # If using Anthropic
```

### Usage in Photos Extension

```rust
impl PhotosAgent {
    #[on_event(EntryCreated)]
    async fn on_photo(&self, entry: Entry, ctx: &AgentContext<PhotosMind>) -> Result<()> {
        // DON'T add to message history
        // Instead: Add to structured memory
        ctx.memory().history.append(PhotoEvent::PhotoAnalyzed {
            photo_id: entry.id(),
            faces_detected: 5,
            timestamp: Utc::now(),
        }).await?;

        // When agent needs to use LLM (rare):
        if self.needs_llm_decision(ctx).await? {
            // Context manager automatically:
            // 1. Loads relevant memories (last 100 face detections)
            // 2. Prepares concise summary
            // 3. Includes in LLM request
            // 4. Keeps message history small

            let response = ctx.llm()
                .query("Should I create a moment for these photos?")
                .with_memory_context(/* auto-loaded */)
                .execute()
                .await?;
        }

        Ok(())
    }
}
```

---

## Key Insights

### 1. **Most Agents Don't Need Conversations**

Spacedrive's agents are primarily event-driven processors, not chatbots:

```rust
// Photos agent processes 10,000 photos
// - Does NOT need to "remember" each one in message history
// - DOES need to accumulate knowledge (face clusters, places)
// - Solution: Structured memory, not message history
```

### 2. **Separate Concerns**

**Message History** (for LLM context):
- Tool calling conversations
- Recent user interactions
- Current task reasoning
- **Keep minimal** (< 20 messages)

**Agent Memory** (for knowledge):
- Historical events (all face detections)
- Learned patterns (face clusters)
- Working state (pending queue)
- **Can be large** (GBs)

### 3. **Anthropic's Prompt Caching is Powerful**

For agents with large tool schemas or system prompts:
- **First request**: Full context processed (~$0.015/1M tokens)
- **Cached requests**: Only new messages processed (~$0.0015/1M tokens)
- **Savings**: 90% on repeated requests
- **TTL**: 5 minutes (enough for interactive sessions)

### 4. **SubAgent Pattern is Underrated**

From rust-deep-agents-sdk's design:

```
Main Agent (small context)
   ↓
   ├─> SubAgent: Research Face Detection Models (isolated context)
   ├─> SubAgent: Analyze GPS Clustering (isolated context)
   └─> SubAgent: Generate Moment Titles (isolated context)
   ↓
Main Agent receives only results (not intermediate reasoning)
```

**Result:** Main agent's context stays clean

---

## Implementation Recommendations for Spacedrive

### Immediate (Phase 1 - Ship with Extension System)

```rust
pub struct AgentContextConfig {
    // Simple truncation
    pub max_message_history: usize,  // Default: 20

    // State offloading (already in design)
    pub memory_enabled: bool,  // Default: true
}

impl AgentContext<M> {
    async fn prepare_llm_messages(&self) -> Vec<Message> {
        let mut messages = Vec::new();

        // System prompt (will be cached if Anthropic)
        messages.push(Message::system(self.instructions.clone()));

        // Recent message history (limited)
        let history = self.history.read().await;
        messages.extend(history.iter()
            .rev()
            .take(self.config.max_message_history)
            .rev()
            .cloned());

        messages
    }
}
```

### Near-Term (Phase 2 - After Memory System Works)

```rust
impl AgentContext<M> {
    async fn prepare_llm_messages_with_memory(&self) -> Vec<Message> {
        let mut messages = Vec::new();

        // Base system prompt
        messages.push(Message::system(self.instructions.clone()));

        // Memory-derived context
        let memory_context = self.memory()
            .retrieve_relevant_facts(/* current task */)
            .await?;

        if !memory_context.is_empty() {
            messages.push(Message::system(format!(
                "Relevant information from your memory:\n{}",
                memory_context.join("\n")
            )));
        }

        // Recent messages
        messages.extend(self.recent_messages(20));

        messages
    }
}
```

### Future (Phase 3 - Advanced)

```rust
pub enum ContextStrategy {
    // Simple truncation
    SimpleWindow { max_messages: usize },

    // Summarization (requires LLM call)
    LlmSummarization {
        max_messages: usize,
        summarize_after: usize,
    },

    // Memory-based (use structured memory)
    MemoryBased {
        max_messages: usize,
        memory_context_size: usize,
    },

    // Hybrid
    Hybrid {
        window: usize,
        memory_enabled: bool,
        caching_enabled: bool,
    },
}
```

---

## Specific to Spacedrive: Event-Driven Context

Unique challenge: Agents receive thousands of events (every file indexed).

### Problem

```rust
// If agent kept message history for each event:
// EntryCreated(photo1.jpg)
// EntryCreated(photo2.jpg)
// ...
// EntryCreated(photo10000.jpg)
// → 10,000 messages in history → CRASH
```

### Solution: Event Aggregation

```rust
impl AgentContext<M> {
    async fn handle_event(&self, event: VdfsEvent) {
        // Don't add to message history!
        // Add to structured memory instead

        match event {
            VdfsEvent::EntryCreated { entry, .. } => {
                self.memory().history.append(PhotoEvent::PhotoAnalyzed {
                    photo_id: entry.id(),
                    // ... extracted facts
                }).await?;

                // NO message history update
            }
        }

        // Only use LLM when making decisions
        // Not when processing events
    }

    async fn make_decision(&self, question: &str) -> Result<Decision> {
        // NOW we need LLM - prepare context
        let context = self.memory()
            .summarize_recent_activity()  // "Analyzed 100 photos, found 25 people"
            .await?;

        let response = self.llm()
            .query(question)
            .with_context(context)
            .execute()
            .await?;

        Ok(response)
    }
}
```

**Key Principle:** Events → Memory (structured), Decisions → LLM (with context)

---

## Performance Benchmarks (from rust-deep-agents-sdk docs)

**Context Window Impact:**

| Messages | Context Size | Latency (GPT-4) | Cost per Request |
|----------|--------------|-----------------|------------------|
| 5 | ~2k tokens | 1.2s | $0.02 |
| 20 | ~8k tokens | 2.1s | $0.08 |
| 50 | ~20k tokens | 4.5s | $0.20 |
| 100 | ~40k tokens | 8.2s | $0.40 |
| 200 | ~80k tokens | 15.1s | $0.80 |

**With Anthropic Caching (cached system prompt):**

| Messages | First Request | Cached Request | Savings |
|----------|---------------|----------------|---------|
| 20 | $0.08 | $0.02 | 75% |
| 50 | $0.20 | $0.05 | 75% |
| 100 | $0.40 | $0.10 | 75% |

---

## Code Examples to Borrow

### From rust-deep-agents-sdk

**Summarization Middleware** (agents-runtime/src/middleware.rs:98-140)
- Clean middleware pattern
- Simple truncation with summary note
- ~40 lines, easy to adapt

**Prompt Caching Middleware** (agents-runtime/src/middleware.rs:428-490)
- Anthropic-specific optimization
- Metadata-based cache control
- ~60 lines, copy as-is

### From ccswarm

**Working Memory Pattern** (ccswarm/src/session/memory.rs:654-681)
- Miller's Law (7±2) capacity
- FIFO eviction when full
- Decay-based cleanup
- ~30 lines, interesting concept

**Memory Consolidation** (ccswarm/src/session/memory.rs:439-460)
- Transfer working → long-term
- Significance-based filtering
- Different storage per item type
- ~40 lines, good pattern for Spacedrive's three-memory system

### From rust-agentai

**Simple History Management** (agentai/src/agent.rs:36-72)
- Clean API surface
- Mutable history vector
- Educational example of what NOT to do long-term
- But good for Phase 1 (simple)

---

## Conclusion

### Recommended Approach for Spacedrive

**Phase 1 (Ship with Extension System):**
- Simple message truncation (keep last 20)
- State offloading to memory systems (already designed)
- No summarization yet

**Phase 2 (After Memory System Works):**
- Memory-derived context summaries
- Prompt caching for Anthropic users
- Consolidation triggers

**Phase 3 (Advanced):**
- LLM-based summarization (optional)
- Adaptive window sizing
- SubAgent delegation support

### Key Takeaway

**Spacedrive's event-driven architecture naturally avoids context bloat:**
- Events → Memory (structured storage)
- Memory → Context (when LLM needed)
- Not: Events → Messages → LLM

This is a unique advantage over chatbot-style agents that accumulate conversational history.

---

## Files to Reference

**Primary Reference:**
- `rust-deep-agents-sdk/crates/agents-runtime/src/middleware.rs` (lines 98-140, 428-490)
  - Summarization and caching patterns ready to adapt

**Additional Study:**
- `ccswarm/crates/ccswarm/src/session/memory.rs` (lines 1-760)
  - Multi-memory architecture concepts
- `ccswarm/crates/ccswarm/src/session/session_optimization.rs` (lines 1-555)
  - Session reuse and compression framework

**Anti-Pattern:**
- `rust-agentai/crates/agentai/src/agent.rs` (lines 36-72)
  - Shows unbounded history problem to avoid

