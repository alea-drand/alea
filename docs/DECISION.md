# Alea Site Design — Round 1 Decision

**Status:** Round 1 complete. Claude Design wins on the splash hero + beacon module. Pencil stays as reference brand kit.
**Branch:** `feature/site-design`
**Date:** 2026-04-19

## What was produced this arc

| Artifact | Tool | State | Location |
|---|---|---|---|
| Brand system (colors, typography, wordmark, buttons, links, rubric rules) | Pencil MCP | In-memory only (Pencil disconnected before save) | `docs/alea-site.pen` — needs Pencil session to recover |
| 4 desktop splash variants (Pure Trajan, Byte Column, Byte Bust, Live Beacon) | Pencil MCP | In-memory only | same |
| 4 mobile variants (A/B/C/D at 390×844) | Pencil MCP | In-memory only | same |
| /quickstart frame | Pencil MCP | In-memory only | same |
| Splash hero + beacon module | claude.ai/design | Saved in web session, screenshots shared | n/a (web) |
| Relayer API contract | Markdown | Committed | `docs/RELAYER-API.md` |
| Claude Design round 2 prompt | Markdown | Committed | `docs/CLAUDE-DESIGN-PROMPT-v2.md` |

## Winner: Claude Design v1 direction

Aaron's assessment after seeing both tools side-by-side: "claude design did much better".

Reasons the CD output outclassed Pencil round 1:

1. **Typography register.** CD used a classical italic serif for both hero and subhead (Garamond/Newsreader family) rather than inscription all-caps Cinzel. Mixed case "Alea iacta est." with "lea" lowercase reads more humanist/editorial — less forced monument.
2. **Horizontal cartouche composition.** Hero dominates left, beacon pinned right, thin 1px ink rules at top and bottom of frame. Pencil's vertical stacks with a left-side column/bust decoration were less confident.
3. **Utility top bar.** `■ LIVE ON DEVNET  PROGRAM …  ~407K CU` left + `◆ OPEN SOURCE · MIT` right — dense real info as ornament. Pencil didn't reach this.
4. **Beacon module polish.** Dark `#0A0A0A` header bar with rubric `■` indicator + `CADENCE` callout right-justified. Pencil's outlined box was more wireframe.
5. **Inline VERIFY button.** Sized to beacon width, `VERIFY A ROUND WITH ALEA →` with `ALEA` in rubric inside the button text. Echoes hero accent.

## What Pencil was good for

Not wasted. Pencil produced:
- A complete brand system spec with 7 color swatches + usage, 3 font specimens at required weights, 4 button states, 3 link states, rubric do/don't panel
- 4 hero variant ideas that stress-tested how far the direction can stretch (Pure Trajan pushed typography-only; Live Beacon proved the live-data register; Byte Column and Byte Bust probed ornamental ASCII)
- Mobile reflow patterns (2-line hero wrap, 2×2 link grid, compact beacon box)
- /quickstart page layout

If Pencil MCP reconnects and the `.pen` file is recoverable, these artifacts preserve as a reference library. If not, the knowledge is in the screenshots Aaron already saw inline in this session.

## Round 2 plan

Aaron's call: iterate inside Claude Design, not Pencil.

1. **Paste `docs/CLAUDE-DESIGN-PROMPT-v2.md` into claude.ai/design.** Contains every refinement agreed this session:
   - Rename `EPOCH` row to `CHAIN` (drand uses "round" not "epoch")
   - Longer hex prefixes (12+8 chars) so hashes feel cryptographic
   - Add drand source attribution (`CHAIN  evmnet · fetched from api.drand.sh`)
   - Live heartbeat (thin rubric bar filling L→R over 3s at top of beacon header)
   - CADENCE label becomes live countdown (`NEXT IN 2.3s`)
   - STATUS transitions: `✓ VERIFIABLE` → `✓ VERIFIED AT slot N ↗` post-click
   - Click-to-verify expand: 3-step reveal (FETCHING / VERIFYING / VERIFIED) then persistent proof block with RAND / SIG / TX / PROG / COST
   - Rolling verification log strip (last 5 proofs, live updating)
   - Button loading state (`VERIFYING…` ellipsis), rate-limit state (`NEXT VERIFICATION IN 4s`), offline state (`DEMO OFFLINE — SEE LAST PROOF ↗`)

2. **Use Claude Design's tweaks panel for small adjustments.** Do not re-prompt if the first generation lands mostly right.

3. **If it drifts toward generic SaaS,** re-prompt with the same v2 brief and doubled anti-patterns section.

## Demo mode — decided

The `VERIFY` button hits a real Solana relayer (not mock, not pre-verified cherry-pick).

**Phase 4 (now): devnet relayer**
- Alea only deployed to devnet currently
- Devnet SOL is free (programmatic topup via Helius faucet API, QuickNode fallback, public airdrop tertiary)
- Starting fund: 1 SOL devnet (≈ 200,000 verifies headroom)
- Per-click cost: effectively zero

**Phase 5+: mainnet-beta relayer**
- Flip `ALEA_CLUSTER` env var when mainnet deploy ships
- Starting float: 0.1 SOL (~$20 at current price)
- Per-click cost: ~0.00005 SOL (~$0.01)
- Realistic monthly cost at splash traffic: $5–50/mo
- Rate limit 6/min per IP bounds worst case to single-digit $/day

Full API contract in `docs/RELAYER-API.md`.

## Next session — code phase scope

When Aaron queues the code phase as a separate arc:

1. Scaffold Next.js 15 + Tailwind v4 + Motion for React in a fresh worktree off this branch
2. Translate the finalized Claude Design HTML into the component tree
3. Implement the relayer service per `docs/RELAYER-API.md` contract
4. Set up devnet keypair + faucet topup cron
5. Playwright screenshot verification across 375/768/1024/1440 breakpoints
6. Vercel preview deploy
7. alea.so DNS cutover

## Open threads (none blocking round 2)

- **Pencil `.pen` recovery.** If Aaron wants the brand kit artifacts committed for future reference, restart the Pencil desktop app, re-open this worktree's `docs/alea-site.pen` path (Pencil may auto-save recovery file), save explicitly, then export screenshots and commit. Not blocking — Claude Design is the active track.
- **Mainnet cost planning.** $5–50/mo is comfortable for a public good splash. If abuse pushes cost higher, Cloudflare Turnstile on `/api/verify` is the pre-built lever.
- **Rate limit tuning.** Start with 6/min per IP. Monitor and tighten to 3/min or add Turnstile if abused. Aaron flagged this will be something he adjusts.

## Related files

- `docs/SITE-DESIGN.md` — directory overview
- `docs/RELAYER-API.md` — backend contract (implementation queued)
- `docs/CLAUDE-DESIGN-PROMPT-v2.md` — paste-ready iteration prompt
- `~/vault/10-projects/alea-site-design-spec.md` — canonical brand spec (vault)
- `~/vault/10-projects/alea-site-execution-plan.md` — prior execution plan (vault)
- `~/.claude/plans/us-rag-for-context-greedy-owl.md` — this arc's plan
