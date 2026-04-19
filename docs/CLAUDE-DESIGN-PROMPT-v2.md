# Alea Site — Claude Design prompt v2 (paste-ready)

Round 2 prompt for `claude.ai/design`. Builds on your v1 output (which landed the right aesthetic register), adds all the decisions we locked this session, and pins copy so the next iteration doesn't drift.

## How to use

1. Paste the fenced block below verbatim into `claude.ai/design`
2. Do NOT re-prompt if the first generation drifts — use the tweaks panel
3. Upload `/Users/aaron/vault/10-projects/alea-site-design-spec.md` alongside if Claude Design allows doc attachments (grounds it in the brand kit)
4. Do NOT connect the `alea-drand/alea` GitHub repo (Anchor/Rust, no frontend — would pollute design system extraction)

## What is locked vs what is tunable

**Locked (do not let Claude Design change):**
- Palette: stone / ink / marble / dust / rule / rubric / rubric-ink hex values
- Hero copy: `Alea iacta est.` (mixed case, "Alea" rubric red, rest ink) — v1 matched this, v2 should keep
- Subhead: `Stateless drand verification · Built on Solana.` (italic serif, v1 nailed it)
- Voice: Latin cadence, no "first/revolutionary/democratize", no SaaS language
- No LoE / League of Entropy / beacon-operator references anywhere (private north star only)
- Real values: program ID `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`, ~407K CU, R5 audit, MIT

**Tunable this round:**
- Beacon module details (heartbeat, rolling log, post-click proof block)
- Rubric count per page (can land at 4-5 if each is <2 characters)
- Layout balance between hero and beacon module

---

## Paste this prompt

ALEA — Splash Site, Iteration 2

Build on your previous output for alea.so (the classical serif hero with "Alea iacta est." on the left and a live beacon panel on the right). Keep the aesthetic direction — you nailed it. This iteration focuses on the demo module and a few typography/copy refinements.

### WHAT ALEA IS (unchanged)

Alea is the first on-chain drand BN254 verifier for Solana. MIT, open source, audited, live on Solana devnet at program `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`. drand (League of Entropy) emits verifiable random beacons every 3 seconds. Alea proves those beacons on-chain in ~407,000 compute units per verify. Stateless, no operator, no admin keys, upgrade authority going to a Squads multisig.

This is a public good. Not a SaaS. Visitors should feel they encountered a permanent artifact of the chain — monument-grade, museum-quiet.

### WHAT TO KEEP FROM ITERATION 1

- Warm stone background (`#F5F3EE`)
- Hero typography — a Latin-inflected serif with italic stress. Mixed case "Alea iacta est." where "Alea" is rubric red `#C81D25` and "iacta est." is ink `#0A0A0A`. Whatever serif you chose (appeared to be a Garamond/Newsreader relative) — KEEP IT. The character was right. Do NOT switch to all-caps Cinzel or similar inscription face.
- Italic serif subhead "Stateless drand verification · Built on Solana" in dust grey — KEEP
- Thin 1px ink horizontal rules at very top and very bottom of the frame (cartouche framing)
- Utility top bar, left-justified: `■ LIVE ON DEVNET   PROGRAM ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U   ~407K CU`. Right-justified: `◆ OPEN SOURCE · MIT`
- Link row at bottom: `DOCS   GITHUB   QUICK START` — small caps mono, underlined
- Live beacon module pinned to the right of the hero, dark header bar (`#0A0A0A`) with small rubric indicator square + `LIVE BEACON · DRAND EVMNET` label left and `03S CADENCE` right

### WHAT TO CHANGE IN THE BEACON MODULE

**1. Rename the `EPOCH` row.** drand uses "round" not "epoch". Replace the EPOCH row with a `CHAIN` row showing the drand chain name. The module reads top-to-bottom:

```
ROUND    6,039,957
RAND     0x5c38957c9d2a8b4c…e1f8db59967062
SIG      0xaf2efa7a23e1c90a…7b5dfe5e9a
CHAIN    evmnet  ·  fetched from api.drand.sh
STATUS   ✓ VERIFIABLE
```

**2. Longer hex prefixes.** Current truncation shows 8 + 6 hex chars. Extend to 12 + 8 so the hash feels like real cryptographic material, not decoration.

**3. Add drand attribution in CHAIN row.** `CHAIN evmnet · fetched from api.drand.sh` — good cryptographic citizenship; clarifies Alea does not generate randomness, it proves it.

**4. Live heartbeat.** A thin rubric-red hairline bar at the very top of the dark header fills left-to-right over 3 seconds, resets when a new round arrives, repeats. No other motion. Respect `prefers-reduced-motion` — bar stays static at 0%.

**5. CADENCE label becomes countdown.** Replace static `03S CADENCE` with live `NEXT IN 2.3s` ticking down, resets to ~3.0s when a new round arrives. Keep right-justified in the dark header.

**6. Status line before-click vs after-click.**
- Before click: `STATUS   ✓ VERIFIABLE` (dust on ✓, ink on VERIFIABLE)
- After click: `STATUS   ✓ VERIFIED AT slot 312,441,952 ↗` (rubric ✓, slot number links to Explorer)

