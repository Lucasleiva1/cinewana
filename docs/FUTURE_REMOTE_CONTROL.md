# Future remote control

WebSocket commands will deserialize into the existing `PlayerCommand` enum. `PlayerState` snapshots and deltas will be published immediately. Pairing will use expiring QR challenges, hashed device tokens, certificate pinning, and revocation.

