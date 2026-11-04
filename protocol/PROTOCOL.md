# AERIS Protocol — draft / черновик

**Version:** 0.1-draft  
**Status:** design only; not yet implemented.

## Goals / Цели

- Direct local-network communication; no Google Cast dependency.
- Separate transport/protocol from Telegram plugin internals.
- Manual pairing first; automatic discovery later.
- Send media URLs and metadata, then playback-control commands.
- Explicit authentication and safe LAN exposure before public release.

## Transport

Initial proposal: persistent WebSocket connection over the local network. Desktop listens on a configurable TCP port; the plugin connects using a QR code or manually entered address.

The first prototype may use plain WebSocket only on a trusted private network for development. Before public release, implement authenticated pairing and encrypted transport (WSS or an equivalent secure channel). Do not expose an unauthenticated receiver to public networks.

## Message envelope

```json
{
  "protocol": "aeris",
  "version": 1,
  "type": "ping",
  "id": "request-id",
  "payload": {}
}
```

## Initial message types

| Type | Direction | Purpose |
|---|---|---|
| hello | plugin → desktop | Negotiate protocol version |
| hello_ack | desktop → plugin | Return capabilities |
| pair | plugin → desktop | Submit one-time pairing token |
| pair_result | desktop → plugin | Pairing result |
| play_url | plugin → desktop | Start media URL with metadata |
| play / pause / stop | plugin → desktop | Playback controls |
| seek | plugin → desktop | Seek to a position in seconds |
| set_volume | plugin → desktop | Set volume from 0.0 to 1.0 |
| state | desktop → plugin | Playback state and position |
| error | either direction | Structured error |

## Media payload (draft)

```json
{
  "url": "https://example.invalid/media.mp4",
  "title": "Example video",
  "thumbnail": "https://example.invalid/cover.jpg",
  "mime_type": "video/mp4"
}
```

The desktop must be able to access the supplied URL. Telegram private media may require a separate transfer/proxy design; do not assume a URL available inside the Android app is also accessible from Windows.

## Security requirements

- Bind only to private LAN interfaces by default.
- Require explicit pairing before accepting commands.
- Expire and revoke pairing tokens.
- Validate message size, URL schemes and numeric values.
- Never execute commands or filesystem paths received from the phone.
- Avoid logging tokens or private media URLs.
- Show paired devices and provide disconnect/revoke controls.
