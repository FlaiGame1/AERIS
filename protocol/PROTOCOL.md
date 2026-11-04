# AERIS Protocol v1

**Status:** early development implementation  
**Transport:** WebSocket over TCP  
**Default port:** 45872  
**WebSocket path:** /ws

## 1. Overview

AERIS Plugin and AERIS Desktop communicate directly over the local network. Telegram servers are not part of the AERIS transport path.

Implemented:
- WebSocket negotiation (hello / hello_ack);
- six-character pairing code, valid until the Desktop receiver is stopped;
- session pairing (pair / pair_result);
- ping/pong;
- verified media transfer in binary chunks;
- audio playback in AERIS Desktop;
- remote audio controls (play, pause, stop, seek and volume).

## 2. Network and security

Desktop binds to a detected private LAN address. The transport currently uses unencrypted ws:// and a temporary pairing code generated at receiver startup. Use only on a trusted private Wi-Fi network. Do not expose TCP port 45872 to the public internet.

Encrypted transport, persistent device credentials and pairing revocation remain future work.

The pairing code remains valid until the receiver is stopped so the phone can reconnect for each control command. Restarting Desktop generates a new code.

## 3. Message envelope

Every text message is a UTF-8 JSON object:

    {
      "protocol": "aeris",
      "version": 1,
      "type": "hello",
      "id": "request-id",
      "payload": {}
    }

- protocol: always "aeris".
- version: integer protocol version, currently 1.
- type: message type.
- id: optional request identifier echoed by the response.
- payload: message-specific object.

Maximum incoming text message size: 64 KiB. Maximum binary chunk size: 1 MiB.

## 4. Connection and pairing

1. Desktop starts the receiver and displays ws://<LAN-IP>:45872/ws and a six-character code.
2. Plugin opens a WebSocket connection and sends hello.
3. Desktop replies hello_ack.
4. Plugin sends pair with the displayed code.
5. Desktop replies pair_result.
6. The paired connection may transfer media or issue player commands.

A new connection must pair again with the current code. The code remains valid until the Desktop receiver is stopped.

## 5. Media transfer

After pairing, the plugin sends begin_media with:
- file_name
- mime_type
- size
- sha256

Desktop replies media_ready. The plugin sends binary WebSocket frames of at most 1 MiB and then finish_media. Desktop verifies the exact byte count and SHA-256 before playback.

The current plugin can resolve a Telegram media path only when the file is already present in Telegram's local cache. A file that has not been downloaded to the phone cannot yet be transferred. Telegram blocking does not affect the direct phone-to-PC LAN connection, but it can prevent Telegram from downloading uncached media.

Files are limited to 512 MiB and stored under %TEMP%\AERIS\Received.

## 6. Playback commands

All commands require a paired session.

| Type | Direction | Payload | Purpose |
|---|---|---|---|
| play | Plugin → Desktop | {} | Resume current AERIS audio |
| pause | Plugin → Desktop | {} | Pause current AERIS audio |
| stop | Plugin → Desktop | {} | Stop current AERIS audio |
| seek | Plugin → Desktop | position (seconds) | Seek within current audio |
| set_volume | Plugin → Desktop | volume (0.0–1.0) | Set AERIS audio volume |
| ping | Plugin → Desktop | {} | Connection check |

Successful commands receive command_result:

    {
      "protocol": "aeris",
      "version": 1,
      "type": "command_result",
      "id": "pause-1",
      "payload": {"success": true, "command": "pause"}
    }

Failed commands receive error with code player_command_failed or another relevant error code.

AERIS Desktop currently plays supported audio formats internally. Video and unsupported media formats are opened with the Windows default application; remote controls for external video players are not implemented.

## 7. Close and errors

The plugin sends a WebSocket close frame with status code 1000 and waits briefly for the Desktop close response. A normal close should not produce a connection-reset warning.

Errors use this structure:

    {
      "protocol": "aeris",
      "version": 1,
      "type": "error",
      "id": "request-id",
      "payload": {"code": "player_command_failed", "message": "No media is playing"}
    }

## 8. Roadmap

- automatic download of uncached Telegram media;
- native video playback and remote video controls;
- Telegram media-player overflow-menu and profile music integration;
- playback-state synchronization;
- encrypted transport, persistent device credentials and pairing revocation.

Arbitrary filesystem paths or shell commands are never accepted from the phone.
