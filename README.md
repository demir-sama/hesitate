# Hesitate

**A wallet that pauses before your AI agent does something stupid.**

AI agents are getting their own NEAR accounts. They read web pages, READMEs, and tool output, and a single prompt injection can make one pay the wrong account or hand over a full-access key. Hesitate puts a smart-contract vault between the agent and its money. A separate AI sentinel scores every non-trivial transaction. Drains, look-alike receivers, key grabs and drip attacks get held or refused on-chain before any funds move.

Live demo: https://hesitate-sama-5806.vercel.app · Contract: [`hesitate-vault.testnet`](https://testnet.nearblocks.io/address/hesitate-vault.testnet)

Track: **AI Security & Defense** (spending limits, policy engines, function-call keys).

## How it works

```
agent (FC key: propose only) ──► vault contract rules ──► sentinel (FC key: score/deny only) ──► receiver
                                     │                         │
                                     └── Refused / Held ◄──────┘── owner (approve/reject) decides
```

Four roles, each with the least power it needs:

| Role | Key | Can call |
|---|---|---|
| Agent | function-call key | `propose` |
| Sentinel | function-call key | `score`, `sentinel_deny` |
| Owner | wallet | `approve`, `reject`, `set_policy`, `set_lists`, `withdraw` |
| Vault | contract | executes transfers / function calls |

**On-chain rules** (checked inside `propose`):
1. A receiver that is the vault itself, or is on the denylist → **Refused**
2. Dangerous methods (`add_full_access_key`, `add_key`, `delete_*`, `deploy`, `upgrade`, `set_owner`, `approve`…) → **Held**
3. Over the per-transaction cap or the daily cap → **Held**
4. Allowlisted, tiny (≤ `instant_max`), plain transfer → paid **instantly**
5. Everything else → **Waiting** for the sentinel during a short window

**Sentinel** (off-chain, `web/src/lib/sentinel.ts`) combines deterministic signals with an LLM review: look-alike receivers (Levenshtein distance to the allowlist), injection phrases in the agent's source content, a receiver that doesn't exist, first-time receivers, size relative to the cap, payment bursts, and suspicious args. The final risk is `max(rules, AI)`, so the AI can only *raise* risk and never wave something through. A low score releases a Waiting proposal; a high score holds it. **Fail-closed:** if the sentinel never answers, `release` holds the proposal.

## Demo scenarios (all real testnet transactions)
- Pay the GPU rental (allowlisted, 0.03 Ⓝ) → paid instantly
- Buy a dataset from a new merchant (0.12 Ⓝ) → sentinel scores it low and releases it
- A poisoned README tells the agent to pay `gpu-rental-billing.testnet` → held (look-alike + injection)
- "Enable auto-compound" calls `add_full_access_key` → held
- Fake airdrop claim → refused (denylist)
- Drip attack: 0.049 Ⓝ repeated → held on the burst
- Free-form: paste your own malicious content; a deliberately naive LLM agent follows it, and Hesitate catches it.

## Repo
- `contract/`: Rust, near-sdk 5. `cargo test` runs 9 unit tests; build with `cargo near build non-reproducible-wasm`.
- `web/`: Next.js 15, near-api-js, HOT near-connect wallet, Vercel AI Gateway. Env: `AGENT_KEY`, `SENTINEL_KEY`, `OWNER_KEY` (function-call private keys).

## Limits (honest)
- The sentinel is a single off-chain service. The next step is running it in a TEE or across several sentinels.
- The demo vault account still has its deployer full-access key. In production you would remove it, or put it behind a multisig.
- The public demo owner key can only approve small, low-risk holds. Real approvals come from the owner's wallet.
