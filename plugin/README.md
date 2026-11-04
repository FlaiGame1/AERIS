# AERIS Plugin

**AERIS Plugin 0.3.9** is the Android-side component of AERIS. It is a Python plugin for compatible AyuGram / exteraGram clients. AERIS does not ship a standalone Android app or APK.

## What works

- Adds **«Воспроизвести через AERIS»** to the message long-press context menu.
- Resolves downloaded media through the active Telegram client's FileLoader; it does not assume an AyuGram or exteraGram cache directory.
- Sends audio and video directly to AERIS Desktop over the local network.
- Pairs once with the six-character code, then reuses a remembered device token.
- Provides playback controls and playback status from plugin settings.

## Best-effort integrations

- Audio-player overflow menu injection uses Telegram's internal ItemOptions / AudioPlayerAlert implementation and may vary by client version.
- Profile music extraction depends on the profile fields exposed by the installed client.
- The three-dot overflow menu attached to an individual video message is not implemented yet; use the long-press context menu.

## Setup

1. Start AERIS Desktop on Windows and keep it on the same trusted Wi-Fi/LAN as the phone.
2. In the plugin settings, enter the PC's LAN IP and port 45872.
3. Enter the six-character pairing code shown by Desktop for the first connection.
4. In Telegram, long-press a downloaded audio or video message and select **«Воспроизвести через AERIS»**.
5. After successful pairing, the saved device token is reused automatically.

If Telegram cannot download media, use the browser uploader displayed by AERIS Desktop (`http://<PC-LAN-IP>:45873`) to choose a file from Android storage.

## Compatibility and limitations

The plugin relies on the compatible client's plugin SDK and FileLoader bridge. It can only send media that the client exposes as a readable local file; it does not independently decrypt cache blobs or bypass Telegram's download restrictions. The phone and PC must be able to communicate over the same private LAN.

The LAN transport is currently unencrypted. Use only a trusted private network and never expose ports 45872 or 45873 to the public internet.

See [the shared protocol specification](../protocol/PROTOCOL.md) and [the project README](../README.md).
