# togglerecord

A multistream valve-like plugin that ensures multiple streams start/end at the
same time.

It supports both live and non-live input and toggles recording via the
`record` property. Live inputs will be dropped when not recording, while
non-live inputs will be blocked.

## Use cases

The `is-live` property refers to whether the output of the element will be
live. So, based on whether the input is live and on whether the output
`is-live`, we have these four behaviours:

- Live input + `is-live=false`:
  - While not recording, drop input
  - When recording is started, offset to collapse the gap

- Live input + `is-live=true`:
  - While not recording, drop input
  - Don't modify the offset

- Non-live input + `is-live=false`: 
  - While not recording, block input
  - Don't modify the offset

- Non-live input + `is-live=true`:
  - While not recording, block input
  - When recording is started, offset to current running time
