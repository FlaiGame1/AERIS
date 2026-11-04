# AERIS Protocol v1

**Status:** early development implementation  
**Transport:** WebSocket over TCP  
**Default port:** 45872  
**WebSocket path:** /ws

## 1. Overview

AERIS uses a small JSON message protocol between AERIS Plugin and AERIS Desktop. It does not depend on Google Cast.

The current development server implements:
- WebSocket connection and protocol negotiation (hello / hello_ack);
- short-lived six-character pairing code;
- session pairing (pair / pair_result);
- ping/pong;
- validation of command envelopes and HTTP(S) media URLs;
- receipt acknowledgements for playback commands.

**Playback is not connected yet.** command_ack currently confirms that a command was received, not that media started playing.

## 2. Network and security

The desktop binds to the detected private LAN address, not to every network interface. The first prototype uses unencrypted ws:// and a temporary pairing code that expires after three minutes. This is suitable only for testing on a trusted private Wi-Fi network.

Do not use this development build on public, shared or otherwise untrusted networks. Before a public release, AERIS must add encrypted transport, persistent device credentials, pairing revocation and stronger session management.

A pairing code currently authorizes one WebSocket session. Reconnect and remembered-device support are planned.

## 3. Message envelope

Every message is a UTF-8 JSON object:

    {
      "protocol": "aeris",
      "version": 1,
      "type": "hello",
      "id": "request-identifier",
      "payload": {}
    }

- protocol: always "aeris".
- version: integer protocol version, currently 1.
- type: message type.
- id: optional request identifier echoed by the response.
- payload: message-specific object.

Maximum incoming text message size: 64 KiB.

## 4. Connection and pairing

1. Desktop starts the server and displays ws://<LAN-IP>:45872/ws and a six-character code.
2. Plugin opens a WebSocket connection.
3. Plugin sends hello.
4. Desktop replies with hello_ack.
5. Plugin sends pair with the displayed code.
6. Desktop replies with pair_result.
7. After successful pairing, the plugin may send playback commands on that connection.

The pairing code is valid for three minutes from server startup. Restarting the server creates a new code.

### hello

    {
      "protocol": "aeris", "version": 1, "type": "hello",
      "id": "hello-1", "payload": {}
    }

### hello_ack

    {
      "protocol": "aeris", "version": 1, "type": "hello_ack",
      "id": "hello-1",
      "payload": {
        "server_name": "AERIS Desktop",
        "protocol_version": 1,
        "pairing_required": true,
        "pairing_expires_in": 160
      }
    }

### pair

    {
      "protocol": "aeris", "version": 1, "type": "pair",
      "id": "pair-1", "payload": {"code": "A1B2C3"}
    }

Successful response:

    {
      "protocol": "aeris", "version": 1, "type": "pair_result",
      "id": "pair-1", "payload": {"paired": true, "session": "temporary"}
    }

## 5. Commands

All commands below require a paired session.

| Type | Direction | Payload | Purpose |
|---|---|---|---|
| play_url | Plugin → Desktop | url, optional title, thumbnail, mime_type | Request playback of a reachable HTTP(S) URL |
| play | Plugin → Desktop | {} | Resume playback |
| pause | Plugin → Desktop | {} | Pause playback |
| stop | Plugin → Desktop | {} | Stop playback |
| seek | Plugin → Desktop | position (seconds) | Seek |
| set_volume | Plugin → Desktop | volume (0.0–1.0) | Set volume |
| ping | Either → Desktop | {} | Connection check |

A command receipt response uses command_ack:

    {
      "protocol": "aeris", "version": 1, "type": "command_ack",
      "id": "play-1",
      "payload": {"received": true, "playback": "not_implemented_yet"}
    }

This acknowledgement does not mean playback succeeded. The media player integration and state events will be added in a later implementation step.

## 6. Errors

Errors use this structure:

    {
      "protocol": "aeris", "version": 1, "type": "error",
      "id": "request-id",
      "payload": {"code": "pairing_required", "message": "Pair this device first"}
    }

Initial error codes include invalid_json, message_too_large, unsupported_protocol, pairing_required, invalid_or_expired_code, invalid_media_url and unknown_message_type.

## 7. Media transfer roadmap

A media URL is useful only if the Windows PC can access it. Telegram-private media URLs often cannot be reused outside the Android app.

The protocol will therefore gain a separate authenticated media-transfer flow for private Telegram media. It must support bounded chunk sizes, integrity checks, cancellation and cleanup of temporary files. Arbitrary filesystem paths or shell commands will never be accepted from the phone.
