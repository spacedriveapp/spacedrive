# GraphQL API with async-graphql

Spacedrive's new API uses GraphQL with full type safety from Rust to TypeScript.

## Type Safety Comparison

### rspc (Old Approach)
```rust
// Backend
rspc::router! {
    pub async fn create_library(name: String) -> Result<Library> {
        // implementation
    }
}
```

```typescript
// Frontend - custom generated types
const library = await client.mutation(['create_library', name]);
```

### async-graphql (New Approach)
```rust
// Backend
#[Object]
impl Mutation {
    async fn create_library(&self, input: CreateLibraryInput) -> Result<LibraryType> {
        // implementation
    }
}
```

```typescript
// Frontend - standard GraphQL with full types
const { data } = await createLibrary({
  variables: { input: { name: "My Library" } }
});
```

## Advantages of async-graphql

### 1. **Better Tooling**
- GraphQL Playground for API exploration
- Apollo DevTools for debugging
- VSCode extensions with autocomplete
- Postman/Insomnia support out of the box

### 2. **Flexible Queries**
```graphql
# Frontend can request exactly what it needs
query GetLibrary($id: UUID!) {
  library(id: $id) {
    name
    # Only fetch heavy statistics if needed
    statistics {
      totalFiles
      totalSize
    }
  }
}
```

### 3. **Built-in Features**
- Field-level permissions
- Automatic N+1 query prevention with DataLoader
- Built-in introspection
- Subscriptions for real-time updates

### 4. **Type Generation**
```bash
# Simple command generates all TypeScript types
npm run graphql-codegen

# Generates:
# - Types for all queries/mutations
# - React hooks
# - Full TypeScript interfaces
```

### 5. **Better Error Handling**
```graphql
mutation CreateLibrary($input: CreateLibraryInput!) {
  createLibrary(input: $input) {
    ... on Library {
      id
      name
    }
    ... on LibraryError {
      code
      message
      field
    }
  }
}
```

## Migration Benefits

| Feature | rspc | async-graphql |
|---------|------|---------------|
| **Type Safety** | Custom | Industry Standard |
| **Tooling** | Limited | Extensive |
| **Community** | Abandoned | Active |
| **Learning Curve** | Custom API | Standard GraphQL |
| **Code Generation** | Custom | graphql-codegen |
| **Real-time** | Custom | Subscriptions |
| **File Upload** | Custom | Multipart spec |
| **Caching** | Manual | Apollo Cache |

## Example: Full Type Safety Flow

### 1. Define in Rust
```rust
#[derive(SimpleObject)]
struct LibraryType {
    id: Uuid,
    name: String,
    #[graphql(deprecation = "Use statistics.totalFiles")]
    file_count: i64,
}
```

### 2. Auto-generated TypeScript
```typescript
export interface Library {
  id: string;
  name: string;
  /** @deprecated Use statistics.totalFiles */
  fileCount: number;
}
```

### 3. Use in Frontend
```typescript
// Full autocomplete and type checking
const { data } = useGetLibraryQuery({ 
  variables: { id: libraryId } 
});

// TypeScript knows exactly what fields are available
console.log(data.library.name); // ✅
console.log(data.library.invalid); // Type error!
```

## Performance Benefits

### Batching & Caching
```typescript
// Apollo Client automatically batches and caches
const MultipleLibraryComponent = () => {
  // These are automatically batched into one request
  const lib1 = useGetLibraryQuery({ variables: { id: id1 } });
  const lib2 = useGetLibraryQuery({ variables: { id: id2 } });
  const lib3 = useGetLibraryQuery({ variables: { id: id3 } });
};
```

### Optimistic Updates
```typescript
const [createLibrary] = useCreateLibraryMutation({
  optimisticResponse: {
    createLibrary: {
      id: 'temp-id',
      name: input.name,
      __typename: 'Library'
    }
  },
  update: (cache, { data }) => {
    // UI updates immediately, rolls back on error
  }
});
```

## Conclusion

While rspc provided type safety, async-graphql gives us:
- **Industry standard** that developers already know
- **Better tooling** and ecosystem
- **Active maintenance** and updates
- **More features** out of the box
- **Same level of type safety** with better DX

The migration from rspc to GraphQL modernizes the API while maintaining the type safety that Spacedrive requires.