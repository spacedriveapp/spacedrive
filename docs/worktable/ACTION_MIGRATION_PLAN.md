# Complete Action Migration Plan

**Date:** 2025-01-27
**Status:** **Comprehensive Migration Strategy**

## 📊 **Current Status Analysis:**

### **✅ COMPLETED (6 actions):**

- `LibraryCreateAction` → `CoreAction` ✅
- `LibraryDeleteAction` → `CoreAction` ✅
- `VolumeSpeedTestAction` → `CoreAction` ✅
- `VolumeTrackAction` → `LibraryAction` ✅
- `VolumeUntrackAction` → `LibraryAction` ✅
- `FileCopyAction` → `LibraryAction` ✅

### **🔄 PARTIALLY DONE (3 actions):**

- `LibraryRenameAction` → Still using old `ActionTrait` (needs update to `LibraryAction`)
- `LocationAddAction` → Still using old `ActionTrait` (needs update to `LibraryAction`)
- `FileDeleteAction` → Still using old `ActionTrait` (needs update to `LibraryAction`)

### **❌ NOT STARTED (11 actions):**

- `LibraryExportAction` → `LibraryAction` (exports specific library)
- `LocationRemoveAction` → `LibraryAction` (removes location from library)
- `LocationIndexAction` → `LibraryAction` (indexes location in library)
- `LocationRescanAction` → `LibraryAction` (rescans location in library)
- `FileValidateAction` → `LibraryAction` (validates files in library)
- `DuplicateDetectionAction` → `LibraryAction` (detects duplicates in library)
- `ThumbnailAction` → `LibraryAction` (generates thumbnails in library)
- `IndexingAction` → `LibraryAction` (indexes content in library)
- `MetadataAction` → `LibraryAction` (extracts metadata in library)
- `DeviceRevokeAction` → `LibraryAction` (revokes device from library)
- `ContentAction` → `LibraryAction` (analyzes content in library)

## 🎯 **Classification Strategy:**

### **CoreAction (Global Level - 3 total):**

- ✅ `LibraryCreateAction` - Creates libraries
- ✅ `LibraryDeleteAction` - Deletes libraries
- ✅ `VolumeSpeedTestAction` - Tests volumes globally

### **LibraryAction (Library-Scoped - 17 total):**

All other actions operate within library context and should be `LibraryAction`.

## 🚀 **Migration Plan:**

### **Phase 1: Fix Partially Done (3 actions)**

1. **LibraryRenameAction** - Update from `ActionTrait` to `LibraryAction`
2. **LocationAddAction** - Update from `ActionTrait` to `LibraryAction`
3. **FileDeleteAction** - Update from `ActionTrait` to `LibraryAction`

### **Phase 2: Port ActionHandler to LibraryAction (11 actions)**

For each action:

1. **Add library_id field** to action struct (if missing)
2. **Add constructor methods** for clean API
3. **Replace ActionHandler with LibraryAction** implementation
4. **Port business logic** from old execute method
5. **Add comprehensive validation** (without library existence boilerplate)
6. **Remove old handler struct and registration**

### **Phase 3: Return Type Strategy**

- **Domain object actions** → Return native output types (`VolumeTrackOutput`, etc.)
- **Job-dispatching actions** → Return `JobHandle` naturally
- **Simple operations** → Return native types or success indicators

## 💡 **Pattern Template:**

### **For LibraryAction:**

```rust
impl LibraryAction for SomeAction {
    type Output = SomeOutput; // or JobHandle for job actions

    async fn execute(self, library: Arc<Library>, context: Arc<CoreContext>) -> Result<Self::Output, ActionError> {
        // Library already validated - use directly!
        // Port business logic from old ActionHandler
    }

    fn action_kind(&self) -> &'static str { "some.action" }
    fn library_id(&self) -> Uuid { self.library_id }

    async fn validate(&self, library: &Arc<Library>, context: Arc<CoreContext>) -> Result<(), ActionError> {
        // No library existence boilerplate needed!
        // Add domain-specific validation
    }
}
```

## 🎯 **Estimated Work:**

- **Phase 1**: ~30 minutes (3 simple updates)
- **Phase 2**: ~2 hours (11 ActionHandler ports)
- **Phase 3**: ~30 minutes (cleanup and testing)

**Total: ~3 hours to complete all 20 actions**

The architecture is proven perfect - this is just systematic implementation work!
