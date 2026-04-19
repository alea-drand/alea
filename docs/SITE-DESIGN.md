# alea.so — site design (Pencil track)

Design-only artifacts for the alea.so splash + `/quickstart`. Lives on `feature/site-design` branch. **No code lives here** — when design is locked, code phase queues on a separate branch.

## Layout

- `docs/alea-site.pen` — single Pencil document: brand system + 4 splash variants (desktop + mobile) + `/quickstart`
- `docs/brand/` — exported brand kit (PDF / PNG)
- `docs/screenshots/` — per-frame PNG exports at 2× scale
- `docs/screenshots/claude-design/` — drop zone for `claude.ai/design` output to compare against
- `docs/COMPARISON.md` — side-by-side grid (Pencil × Claude Design)
- `docs/DECISION.md` — final winner + merge rationale (Phase H)

## Source of truth

- Design spec: `~/vault/10-projects/alea-site-design-spec.md`
- Execution plan: `~/.claude/plans/us-rag-for-context-greedy-owl.md`
- Protocol status: `~/vault/80-learnings/claude-memory/project_drand_solana.md`

## Scope boundary

Pencil MCP + markdown only. No `package.json`, no `src/`, no framework dependencies. The code phase queues on a separate branch when Aaron picks the winner.
