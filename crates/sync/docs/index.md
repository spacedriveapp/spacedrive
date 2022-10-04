# Owned Records

Node which owns the record is the sole source of truth for that record's state

# Shared Records

This includes Shared Record O-M Relations, since the foreign key is stored on the Many record.

## Create

```json
{
	"type": "CREATE",
	"recordId": "{uuid}",
	"model": "{model}",
	"data": {
		"key": "value"
	},
	"node": "{uuid}",
	"timestamp": {
		"hybrid": "logical clock"
	}
}
```

## Update

```json
{
	"type": "UPDATE",
	"recordId": "{uuid}",
	"field": "{field}",
	"value": "{value}",
	"node": "{uuid}",
	"timestamp": {
		"hybrid": "logical clock"
	}
}
```

## Delete

```json
{
	"type": "DELETE",
	"recordId": "{uuid}",
	"node": "{uuid}",
	"timestamp": {
		"hybrid": "logical clock"
	}
}
```

# Shared Record M-M Relations

x-M relations usually signify an item belonging to a group. 1-M relations are handled normally by Shared Records since the ID of the record is just the ID of the M record. M-M relations require custom handling since they are identified by the two records they join, so 2 create instructions

UNANSWERED: M-M relations that _can_ be duplicated. In this case, a single ID for the relation would suffice, in the same way that 1-M relations do.

NOTE: Ordering is very important when syncing relations. If a target of a relation doesn't exist, what should happen? This presents two options:

1. Don't use a foreign key, just join/fetch separately on a possibly non-existent foreign id. This is pretty cringe since Prisma only affords the niceties of relations if foreign keys are actually used.
2. Require that all operations are synced in order, independent of which node they were created on. This is nicer since it means that in order for a node to create a relation in the first place it must possess a message indicating creation of the relation target, but it is much more difficult to coordinate deletion of messages this way. Probably still doable though.

Option 2 is probably the best way to go, since having to do annoying joins and losing database ergonomics is not great. Additionally, option 2 would result in the ability to sync shared data between any two nodes, even if the node being synced from didn't create the operation in the first place.

## Create

```json
{
	"type": "CREATE",
	// Record that is being assigned to a group eg. a file
	"relationItem": "{uuid}",
	// Group that the record is being assigned to eg. a photo album
	"relationGroup": "{uuid}",
	// Name of the model which represents the relation
	"relation": "model",
	"node": "{uuid}",
	"timestamp": {
		"hybrid": "logical clock"
	}
}
```

## Update

```json
{
	"type": "UPDATE",
	"relationItem": "{uuid}",
	"relationGroup": "{uuid}",
	"relation": "model",
	"field": "field",
	"value": "{value}",
	"node": "{uuid}",
	"timestamp": {
		"hybrid": "logical clock"
	}
}
```

## Delete

```json
{
	"type": "DELETE",
	"relationItem": "{uuid}",
	"relationGroup": "{uuid}",
	"relation": "relation",
	"node": "{uuid}",
	"timestamp": {
		"hybrid": "logical clock"
	}
}
```
