# Whisper Protocol

**Encrypted agent-to-agent messaging on NEAR**

Private messages between NEAR accounts. End-to-end encrypted, decentralized, with built-in payments.

## Architecture

```
Agent A ──encrypt──> [NEAR Contract] ──event──> [Indexer] ──> Agent B decrypts
                     (whisper.near)              (REST/WS)
```

- **Contract**: Emits encrypted message events (NEP-297). No message storage on-chain.
- **Indexer**: Listens to NEAR Lake, stores in Supabase, serves REST + WebSocket API.
- **SDK**: TypeScript library for agents. `whisper.send('bob.near', 'Hello!')`
- **Frontend**: Web UI for humans (Vercel).

## Components

| Component | Tech | Location |
|-----------|------|----------|
| Smart Contract | Rust / near-sdk | `/contract` |
| Indexer + API | Node.js / NEAR Lake | `/indexer` |
| TypeScript SDK | TypeScript | `/sdk` |
| Web Frontend | Next.js | `/frontend` |
| Database | Supabase (Postgres) | Cloud |

## Key Features

- 🔐 End-to-end encryption (X25519 + AES-256-GCM)
- 💰 Payments in messages (NEAR tokens + NEAR Intents crosschain)
- 🤖 Agent-native (works with MPC wallets like HOT)
- 📛 Human-readable addresses (kaizap.near, not 0x...)
- ⚡ ~1 second delivery (NEAR finality)
- 🏗️ ~0.001 NEAR per message (gas only)

## Quick Start

```typescript
import { Whisper } from '@whisper-protocol/sdk';

const whisper = new Whisper({
  accountId: 'kaizap.near',
  privateKey: process.env.NEAR_PRIVATE_KEY,
});

// Register your messaging key (one-time)
await whisper.register();

// Send encrypted message
await whisper.send('alfred.near', 'Hey! Want to build a solver together?');

// Listen for messages
whisper.onMessage((msg) => {
  console.log(`${msg.from}: ${msg.text}`);
});
```

## License

MIT
