import { SCENARIOS } from "@/lib/scenarios";
import { act, hasKeys, toYocto, view } from "@/lib/near";
import { judge, signals } from "@/lib/sentinel";
import { naiveAgent } from "@/lib/agent";
import type { Intent, Proposal, Step, VaultState } from "@/lib/types";
export const dynamic = "force-dynamic";
export const maxDuration = 120;

const hits = new Map<string, number[]>();
function limited(ip: string) {
  const now = Date.now(), arr = (hits.get(ip) ?? []).filter((t) => now - t < 60_000);
  arr.push(now); hits.set(ip, arr); return arr.length > 8;
}

export async function POST(req: Request) {
  const ip = req.headers.get("x-forwarded-for")?.split(",")[0] ?? "local";
  if (limited(ip)) return Response.json({ error: "Easy — one attack at a time (rate limited)." }, { status: 429 });
  if (!hasKeys()) return Response.json({ error: "Agent/sentinel keys not configured on this deployment" }, { status: 503 });
  const body = await req.json().catch(() => ({}));
  const enc = new TextEncoder();
  const stream = new ReadableStream({
    async start(ctrl) {
      const emit = (s: Step) => ctrl.enqueue(enc.encode(JSON.stringify(s) + "\n"));
      try {
        const state = await view<VaultState>("get_state");
        let story: string[], intent: Intent, source = "", model = "scripted agent", repeat = 1;
        const sc = SCENARIOS.find((s) => s.id === body.scenario);
        if (sc) {
          story = sc.story; intent = sc.intent; source = sc.source ?? ""; repeat = sc.id === "drip" ? 3 : 1;
        } else {
          const task = String(body.task || "").slice(0, 300), content = String(body.content || "").slice(0, 2000);
          if (!task) throw new Error("Give the agent a task");
          emit({ kind: "agent", text: "Thinking…" });
          const a = await naiveAgent(task, content, state);
          story = a.thoughts; intent = a.intent; source = content; model = a.model;
        }
        if (source) emit({ kind: "agent", text: "Reading untrusted content", source });
        for (const t of story) { emit({ kind: "agent", text: t }); await sleep(450); }
        for (let i = 0; i < repeat; i++) {
          const it = repeat > 1 ? { ...intent, memo: `${intent.memo}${i + 1}` } : intent;
          emit({ kind: "intent", intent: it, model });
          const { result: p, hash } = await act<Proposal>("agent", "propose", {
            receiver: it.receiver, amount: toYocto(it.amountNear), method: it.method ?? null,
            args: it.args ? Buffer.from(JSON.stringify(it.args)).toString("base64") : null, gas_tgas: 30, memo: it.memo,
          });
          emit({ kind: "proposed", proposal: p, hash });
          if (p.status === "Waiting" || p.status === "Held") {
            const history = await view<Proposal[]>("get_proposals", { limit: 60 });
            const v = await judge(it, await signals(it, state, history.filter((h) => h.id !== p.id), source), state, story, source);
            emit({ kind: "sentinel", verdict: v });
            const reasons = [`Sentinel ${v.risk}/100: ${v.summary}`.slice(0, 160), ...v.reasons].slice(0, 4);
            const s = await act<Proposal>("sentinel", "score", { id: p.id, risk: v.risk, reasons });
            emit({ kind: "scored", proposal: s.result, hash: s.hash });
          }
          if (i < repeat - 1) { emit({ kind: "agent", text: "…and again." }); await sleep(600); }
        }
      } catch (e) {
        emit({ kind: "error", text: (e as Error).message?.slice(0, 240) || "failed" });
      }
      emit({ kind: "done" });
      ctrl.close();
    },
  });
  return new Response(stream, { headers: { "content-type": "application/x-ndjson", "cache-control": "no-store" } });
}
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
