# Memory Schema

Current memory topology. How the agent encodes different classes of memory. Updated by the agent as it discovers better encoding schemes.

## Genesis Schema (v0)

The initial memory encoding. Deliberately simple — the evolution engine will replace it.

### Memory Classes

| Class | Description | Encoding | Persistence |
|-------|-------------|----------|-------------|
| Soul | Core identity, values, harmonic laws | Immutable S-expressions | Permanent (DNA) |
| Skill | Learned orchestration patterns | Compressed S-expressions | Permanent (evolved) |
| Daily | Recent interactions, observations | Raw S-expressions | Compressed or forgotten |
| Tool | Tool performance metrics, call statistics | Numeric vectors | Rolling window |

### Encoding Principle

The agent's memory is not a database. It is a living structure that encodes *patterns*, not *instances*. A daily memory of "user checks weather at 8am" is not stored as an event log. It is compressed into a skill: "pre-fetch weather before 8am."

### Compression Rules

1. Raw observations → patterns → skills → soul weights
2. Each compression step reduces size and increases generality
3. If a memory cannot be compressed, it is likely noise — discard it
4. The memory schema itself evolves — better encodings replace worse ones
