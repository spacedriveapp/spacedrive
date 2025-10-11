<!--CREATED: 2025-10-11-->
# API Module Design: Unified Entry Point & Permission Layer

## Problem Analysis

Your architectural insight is spot-on. The current system has several issues:

### **Current Issues:**
1. **Session handling scattered**: Library operations get `library_id` from multiple places
2. **No permission layer**: Operations execute without auth/permission checks
3. **Context confusion**: Session state should be parameter, not stored in CoreContext
4. **API entry points distributed**: Multiple handlers, no unified API surface

### **Your Vision:**
- **Session as parameter**: Operations receive session context explicitly
- **Unified API entry point**: Single place where applications call operations
- **Permission layer**: Auth and authorization happen at API boundary
- **Clean separation**: Core logic separate from API concerns

## Proposed `infra/api` Module Architecture

### **Module Structure**
```
core/src/infra/api/
├── mod.rs              // Public API exports
├── dispatcher.rs       // Unified operation dispatcher
├── session.rs          // Session context and management
├── permissions.rs      // Permission and authorization layer
├── context.rs          // API request context
├── middleware.rs       // API middleware pipeline
├── error.rs            // API-specific error types
└── types.rs            // API surface types
```

### **Core Components**

#### **1. Session Context (`session.rs`)**
```rust
/// Rich session context passed to operations
#[derive(Debug, Clone)]
pub struct SessionContext {
    /// User/device authentication info
    pub auth: AuthenticationInfo,

    /// Currently selected library (if any)
    pub current_library_id: Option<Uuid>,

    /// User preferences and permissions
    pub permissions: PermissionSet,

    /// Request metadata
    pub request_metadata: RequestMetadata,

    /// Device context
    pub device_id: Uuid,
    pub device_name: String,
}

#[derive(Debug, Clone)]
pub struct AuthenticationInfo {
    pub user_id: Option<Uuid>,           // Future: user authentication
    pub device_id: Uuid,                 // Device identity
    pub authentication_level: AuthLevel, // None, Device, User, Admin
}

#[derive(Debug, Clone)]
pub enum AuthLevel {
    None,           // Unauthenticated
    Device,         // Device-level access
    User(Uuid),     // User-level access
    Admin(Uuid),    // Admin-level access
}
```

#### **2. Unified Dispatcher (`dispatcher.rs`)**
```rust
/// The main API entry point - this is what applications call
pub struct ApiDispatcher {
    core_context: Arc<CoreContext>,
    permission_layer: PermissionLayer,
}

impl ApiDispatcher {
    /// Execute a library action with session context
    pub async fn execute_library_action<A>(
        &self,
        action_input: A::Input,
        session: SessionContext,
    ) -> Result<A::Output, ApiError>
    where
        A: LibraryAction + 'static,
    {
        // 1. Permission check
        self.permission_layer.check_library_action::<A>(&session).await?;

        // 2. Require library context
        let library_id = session.current_library_id
            .ok_or(ApiError::NoLibrarySelected)?;

        // 3. Create action
        let action = A::from_input(action_input)
            .map_err(ApiError::InvalidInput)?;

        // 4. Execute with enriched session context
        let manager = ActionManager::new(self.core_context.clone());
        let result = manager.dispatch_library_with_session(
            library_id,
            action,
            session
        ).await?;

        Ok(result)
    }

    /// Execute a core action with session context
    pub async fn execute_core_action<A>(
        &self,
        action_input: A::Input,
        session: SessionContext,
    ) -> Result<A::Output, ApiError>
    where
        A: CoreAction + 'static,
    {
        // 1. Permission check
        self.permission_layer.check_core_action::<A>(&session).await?;

        // 2. Create action
        let action = A::from_input(action_input)
            .map_err(ApiError::InvalidInput)?;

        // 3. Execute with session context
        let manager = ActionManager::new(self.core_context.clone());
        let result = manager.dispatch_core_with_session(action, session).await?;

        Ok(result)
    }

    /// Execute a library query with session context
    pub async fn execute_library_query<Q>(
        &self,
        query_input: Q::Input,
        session: SessionContext,
    ) -> Result<Q::Output, ApiError>
    where
        Q: LibraryQuery + 'static,
    {
        // 1. Permission check
        self.permission_layer.check_library_query::<Q>(&session).await?;

        // 2. Require library context
        let library_id = session.current_library_id
            .ok_or(ApiError::NoLibrarySelected)?;

        // 3. Create query
        let query = Q::from_input(query_input)
            .map_err(ApiError::InvalidInput)?;

        // 4. Execute with session context
        let result = query.execute(self.core_context.clone(), session, library_id).await?;

        Ok(result)
    }

    /// Execute a core query with session context
    pub async fn execute_core_query<Q>(
        &self,
        query_input: Q::Input,
        session: SessionContext,
    ) -> Result<Q::Output, ApiError>
    where
        Q: CoreQuery + 'static,
    {
        // Permission check
        self.permission_layer.check_core_query::<Q>(&session).await?;

        // Create and execute
        let query = Q::from_input(query_input).map_err(ApiError::InvalidInput)?;
        let result = query.execute(self.core_context.clone(), session).await?;

        Ok(result)
    }
}
```

