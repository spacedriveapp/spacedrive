Currently the indexer will run on a location when it is added, populating the database. The location watcher will run on startup and watch for OS events to atomically update the index. The user can also explicitly re-index a location or path of a location at anytime. However this is not ideal, since Spacedrive going offline for a period would mean it to be impossible to know about changes within a location during that period.

One method to solve this would be to detect offline periods and mark locations as stale, triggering a reindex, or just dispatch those re-indexing jobs upon detection of an offline period. This however would be pretty intensive for users with lots of large locations should Spacedrive go offline. I believe locations should have a last index timestamp at least.

I would like your thoughts and potential ideas that factor in performance to keep the Spacedrive index as up-to-date as possible at all times.
