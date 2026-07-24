# Wraith Protocol Indexer Example

A Subgraph-style TypeScript indexer for Wraith Protocol on Stellar that ingests contract events, stores them in Postgres, and exposes a GraphQL API.

## Features

- Ingests events from all 4 Wraith contracts (Stealth Announcer, Registry, Sender, Wraith Names)
- Reorg-safe with watermark tracking and rollback
- GraphQL API powered by PostGraphile
- Docker Compose setup for easy deployment

## Quick Start

```bash
# Start with Docker Compose
docker-compose up -d
```

The GraphQL API will be available at http://localhost:3000/graphiql.

## Manual Setup

1. Install dependencies:
   ```bash
   npm install
   ```

2. Copy .env.example to .env and configure:
   ```bash
   cp .env.example .env
   ```

3. Start Postgres:
   ```bash
   docker-compose up -d postgres
   ```

4. Run the indexer:
   ```bash
   npm run dev
   ```

## GraphQL Endpoints

### Name Resolution
```graphql
query ResolveName($name: String!) {
  allNames(condition: { name: $name }, orderBy: CREATED_AT_DESC, first: 1) {
    nodes {
      name
      owner
      stealthMetaAddress
    }
  }
}
```

### Announcement Lookup
```graphql
query GetAnnouncements($stealthAddress: String!) {
  allAnnouncements(condition: { stealthAddress: $stealthAddress }) {
    nodes {
      ledger
      transactionHash
      schemeId
      stealthAddress
      ephemeralPubKey
      metadata
    }
  }
}
```

### Registry Lookup
```graphql
query GetRegistry($registrant: String!, $schemeId: Int!) {
  allRegistries(condition: { registrant: $registrant, schemeId: $schemeId }, orderBy: CREATED_AT_DESC, first: 1) {
    nodes {
      registrant
      schemeId
      stealthMetaAddress
    }
  }
}
```

## Testing

Run tests with:
```bash
npm test
```

The tests verify reorg handling logic.
