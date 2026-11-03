# Hyaku Development & Writing Rules (Unslop Standards)

Always follow these principles across all code, comments, commit messages, in-game strings, and documentation:

## 1. Code & Comments
- **No obvious or patronizing comments**: Do not restate what the code clearly does (e.g. avoid `// Increment i by 1` or `// Return response`).
- **Explain "Why", not "What"**: Reserve comments for non-obvious invariants, math rationale, protocol details, or tricky terminal/networking edge cases.
- **Zero dead code**: Delete commented-out code, unused scaffolding, and speculative abstractions instead of leaving them around.
- **Direct, functional design**: Avoid over-engineering, enterprise design patterns for simple logic, or unnecessary wrapper layers.

## 2. Commit Messages
- **Natural, direct, lowercase or standard imperative**: Write plain human commit messages (e.g., `fix slash collision on diagonal`, `add combat and tether coop`, `update release workflow`).
- **No AI boilerplate**: Never use buzzwords, marketing framing, emojis, or robotic structured summaries in commit subjects.

## 3. Prose, UI & Game Strings
- **No AI jargon or buzzwords**: Never use words like *seamless*, *robust*, *delve*, *leverage*, *revolutionize*, *comprehensive*, *testament*, *tapestry*, *landscape*, *crucial*, or *unwavering*.
- **Grounded, atmospheric tone**: In-game announcements, terminal prompts, and UI copy should be punchy, arcade-like, and functional.
- **No templated outlines or staccato lists**: In documentation, favor direct technical instructions over bloated bullet listicles and formulaic opening/closing sentences.
- **Preserve facts strictly**: Never invent mechanics, classes, keys, or numbers that don't match the actual implementation.
