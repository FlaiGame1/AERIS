# AERIS Protocol v1

**Status:** early development implementation  
**Transport:** WebSocket over TCP  
**Default port:** 45872  
**WebSocket path:** /ws

## Overview

AERIS sends media from the Android Telegram client to a Windows receiver over the local network. The receiver has a native audio engine for supported audio formats. Video and other media are opened with the Windows default application in this version.

## Network and pairing

The desktop binds to the detected private LAN address. The six-character pairing code is generated at receiver startup and remains valid until the receiver stops. This allows the plugin to reconnect after a Wi-Fi interruption without asking the user to re-pair. Restarting AERIS Desktop rotates the code.

The first implementation uses unencrypted `ws://`. Use only on a trusted private network; do not expose TCP port 45872 to the internet. Encrypted transport, persistent device credentials and pairing revocation are future work.

## Message envelope

```json
{
  "protocol": "aeris",
  "version": 1,
  "type": "hello",
  "id": "request-id",
  "payload": {}
}
```

Text messages are limited to 64 KiB. Binary media frames are limited to 1 MiB each.

## Connection

1. Desktop starts the server and displays its LAN address and pairing code.
2. Plugin opens a WebSocket connection and sends `hello`.
3. Desktop replies with `hello_ack`.
4. Plugin sends `pair` with the displayed code.
5. Desktop replies with `pair_result`.
6. Plugin keeps this WebSocket connection open and reuses it for transfers and playback controls. A normal WebSocket close handshake is used when the plugin unloads.

The plugin reconnects and pairs again if the connection drops. The code remains valid for the lifetime of the running desktop receiver.

## Media transfer

After pairing, the plugin sends `begin_media` with `file_name`, `mime_type`, `size` and `sha256`. Desktop replies `media_ready`. The plugin sends binary frames of at most 1 MiB and then `finish_media`. Desktop verifies exact size and SHA-256 and stores the file under `%TEMP%\\AERIS\\Received`.

Supported audio files (MP3, WAV, FLAC, OGG/OGA, OPUS and supported M4A/AAC variants) are played by the native AERIS audio engine. Video and other formats are opened with the Windows default application; native video playback is not implemented yet.

The plugin resolves files from Telegram's local cache. If Telegram has not downloaded a media file, the user must download it in Telegram first. The transfer itself is local phone-to-PC traffic and does not route through Telegram servers.

## Playback commands

All commands require a paired session.

| Type | Direction | Payload | Purpose |
|---|---|---|---|
| play | Plugin → Desktop | `{}` | Resume audio |
| pause | Plugin → Desktop | `{}` | Pause audio |
| stop | Plugin → Desktop | `{}` | Stop audio |
| seek | Plugin → Desktop | `{"position": 42.0}` | Seek to seconds |
| set_volume | Plugin → Desktop | `{"volume": 0.0}`–`1.0` | Set audio volume |
| get_status | Plugin → Desktop | `{}` | Get current audio state |
| ping | Plugin → Desktop | `{}` | Connection check |

A successful control response is `command_ack` with the current player state in `payload.player`. `get_status` returns `player_status` with `title`, `state`, `position_seconds` and `volume`. An error response uses `type: "error"` and includes a machine-readable code and message.

The Android plugin exposes play, pause, stop, seek, volume and status actions in its settings. Playback controls currently operate the built-in AERIS audio engine; they do not control a video opened in an external Windows application.

## Security and limitations

- No arbitrary file paths, shell commands or remote desktop control are accepted.
- WebSocket traffic is unencrypted in this development version.
- Pairing is based on a six-character code valid for the receiver process lifetime.
- Media transfer limit is 512 MiB.
- Automatic downloading of uncached Telegram media is not implemented.
- Native video playback and Telegram's built-in player overflow-menu integration are not implemented.
