//! ActionHandler trait removed - using unified ActionTrait instead
//!
//! All actions now implement ActionTrait directly for unified dispatch
//! through ActionManager.dispatch<A: ActionTrait>()