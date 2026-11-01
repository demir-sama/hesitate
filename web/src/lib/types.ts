export type Status = "Waiting" | "Cleared" | "Held" | "Executed" | "Rejected" | "Refused";
export type Proposal = { id: string; receiver: string; amount: string; method: string | null; args: string | null; gas_tgas: number; memo: string; created_ns: string; status: Status; risk: number | null; reasons: string[]; decided_by: string | null };
export type Policy = { per_tx_cap: string; daily_cap: string; instant_max: string; window_sec: number; hold_at_risk: number; fail_closed: boolean };
export type VaultState = { owner: string; agent: string; sentinel: string; policy: Policy; allow: string[]; deny: string[]; spent_today: string; balance: string; paused: boolean; next_id: string };
export type Intent = { receiver: string; amountNear: number; method?: string | null; args?: Record<string, unknown> | null; memo: string };
export type Signal = { id: string; label: string; weight: number; hit: boolean; detail: string };
export type Verdict = { risk: number; rulesRisk: number; aiRisk: number | null; signals: Signal[]; reasons: string[]; model: string; summary: string };
export type Step =
  | { kind: "agent"; text: string; source?: string }
  | { kind: "intent"; intent: Intent; model: string }
  | { kind: "proposed"; proposal: Proposal; hash: string }
  | { kind: "sentinel"; verdict: Verdict }
  | { kind: "scored"; proposal: Proposal; hash: string }
  | { kind: "error"; text: string }
  | { kind: "done" };
