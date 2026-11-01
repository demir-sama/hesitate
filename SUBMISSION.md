# NEAR Hackathon submission draft: Hesitate

**Project name:** Hesitate
**Track:** AI Security & Defense
**Tagline:** A wallet that pauses before your AI agent does something stupid.
**Repo:** https://github.com/demir-sama/hesitate
**Demo:** https://hesitate-sama-5806.vercel.app
**Contract:** hesitate-vault.testnet

**Description:**
Agents with wallets can be prompt-injected. Hesitate is a NEAR vault contract plus an AI sentinel. The agent only holds a function-call key that can `propose`. The contract refuses denylisted receivers and self-calls outright, and holds key grabs and anything over the caps. Every other transaction waits a few seconds while a sentinel (rules + LLM, fail-closed, only able to tighten) scores it. Look-alike receivers, injected instructions, nonexistent accounts and payment bursts get held for the owner. Every step in the live demo is a real testnet transaction you can open on NearBlocks.

**X post draft (Lee posts it with a screen recording):**
My AI agent just tried to pay a scammer because a README told it to. It didn't get the money out.

Hesitate is a NEAR vault that makes agents pause: on-chain caps and denylists, function-call keys, and an AI sentinel that can only say "wait".

Live on testnet: https://hesitate-sama-5806.vercel.app
@NEARProtocol #NEARHackathon
