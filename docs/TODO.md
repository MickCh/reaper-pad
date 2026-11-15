# To do

Open items, most useful first. Remove an item in the same commit that does
it.

## Ideas

- **Named action IDs.** `action:` only takes numeric IDs. Actions from
  extensions (SWS, reaper-pad itself) and ReaScripts have text IDs such as
  `_SWS_ABOUT` or `_REAPER_PAD_TEST`, and their numbers are assigned per
  session, so they cannot be bound today. Accept `action:_NAME` and resolve
  it with `NamedCommandLookup` when the configuration is loaded (report
  unknown names like other config errors).
