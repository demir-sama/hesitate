import { act, accountFacts, toNear, view } from "@/lib/near";
import type { Proposal } from "@/lib/types";
export const dynamic = "force-dynamic";
/**
 * Demo owner actions for the public vault. Rejecting is always allowed. Approving is limited so strangers can't
 * drain the shared testnet vault into attacker look-alikes: the real owner signs approvals from their own wallet.
 */
export async function POST(req: Request) {
  const { id, action } = await req.json();
  try {
    const p = await view<Proposal | null>("get_proposal", { id: String(id) });
    if (!p) return Response.json({ error: "No such proposal" }, { status: 404 });
    if (action === "approve") {
      const f = await accountFacts(p.receiver);
      if (!f.exists || toNear(p.amount) > 0.15 || (p.risk ?? 0) >= 60 || p.method)
        return Response.json({ error: "The public demo won't approve risky holds. Connect the owner wallet to approve this one yourself." }, { status: 403 });
    }
    const r = await act<Proposal>("owner", action === "approve" ? "approve" : "reject", { id: String(id) });
    return Response.json(r);
  } catch (e) { return Response.json({ error: (e as Error).message }, { status: 502 }); }
}
