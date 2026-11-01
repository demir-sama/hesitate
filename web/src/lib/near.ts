import "server-only";
import { Account, JsonRpcProvider, teraToGas, type KeyPairString } from "near-api-js";

export const NETWORK = "testnet";
export const VAULT = process.env.HESITATE_VAULT || "hesitate-vault.testnet";
export const ROLES = { agent: `agent.${VAULT}`, sentinel: `sentinel.${VAULT}`, owner: `owner.${VAULT}` } as const;
export type Role = keyof typeof ROLES;
const KEYS: Record<Role, string | undefined> = { agent: process.env.AGENT_KEY, sentinel: process.env.SENTINEL_KEY, owner: process.env.OWNER_KEY };
export const hasKeys = () => !!(KEYS.agent && KEYS.sentinel);

export const provider = new JsonRpcProvider({ url: process.env.NEAR_RPC || "https://test.rpc.fastnear.com" });
export const YOCTO = 10n ** 24n;
export const toYocto = (near: number) => (BigInt(Math.round(near * 1e6)) * 10n ** 18n).toString();
export const toNear = (y: string | bigint) => Number(BigInt(y) / 10n ** 18n) / 1e6;

export async function view<T>(method: string, args: Record<string, unknown> = {}): Promise<T> {
  return (await provider.callFunction({ contractId: VAULT, method, args })) as T;
}

const accounts: Partial<Record<Role, Account>> = {};
function acct(role: Role) {
  const k = KEYS[role];
  if (!k) throw new Error(`No ${role} key configured`);
  return (accounts[role] ??= new Account(ROLES[role], provider, k as KeyPairString));
}

/** Sign a vault call with a role's function-call access key; returns parsed return value and tx hash. */
export async function act<T>(role: Role, methodName: string, args: Record<string, unknown>): Promise<{ result: T; hash: string }> {
  const r: any = await acct(role).callFunctionRaw({ contractId: VAULT, methodName, args, gas: teraToGas("100"), deposit: 0n, waitUntil: "EXECUTED_OPTIMISTIC" } as any);
  const hash: string = r.transaction?.hash ?? r.transaction_outcome?.id;
  const fail = r.status?.Failure;
  if (fail) throw new Error(panicText(fail));
  const b64 = r.status?.SuccessValue;
  return { result: (b64 ? JSON.parse(Buffer.from(b64, "base64").toString()) : undefined) as T, hash };
}
function panicText(f: any): string {
  const s = JSON.stringify(f);
  const m = s.match(/Smart contract panicked: ([^"\\]+)/);
  return m ? m[1] : s.slice(0, 200);
}

export type AccountFacts = { exists: boolean; hasContract: boolean; balance?: number; storage?: number; codeHash?: string };
export async function accountFacts(id: string): Promise<AccountFacts> {
  try {
    const a: any = await provider.viewAccount({ accountId: id } as any);
    const code = a.code_hash ?? a.codeHash;
    return { exists: true, hasContract: !!code && code !== "11111111111111111111111111111111", balance: toNear(String(a.amount)), storage: Number(a.storage_usage ?? a.storageUsage ?? 0), codeHash: code };
  } catch { return { exists: false, hasContract: false }; }
}

export const explorerTx = (h: string) => `https://testnet.nearblocks.io/txns/${h}`;
export const explorerAcct = (a: string) => `https://testnet.nearblocks.io/address/${a}`;
