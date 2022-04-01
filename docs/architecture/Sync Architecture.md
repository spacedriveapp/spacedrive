# SQLite Sync Architecture
- Files and directories partitioned by client that owns them
- Global state modified using Operational Transforms-like system
	- Transforms defined at the property level
	- Transforms can have different priority
	- eg. Tags that can be created and modified, but deleting takes precedence
- All clients store a list of other clients and a list of pending changes to be synced to each client
- New clients need to retrieve entire database from any client
	- Includes any pending changes from said database
	- Thus will be in the same state as the database it synced from
	- Needs to ask other databases for changes and identify itself


## Global State


## File State