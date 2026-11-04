# Changelog

All notable AERIS changes are documented here.

## [1.0.0] - 2026-11-04

First public release.

### Added
- Android Telegram plugin for AyuGram / exteraGram.
- Direct media transfer from the Telegram client's local cache to AERIS Desktop over a local network.
- Windows desktop receiver and bundled mpv playback runtime.
- Audio and video playback on Windows.
- Device pairing with a short-lived code and remembered device token.
- SHA-256 verification for transferred media.
- Remote playback controls: play, pause, stop, seek and volume.
- Playback status reporting.
- Browser-based local-file uploader that works independently of Telegram.
- Portable Windows x64 package and CI validation.

### Security and limitations
- Transport is unencrypted WebSocket/HTTP and must only be used on a trusted private LAN.
- Telegram media must already be downloaded and accessible through the active client's FileLoader.
- The video message's native three-dot menu integration, transfer resume, device-management UI and encrypted transport are not included in 1.0.0.
- The Windows player opens in a separate mpv window.
