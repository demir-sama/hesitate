import { view, VAULT, ROLES, hasKeys } from "@/lib/near";
import type { Proposal, VaultState } from "@/lib/types";
export const dynamic = "force-dynamic";
export async function GET() {
  try {
    const [state, proposals] = await Promise.all([view<VaultState>("get_state"), view<Proposal[]>("get_proposals", { limit: 40 })]);
    return Response.json({ vault: VAULT, roles: ROLES, live: hasKeys(), state, proposals });
  } catch (e) { return Response.json({ error: (e as Error).message }, { status: 502 }); }
}
