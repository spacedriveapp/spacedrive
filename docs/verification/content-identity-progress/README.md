# Content Identity progress verification

The browser renders the baseline JobRow and changed JobRow with the same running fixture. The baseline shows 100%; the changed row uses reported completion fields. The activity footer uses the actual JobProgressSummary and CircleButton components with fixture data.

`after.png` shows the known queue, discovery with an unknown total, missing progress, and the activity control. `stale-estimate.png` shows that an estimate becomes Estimating after 30 seconds without an update. These images do not prove a live daemon job or native app behavior. The unrelated Media filter and scroll list panels are outside this PR.

Three release-profile content_identity tests passed. They cover capped running progress, identical-byte hashes, and a file read failure that does not stop a batch. Production frontend build and TypeScript check passed.

The existing indicatif 0.17 dependency provides an adaptive rate and ETA. No new dependency or network data transfer is needed. ETA starts after ten seconds and only when discovery is complete. File sizes vary, so the estimate is approximate. The local repaired native app showed increasing counts and ETA in the job row and activity control. The two job-refresh tests verify that running and paused snapshots retain runtime progress, while terminal, new and removed jobs do not inherit it. Full native pause/resume checks remain. Native window capture can return a small background thumbnail; the public screenshots are controlled browser fixtures.
