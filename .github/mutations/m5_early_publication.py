from pathlib import Path

path = Path("crates/oregon-mempool/src/pool.rs")
source = path.read_text()
old = """        let (mut plan, _) =
            self.prepare_admission(transaction, chain_base, chain_utxos, verifier)?;
        let new_total_bytes = self.plan_capacity(&mut plan)?;"""
new = """        let (mut plan, _) =
            self.prepare_admission(transaction, chain_base, chain_utxos, verifier)?;
        self.total_bytes = self
            .total_bytes
            .saturating_add(plan.candidate.entry.encoded_bytes);
        let new_total_bytes = self.plan_capacity(&mut plan)?;"""
if old not in source:
    raise SystemExit("mutation anchor missing")
path.write_text(source.replace(old, new, 1))
