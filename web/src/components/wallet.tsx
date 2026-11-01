"use client";
import { useCallback, useEffect, useRef, useState } from "react";

/** Minimal NEAR Connect (HOT Labs) wrapper: testnet sign-in and function calls signed by the user's own wallet. */
export function useWallet() {
  const ref = useRef<any>(null);
  const [accountId, setAccountId] = useState<string | null>(null);
  useEffect(() => {
    let off = false;
    (async () => {
      const { NearConnector } = await import("@hot-labs/near-connect");
      const c = new NearConnector({ network: "testnet" });
      ref.current = c;
      c.on("wallet:signIn", (t: any) => !off && setAccountId(t.accounts?.[0]?.accountId ?? null));
      c.on("wallet:signOut", () => !off && setAccountId(null));
      try { const { accounts } = await c.getConnectedWallet(); if (!off) setAccountId(accounts?.[0]?.accountId ?? null); } catch {}
    })();
    return () => { off = true; };
  }, []);
  const signIn = useCallback(async () => { try { await ref.current?.connect(); } catch {} }, []);
  const signOut = useCallback(async () => { try { await ref.current?.disconnect(); } catch {} setAccountId(null); }, []);
  const call = useCallback(async (contractId: string, methodName: string, args: Record<string, unknown>, depositNear = "0") => {
    const w = await ref.current?.wallet();
    if (!w) throw new Error("Connect a wallet first");
    const deposit = (BigInt(Math.round(Number(depositNear) * 1e6)) * 10n ** 18n).toString();
    return w.signAndSendTransaction({ receiverId: contractId, actions: [{ type: "FunctionCall", params: { methodName, args, gas: "100000000000000", deposit } }] });
  }, []);
  return { accountId, signIn, signOut, call };
}
