# AERIS Material 3 player

This branch is the isolated Material 3 redesign track. It starts from the current `main` and keeps the complete existing desktop receiver, Android AyuGram/exteraGram plugin, protocol, and release assets intact.

## Target desktop player — phase 1

The first desktop player surface is intentionally minimal:

1. Large square album artwork with rounded corners.
2. Background derived from the artwork palette, with a soft blurred/gradient treatment.
3. Playback timeline below the artwork.
4. Hovering the timeline reveals the exact hovered timestamp in `MM:SS`; the current playback position remains visible in the progress line.
5. Track title below the timeline.
6. Artist name below the title.
7. No transport buttons in this first version.

## Android handoff — later phase

Add one action to the Android player's three-dot menu. Selecting it connects to the paired AERIS desktop and hands off/synchronizes the currently playing track. The Android player then acts as the remote controller. This is deliberately not part of the first visual-only phase.

## Implementation sequence

- Phase 0: isolate the work on this branch and preserve the current desktop app, plugin, and protocol.
- Phase 1: build the desktop player surface and artwork-driven background using local/mock metadata; no protocol changes.
- Phase 2: connect the UI to real desktop playback state and extract artwork/title/artist/duration.
- Phase 3: add the Android three-dot handoff action and bidirectional playback synchronization.
- Phase 4: polish animations, responsive sizing, and test on Windows + Android.

Keep `main` unchanged until the branch is reviewed and explicitly synchronized.
