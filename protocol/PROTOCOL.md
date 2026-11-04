# AERIS Protocol v1

**Status:** early implementation  
**Transport:** WebSocket over TCP  
**Default ports:** 45872 (WebSocket), 45873 (browser upload)  
**WebSocket path:** /ws

## 1. Overview

AERIS sends media from an Android Telegram client to AERIS Desktop over the local network. Desktop plays received files using the bundled mpv runtime. The phone can control the active playback session.

The active implementation includes:
- WebSocket negotiation and pairing;
- persistent device tokens saved on the desktop and in the plugin settings;
- chunked media transfer with SHA-256 verification;
- mpv playback for local audio and video files;
- play, pause, toggle, stop, seek and volume commands.

## 2. Network and security

Desktop listens on the detected private LAN address at TCP port 45872. The first pairing uses a six-character code displayed by Desktop. The plugin then stores a random device token; Desktop persists the token in %APPDATA%\AERIS\paired_devices.json.

The transport is currently unencrypted ws://. Pairing tokens are bearer credentials and are stored locally without encryption. Use AERIS only on a trusted private network. Do not expose port 45872 to the public internet. Removing a device token from the desktop paired-devices file revokes that device.

The pairing code expires three minutes after the receiver starts. Already paired devices can reconnect using their saved token.

## 3. Message envelope

Messages are UTF-8 JSON objects with protocol, version, type, optional id and payload fields. Maximum incoming text message size: 64 KiB.

## 4. Connection and pairing

1. Desktop starts the receiver and displays its LAN address and six-character pairing code.
2. Plugin connects to ws://<LAN-IP>:45872/ws.
3. Plugin sends hello; Desktop replies hello_ack.
4. On first connection, plugin sends pair with the displayed code.
5. Desktop replies pair_result with a random device token.
6. Plugin stores the token in its settings and reuses it for later connections. Desktop stores the token under the current Windows user's AppData folder.

For subsequent connections, pair contains a token instead of the short-lived code.

## 5. Media transfer

After pairing, the plugin sends begin_media with file_name, mime_type, size and sha256. Desktop replies media_ready. The plugin sends binary WebSocket frames of at most 1 MiB, then sends finish_media. Desktop verifies the exact byte count and SHA-256, saves the file under %TEMP%\AERIS\Received, and asks mpv to play it.

The maximum media size is 512 MiB. The plugin currently resolves media only from the Telegram client's local cache. Media must be downloaded in Telegram first. A separate Android file picker / standalone sender is planned so AERIS can work without Telegram.

## 6. Browser upload (Telegram-independent fallback)\n\nDesktop also serves a small upload page over HTTP on TCP port 45873. Open `http://<PC-LAN-IP>:45873` from the phone browser, enter the six-character pairing code, choose a local audio/video file and upload it. The file is streamed directly to the PC, saved under `%TEMP%\\AERIS\\Received`, and passed to mpv. The upload endpoint checks the same short-lived pairing code and enforces the 512 MiB size limit.\n\nThis path does not require Telegram to be installed, logged in or reachable. Both devices must still be on the same LAN, and the Windows firewall must allow private-network traffic to TCP ports 45872 and 45873. The HTTP upload page is unencrypted; use only a trusted private network.\n\n## 7. Playback commands

All commands require a paired session.

| Type | Payload | Purpose |
|---|---|---|
| play | empty | Resume playback |
| pause | empty | Pause playback |
| toggle_pause | empty | Toggle pause / resume |
| stop | empty | Stop current media |
| seek | position in seconds | Seek to an absolute position |
| seek_relative | offset in seconds | Seek by a relative offset |
| set_volume | volume from 0.0 to 1.0 | Set volume |
| volume_delta | delta in percentage points | Change volume |
| ping | empty | Connection check |
| get_status | empty | Return current media title, position, duration, pause state, volume and idle state |

Successful player commands return command_result with payload.success = true. A playback-engine failure returns an error message. `get_status` returns `playback_status` with the current mpv properties in its payload.

## 8. Desktop playback engine

AERIS Desktop launches the bundled mpv.exe on the first playback request and controls it over a local Windows named pipe. The mpv directory must remain beside aeris-desktop.exe. mpv's IPC endpoint is local to the desktop process and is not exposed on the LAN.

## 9. Current limitations

- Telegram media must already be downloaded and present in the app cache.
- A standalone Android file picker and independent Android sender are not implemented yet.
- The player uses a separate mpv playback window rather than an embedded AERIS video surface.
- Transport encryption, device management UI, transfer resume and token revocation UI are future work.
- Playback is limited to files supported by the bundled mpv build.
