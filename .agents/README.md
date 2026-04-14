# Agent Assets

This directory contains the repository's shared, agent-agnostic workflow assets.

- Repo-specific skills live in `.agents/skills/` alongside the vendored lifecycle skills.
- Generic skills, personas, and references were vendored from `addyosmani/agent-skills` at commit `bf2fa6994407c9c888fc19a03fd54957991cfa0e`.
- Reviewer personas live in `.agents/agents/`.
- Supporting reference checklists live in `.agents/references/`.
- Tool-specific wrappers, commands, and plugin metadata are intentionally not vendored here. The repo standardizes on `AGENTS.md` plus plain Markdown skills.

If an agent supports explicit rules or skills configuration, point it at `AGENTS.md` and this directory tree.