### WHAT TO ADD BELOW THE BEACON MODULE

**7. Click-to-verify expand.** On click, the beacon module extends to show a real on-chain proof:

```
  FETCHING BEACON       ROUND 6,039,957
  VERIFYING ON SOLANA   tx pending…
  VERIFIED              ✓
  ───────────────────────
  RAND   0x5c38957c9d2a…e1f8db59
  SIG    0xaf2efa7a23e1…7b5dfe5e9a
  TX     0x4tr4yetr4j3U…TnD9Zdtj  ↗
  PROG   ALEAydzHd4cN2…VqsCk2U   ↗
  COST   ~0.00005 SOL  ·  407,128 CU  ·  confirmed 1.2s
```

- Each of 3 top steps animates in over ~400ms with spring curve
- TX and PROG are underlined mono, open Solana Explorer in new tab
- Button copy changes to `VERIFY ANOTHER →` after first click

**8. Rolling verification log.** Below beacon + proof area:

```
RECENT VERIFICATIONS
  ✓  6,039,956   tx 4tr4…Zdtj   0.8s ago
  ✓  6,039,955   tx 5bbi…GTDut  3.8s ago
  ✓  6,039,954   tx 9xC8…pMnAr  6.8s ago
  ✓  6,039,953   tx 2ty5…JUPW   9.8s ago
  ✓  6,039,952   tx 7mQk…bX4h   12.8s ago
```

- JetBrains Mono 12-13px
- Each row links to Explorer for that tx
- Updates live as new verifications happen (top slides in, bottom falls off)
- Label `RECENT VERIFICATIONS` in small caps, 11px, ink

**9. Button loading state.** When VERIFY pressed, button text becomes `VERIFYING…` with cycling ellipsis (`.` → `..` → `...` → `.`). No spinner. Roman/mechanical register.

**10. Rate-limit / offline state.**
- Rate limit: button disables, text replaces with `NEXT VERIFICATION IN 4s` (countdown)
- Offline: button disables, text replaces with `DEMO OFFLINE — SEE LAST PROOF ↗` (links to Explorer)
- Both use dust-colored text instead of rubric

### RUBRIC RED BUDGET

Total rubric appearances per page ≤ 5, each ≤ 2 characters wide except hero word. Plan:
1. The word "Alea" in hero (prominent — fine)
2. `■` indicator in top-left utility bar
3. `■` indicator in LIVE BEACON header
4. `◆` indicator in top-right OPEN SOURCE · MIT
5. `ALEA` word inside VERIFY button (optional)
6. Heartbeat progress bar at top of beacon header (transient)

### MOTION REGISTER

Near-static. Mechanical. Stone, not water. Moving things:
- Beacon live-ticker values (3s drand cadence)
- Heartbeat progress bar (linear, 3s loop)
- CADENCE countdown text
- VERIFY click reveal (3 steps, ~400ms spring each)
- Hover transitions (150-200ms ease-out)

No parallax. No fade-in-on-scroll. No particle fields. No gradient glow. Respect prefers-reduced-motion throughout.

### COPY — DO NOT USE

- Never: "first", "revolutionary", "democratize", "empower", "the future of", "unlock"
- Never: "Start free trial", "Join waitlist", "Get started", "Sign up"
- Never: "Fast / Secure / Scalable" three-column strip
- Never: emoji (small caps unicode like ✓ and ■ are fine; typographic glyphs not emoji)
- Never: LoE / League of Entropy / beacon operator / "Solana's node" / future-state aspiration

### TECHNICAL CONSTRAINTS

- Target framework: Next.js 15 + Tailwind v4 + Motion for React (output HTML/CSS is fine, code translation happens later)
- Fonts: Google Fonts. Keep v1 serif pick. IBM Plex Mono or JetBrains Mono for monospaced data (tabular figures required)
- Data source: `https://api.drand.sh/public/latest` for live ticker, fetched client-side
- Verify button: POSTs to `/api/verify` on our backend (contract documented in `docs/RELAYER-API.md`)

### OUTPUT

- 1 splash page (HTML) with all changes above
- Same design system tokens as v1 (do not re-invent palette)
- Prepared for Claude Code handoff

Generate now.

---

## Tweaks panel vs re-prompt

Claude Design iterates via its tweaks panel, not by re-prompting. After pasting the above:
- If the generation lands mostly right → use tweaks panel for small adjustments
- If it drifts toward generic SaaS → do NOT tweak; re-prompt with anti-patterns doubled

## Variables that should remain tunable in the panel

- Hero font size (scales whole frame)
- Beacon module width (500px – 620px range)
- Rolling log row count (3, 5, or 7)
- Rubric saturation (keep `#C81D25`; if CD drifts lighter, clamp at original)

## Related files on this branch

- `docs/SITE-DESIGN.md` — directory README
- `docs/RELAYER-API.md` — backend contract the VERIFY button hits
- `~/vault/10-projects/alea-site-design-spec.md` — canonical brand spec (vault-persistent)
- `~/.claude/plans/us-rag-for-context-greedy-owl.md` — this arc's execution plan
