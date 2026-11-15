# To do

Open items from the code review of 0.3.0, most useful first. Remove an item
in the same commit that does it.

## Check in REAPER

The refactoring after 0.3.0 is unit-tested, but the REAPER side was not run
yet. Before the next release go through [manual-tests.md](manual-tests.md),
in particular:

- a pad button bound to one of reaper-pad's own actions (was broken in 0.3.0),
- the new `[sticks.left]` / `[sticks.right]` configuration,
- quitting REAPER while the pad rumbles (new shutdown hook).