#### **3. Permission Layer (`permissions.rs`)**
```rust
/// Permission checking for all operations
pub struct PermissionLayer {
    // Permission rules, policies, etc.
}

impl PermissionLayer {
    /// Check if session can execute library action
    pub async fn check_library_action<A: LibraryAction>(
        &self,
        session: &SessionContext,
    ) -> Result<(), PermissionError> {
        // Future: Check user permissions for this action
        // Future: Check library-specific permissions
        // Future: Rate limiting, quota checks

        match session.auth.authentication_level {
            AuthLevel::None => Err(PermissionError::Unauthenticated),
            AuthLevel::Device | AuthLevel::User(_) | AuthLevel::Admin(_) => {
                // Future: Fine-grained permission checks based on action type
                Ok(())
            }
        }
    }

    /// Check if session can execute core action
    pub async fn check_core_action<A: CoreAction>(
        &self,
        session: &SessionContext,
    ) -> Result<(), PermissionError> {
        // Core actions might need higher privileges
        match session.auth.authentication_level {
            AuthLevel::Admin(_) => Ok(()),
            _ => Err(PermissionError::InsufficientPrivileges),
        }
    }

    // Similar for queries...
}
```

#### **4. Updated Trait Signatures**
```rust
/// Updated LibraryQuery trait with session parameter
pub trait LibraryQuery: Send + 'static {
    type Input: Send + Sync + 'static;
    type Output: Send + Sync + 'static;

    fn from_input(input: Self::Input) -> Result<Self>;

    // NEW: Receives session context instead of just library_id
    async fn execute(
        self,
        context: Arc<CoreContext>,
        session: SessionContext,      // ← Rich session context
        library_id: Uuid,            // ← Still needed for library operations
    ) -> Result<Self::Output>;
}

/// Updated CoreQuery trait with session parameter
pub trait CoreQuery: Send + 'static {
    type Input: Send + Sync + 'static;
    type Output: Send + Sync + 'static;

    fn from_input(input: Self::Input) -> Result<Self>;

    // NEW: Receives session context
    async fn execute(
        self,
        context: Arc<CoreContext>,
        session: SessionContext,      // ← Rich session context
    ) -> Result<Self::Output>;
}
```

### **5. Application Integration Points**

#### **GraphQL Server Integration**
```rust
// In GraphQL resolvers
impl GraphQLQuery {
    async fn files_search(&self, input: FileSearchInput) -> Result<FileSearchOutput> {
        let session = self.extract_session_from_request()?;

        self.api_dispatcher
            .execute_library_query::<FileSearchQuery>(input, session)
            .await
    }
}
```

#### **CLI Integration**
```rust
// In CLI commands
impl CliCommand {
    async fn files_copy(&self, input: FileCopyInput) -> Result<JobReceipt> {
        let session = SessionContext::from_cli_context(&self.config)?;

        self.api_dispatcher
            .execute_library_action::<FileCopyAction>(input, session)
            .await
    }
}
```

#### **Swift Client Integration**
```rust
// In daemon connector
impl DaemonConnector {
    async fn execute_operation(&self, method: String, payload: Data) -> Result<Data> {
        let session = self.current_session()?;

        // Route to appropriate dispatcher method based on method string
        match method.as_str() {
            "action:files.copy.input.v1" => {
                let input: FileCopyInput = decode(payload)?;
                let result = self.api_dispatcher
                    .execute_library_action::<FileCopyAction>(input, session)
                    .await?;
                encode(result)
            }
            // ... other operations
        }
    }
}
```

## Benefits of This Design

### **1. Unified API Surface**
- **Single entry point**: All applications go through `ApiDispatcher`
- **Consistent interface**: Same pattern for all operation types
- **Clear boundaries**: API layer separate from core business logic

### **2. Proper Permission Layer**
- **Authentication**: Device/user/admin levels
- **Authorization**: Operation-specific permission checks
- **Future-ready**: Easy to add fine-grained permissions

### **3. Rich Session Context**
- **Not just library_id**: Full user/device/permission context
- **Request metadata**: Tracking, audit trails, rate limiting
- **Extensible**: Easy to add new session data

### **4. Clean Separation of Concerns**
- **API layer**: Authentication, authorization, routing
- **Core layer**: Business logic, unchanged
- **Operations**: Receive rich context, focus on execution

### **5. Future Extensibility**
- **Multiple auth providers**: Easy to add OAuth, SAML, etc.
- **Library-specific permissions**: Per-library access control
- **Audit trails**: Track all operations with session context
- **Rate limiting**: Per-user/device quotas

## Migration Path

1. **Create `infra/api` module** with base types
2. **Update trait signatures** to receive `SessionContext`
3. **Create `ApiDispatcher`** with permission layer
4. **Update applications** to use unified API
5. **Gradually enhance permissions** as needed

This design gives you a **clean, extensible API layer** that grows with your authentication and permission needs! 🎯

