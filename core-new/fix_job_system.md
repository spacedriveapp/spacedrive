# Job System Issues Fix Summary

## Issues Identified

1. **Duplicate Job Creation**: Two indexer jobs are created for one location add
2. **Progress Always Shows 0%**: Jobs complete but progress remains at 0%  
3. **Status Shows "Queued"**: Completed jobs show as "Queued" instead of "Completed"
4. **Non-linear Progress**: Progress jumps around instead of increasing smoothly

## Root Causes

### 1. Duplicate Jobs
- The CLI might be sending duplicate requests
- Or there's a race condition in job creation
- Need to add deduplication logic

### 2. Progress Not Updating
- Progress forwarding is working in JobManager
- But jobs might be completing too fast for progress to be captured
- Need to ensure final progress is always saved

### 3. Status Issues  
- Jobs complete quickly but status might not be persisted properly
- Need better status synchronization

### 4. Non-linear Progress
- Already fixed by implementing phase-based progress ranges
- Discovery: 5-10%, Processing: 20-60%, Content: 70-98%, Finalizing: 99%

## Fixes Implemented

1. **Progress Persistence**: Added batched progress updates to database
2. **Status Synchronization**: Final progress saved with status updates  
3. **Unified Query System**: Combined memory and database queries
4. **Progress Calculation**: Fixed phase-based progress calculations

## Additional Fixes Needed

1. **Deduplication**: Add request ID or check for existing jobs before creating new ones
2. **Minimum Job Duration**: Ensure jobs run long enough to be monitored
3. **Better Error Handling**: Log all job state transitions

## Testing

After applying fixes:
- Run `sd location add` and check for duplicate jobs with `sd job list`
- Monitor with `sd job monitor` to see real-time progress
- Verify completed jobs show 100% and "Completed" status