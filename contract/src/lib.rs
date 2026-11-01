//! Hesitate — a vault that makes an AI agent's on-chain hands pause.
//!
//! * The **agent** can only `propose` payments / calls (give it a function-call access key limited to `propose`).
//! * Deterministic **policy** runs on-chain first: denylist → refused; over caps → held for the owner;
//!   allowlisted + small → paid instantly; everything else waits in a short **hesitation window**.
//! * During the window a **sentinel** (off-chain AI risk engine) attaches a risk score. It can only *tighten*:
//!   a low score releases the payment early, a high score holds it for the owner. It can never move funds elsewhere.
//! * If nobody scores it, `fail_closed` decides: hold (default) or allow release after the window.
//! * The **owner** approves or rejects held proposals and edits policy.

use near_sdk::json_types::{Base64VecU8, U128, U64};
use near_sdk::store::{IterableMap, IterableSet};
use near_sdk::{env, near, require, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise};

#[near(serializers = [borsh])]
#[derive(BorshStorageKey)]
enum Key { Proposals, Allow, Deny }

#[near(serializers = [borsh, json])]
#[derive(Clone, PartialEq, Debug)]
pub enum Status { Waiting, Cleared, Held, Executed, Rejected, Refused }

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Proposal {
    pub id: U64,
    pub receiver: AccountId,
    pub amount: U128,
    pub method: Option<String>,
    pub args: Option<Base64VecU8>,
    pub gas_tgas: u64,
    pub memo: String,
    pub created_ns: U64,
    pub status: Status,
    pub risk: Option<u8>,
    pub reasons: Vec<String>,
    pub decided_by: Option<AccountId>,
}

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Policy {
    pub per_tx_cap: U128,
    pub daily_cap: U128,
    pub instant_max: U128,
    pub window_sec: u64,
    pub hold_at_risk: u8,
    pub fail_closed: bool,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Vault {
    owner: AccountId,
    agent: AccountId,
    sentinel: AccountId,
    policy: Policy,
    allow: IterableSet<AccountId>,
    deny: IterableSet<AccountId>,
    proposals: IterableMap<u64, Proposal>,
    next_id: u64,
    day: u64,
    spent_today: u128,
    paused: bool,
}

const DAY_NS: u64 = 86_400_000_000_000;
const MAX_GAS_TGAS: u64 = 100;
const KEEP: usize = 200; // proposals kept in state

#[near]
impl Vault {
    #[init]
    pub fn new(owner: AccountId, agent: AccountId, sentinel: AccountId, policy: Policy) -> Self {
        Self { owner, agent, sentinel, policy, allow: IterableSet::new(Key::Allow), deny: IterableSet::new(Key::Deny), proposals: IterableMap::new(Key::Proposals), next_id: 0, day: 0, spent_today: 0, paused: false }
    }

    /// Anyone may top up the vault.
    #[payable]
    pub fn fund(&mut self) {}

    // ---------- agent ----------
    pub fn propose(&mut self, receiver: AccountId, amount: U128, method: Option<String>, args: Option<Base64VecU8>, gas_tgas: Option<u64>, memo: Option<String>) -> Proposal {
        require!(env::predecessor_account_id() == self.agent, "only the agent can propose");
        require!(!self.paused, "vault is paused");
        let gas_tgas = gas_tgas.unwrap_or(30).min(MAX_GAS_TGAS);
        let id = self.next_id; self.next_id += 1;
        let mut p = Proposal { id: id.into(), receiver: receiver.clone(), amount, method: method.clone(), args, gas_tgas, memo: memo.unwrap_or_default().chars().take(140).collect(), created_ns: env::block_timestamp().into(), status: Status::Waiting, risk: None, reasons: vec![], decided_by: None };
        let amt = amount.0;
        self.roll_day();

        if receiver == env::current_account_id() {
            p.status = Status::Refused; p.reasons.push("calls back into the vault itself".into());
        } else if self.deny.contains(&receiver) {
            p.status = Status::Refused; p.reasons.push(format!("{} is on the denylist", receiver));
        } else if matches!(method.as_deref(), Some(m) if is_dangerous(m)) {
            p.status = Status::Held; p.reasons.push(format!("method `{}` can hand over control", method.clone().unwrap()));
        } else if amt > self.policy.per_tx_cap.0 {
            p.status = Status::Held; p.reasons.push("over the per-transaction cap".into());
        } else if self.spent_today + amt > self.policy.daily_cap.0 {
            p.status = Status::Held; p.reasons.push("would exceed today's cap".into());
        } else if self.allow.contains(&receiver) && amt <= self.policy.instant_max.0 && method.is_none() {
            p.reasons.push("allowlisted and small: paid instantly".into());
            p.status = Status::Executed; p.decided_by = Some(env::current_account_id());
            self.insert(p.clone());
            self.execute(&p);
            return p;
        } else {
            p.reasons.push(format!("waiting {}s for the sentinel", self.policy.window_sec));
        }
        self.insert(p.clone());
        p
    }

    // ---------- sentinel (can only tighten) ----------
    pub fn score(&mut self, id: U64, risk: u8, reasons: Vec<String>) -> Proposal {
        require!(env::predecessor_account_id() == self.sentinel, "only the sentinel can score");
        let mut p = self.get_mut(id.0);
        require!(p.status == Status::Waiting || p.status == Status::Held, "already decided");
        p.risk = Some(risk.min(100));
        p.reasons.extend(reasons.into_iter().take(6).map(|r| r.chars().take(160).collect::<String>()));
        if risk >= self.policy.hold_at_risk {
            p.status = Status::Held;
        } else if p.status == Status::Waiting {
            // only a low score on a Waiting (not policy-held) proposal releases it
            self.roll_day();
            if self.spent_today + p.amount.0 <= self.policy.daily_cap.0 {
                p.status = Status::Executed; p.decided_by = Some(self.sentinel.clone());
                self.proposals.insert(id.0, p.clone());
                self.execute(&p);
                return p;
            }
            p.status = Status::Held; p.reasons.push("would exceed today's cap".into());
        }
        self.proposals.insert(id.0, p.clone());
        p
    }

    /// After the window, anyone may release an unscored proposal — only if the owner chose fail-open.
    pub fn release(&mut self, id: U64) -> Proposal {
        let mut p = self.get_mut(id.0);
        require!(p.status == Status::Waiting, "not waiting");
        require!(env::block_timestamp() >= p.created_ns.0 + self.policy.window_sec * 1_000_000_000, "still hesitating");
        self.roll_day();
        if self.policy.fail_closed || self.spent_today + p.amount.0 > self.policy.daily_cap.0 {
            p.status = Status::Held; p.reasons.push("no sentinel verdict in time: held (fail-closed)".into());
        } else {
            p.status = Status::Executed; p.decided_by = Some(env::predecessor_account_id());
            self.proposals.insert(id.0, p.clone());
            self.execute(&p);
            return p;
        }
        self.proposals.insert(id.0, p.clone());
        p
    }

    // ---------- owner ----------
    pub fn approve(&mut self, id: U64) -> Proposal {
        self.only_owner();
        let mut p = self.get_mut(id.0);
        require!(p.status == Status::Held || p.status == Status::Waiting, "nothing to approve");
        self.roll_day();
        p.status = Status::Executed; p.decided_by = Some(self.owner.clone());
        self.proposals.insert(id.0, p.clone());
        self.execute(&p);
        p
    }
    pub fn reject(&mut self, id: U64) -> Proposal {
        self.only_owner();
        let mut p = self.get_mut(id.0);
        require!(p.status == Status::Held || p.status == Status::Waiting, "nothing to reject");
        p.status = Status::Rejected; p.decided_by = Some(self.owner.clone());
        self.proposals.insert(id.0, p.clone());
        p
    }
    pub fn set_policy(&mut self, policy: Policy) { self.only_owner(); require!(policy.instant_max.0 <= policy.per_tx_cap.0, "instant_max > per_tx_cap"); self.policy = policy; }
    pub fn set_lists(&mut self, allow_add: Vec<AccountId>, allow_remove: Vec<AccountId>, deny_add: Vec<AccountId>, deny_remove: Vec<AccountId>) {
        self.only_owner();
        for a in allow_add { self.deny.remove(&a); self.allow.insert(a); }
        for a in allow_remove { self.allow.remove(&a); }
        for a in deny_add { self.allow.remove(&a); self.deny.insert(a); }
        for a in deny_remove { self.deny.remove(&a); }
    }
    /// The sentinel may add to the denylist (tighten) but never remove.
    pub fn sentinel_deny(&mut self, account: AccountId) {
        require!(env::predecessor_account_id() == self.sentinel, "only the sentinel");
        require!(account != self.owner, "cannot deny the owner");
        self.allow.remove(&account); self.deny.insert(account);
    }
    pub fn set_roles(&mut self, owner: Option<AccountId>, agent: Option<AccountId>, sentinel: Option<AccountId>) {
        self.only_owner();
        if let Some(o) = owner { self.owner = o; }
        if let Some(a) = agent { self.agent = a; }
        if let Some(s) = sentinel { self.sentinel = s; }
    }
    pub fn set_paused(&mut self, paused: bool) { self.only_owner(); self.paused = paused; }
    pub fn withdraw(&mut self, amount: U128) -> Promise { self.only_owner(); Promise::new(self.owner.clone()).transfer(NearToken::from_yoctonear(amount.0)) }

    // ---------- views ----------
    pub fn get_state(&self) -> serde_json_state::State {
        serde_json_state::State { owner: self.owner.clone(), agent: self.agent.clone(), sentinel: self.sentinel.clone(), policy: self.policy.clone(), allow: self.allow.iter().cloned().collect(), deny: self.deny.iter().cloned().collect(), spent_today: U128(if env::block_timestamp() / DAY_NS == self.day { self.spent_today } else { 0 }), balance: U128(env::account_balance().as_yoctonear()), paused: self.paused, next_id: U64(self.next_id) }
    }
    pub fn get_proposals(&self, from: Option<U64>, limit: Option<u32>) -> Vec<Proposal> {
        let end = from.map(|f| f.0 + 1).unwrap_or(self.next_id).min(self.next_id);
        let lim = limit.unwrap_or(30).min(100) as u64;
        (end.saturating_sub(lim)..end).rev().filter_map(|i| self.proposals.get(&i).cloned()).collect()
    }
    pub fn get_proposal(&self, id: U64) -> Option<Proposal> { self.proposals.get(&id.0).cloned() }
}

impl Vault {
    fn only_owner(&self) { require!(env::predecessor_account_id() == self.owner, "only the owner"); }
    fn get_mut(&self, id: u64) -> Proposal { self.proposals.get(&id).cloned().unwrap_or_else(|| env::panic_str("no such proposal")) }
    fn roll_day(&mut self) { let d = env::block_timestamp() / DAY_NS; if d != self.day { self.day = d; self.spent_today = 0; } }
    fn insert(&mut self, p: Proposal) {
        let id = p.id.0;
        self.proposals.insert(id, p);
        if id as usize >= KEEP { self.proposals.remove(&(id - KEEP as u64)); }
    }
    fn execute(&mut self, p: &Proposal) {
        self.spent_today += p.amount.0;
        let promise = Promise::new(p.receiver.clone());
        let amt = NearToken::from_yoctonear(p.amount.0);
        match &p.method {
            None => { promise.transfer(amt); }
            Some(m) => { promise.function_call(m.clone(), p.args.clone().map(|a| a.0).unwrap_or_default(), amt, Gas::from_tgas(p.gas_tgas)); }
        }
        env::log_str(&format!("EVENT_JSON:{{\"standard\":\"hesitate\",\"version\":\"1.0.0\",\"event\":\"executed\",\"data\":{{\"id\":{},\"receiver\":\"{}\",\"amount\":\"{}\"}}}}", p.id.0, p.receiver, p.amount.0));
    }
}

/// Methods that can transfer control of an account or all of its tokens.
fn is_dangerous(m: &str) -> bool {
    let m = m.to_ascii_lowercase();
    ["add_full_access_key", "add_key", "delete_account", "delete_key", "deploy", "set_owner", "transfer_ownership", "nft_approve", "ft_approve", "approve_all", "storage_withdraw", "multisig", "upgrade"].iter().any(|d| m.contains(d))
}

pub mod serde_json_state {
    use super::*;
    #[near(serializers = [json])]
    pub struct State { pub owner: AccountId, pub agent: AccountId, pub sentinel: AccountId, pub policy: Policy, pub allow: Vec<AccountId>, pub deny: Vec<AccountId>, pub spent_today: U128, pub balance: U128, pub paused: bool, pub next_id: U64 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;
    const N: u128 = 1_000_000_000_000_000_000_000_000;
    fn ctx(who: usize) { let mut c = VMContextBuilder::new(); c.current_account_id("vault.testnet".parse().unwrap()).predecessor_account_id(accounts(who)).account_balance(NearToken::from_near(10)); testing_env!(c.build()); }
    fn vault() -> Vault {
        ctx(0);
        let mut v = Vault::new(accounts(0), accounts(1), accounts(2), Policy { per_tx_cap: U128(N), daily_cap: U128(2 * N), instant_max: U128(N / 10), window_sec: 20, hold_at_risk: 60, fail_closed: true });
        v.set_lists(vec!["shop.testnet".parse().unwrap()], vec![], vec!["drainer.testnet".parse().unwrap()], vec![]);
        v
    }
    #[test] fn denylist_refused() { let mut v = vault(); ctx(1); let p = v.propose("drainer.testnet".parse().unwrap(), U128(1), None, None, None, None); assert_eq!(p.status, Status::Refused); }
    #[test] fn allowlisted_small_instant() { let mut v = vault(); ctx(1); let p = v.propose("shop.testnet".parse().unwrap(), U128(N / 20), None, None, None, None); assert_eq!(p.status, Status::Executed); }
    #[test] fn over_cap_held() { let mut v = vault(); ctx(1); let p = v.propose("x.testnet".parse().unwrap(), U128(N * 3 / 2), None, None, None, None); assert_eq!(p.status, Status::Held); }
    #[test] fn dangerous_method_held() { let mut v = vault(); ctx(1); let p = v.propose("x.testnet".parse().unwrap(), U128(0), Some("add_full_access_key".into()), None, None, None); assert_eq!(p.status, Status::Held); }
    #[test] fn sentinel_high_holds_low_releases() {
        let mut v = vault(); ctx(1);
        let a = v.propose("x.testnet".parse().unwrap(), U128(N / 2), None, None, None, None);
        let b = v.propose("y.testnet".parse().unwrap(), U128(N / 2), None, None, None, None);
        assert_eq!(a.status, Status::Waiting);
        ctx(2);
        assert_eq!(v.score(a.id, 90, vec!["drain pattern".into()]).status, Status::Held);
        assert_eq!(v.score(b.id, 10, vec![]).status, Status::Executed);
    }
    #[test] #[should_panic(expected = "only the agent")] fn stranger_cannot_propose() { let mut v = vault(); ctx(3); v.propose("x.testnet".parse().unwrap(), U128(1), None, None, None, None); }
    #[test] #[should_panic(expected = "only the sentinel")] fn agent_cannot_score() { let mut v = vault(); ctx(1); let a = v.propose("x.testnet".parse().unwrap(), U128(1), None, None, None, None); v.score(a.id, 0, vec![]); }
    #[test] fn sentinel_cannot_release_policy_hold() { let mut v = vault(); ctx(1); let a = v.propose("x.testnet".parse().unwrap(), U128(N * 3 / 2), None, None, None, None); ctx(2); assert_eq!(v.score(a.id, 0, vec![]).status, Status::Held); }
    #[test] fn owner_approves_held() { let mut v = vault(); ctx(1); let a = v.propose("x.testnet".parse().unwrap(), U128(N * 3 / 2), None, None, None, None); ctx(0); assert_eq!(v.approve(a.id).status, Status::Executed); }
}
