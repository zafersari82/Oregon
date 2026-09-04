from pathlib import Path

path = Path("crates/oregon-mempool/src/pool.rs")
source = path.read_text()

old_direct = "        let mut direct_parents = BTreeSet::new();"
new_direct = (
    "        let mut bypass_missing_dependency = false;\n"
    "        let mut direct_parents = BTreeSet::new();"
)
old_parent = """            let Some(parent) = self.entries.get(&outpoint.txid) else {
                return Err(MempoolError::MissingDependency(*outpoint));
            };"""
new_parent = """            let Some(parent) = self.entries.get(&outpoint.txid) else {
                bypass_missing_dependency = true;
                continue;
            };"""
old_fee = """        let fee =
            validation_state.apply_normal_transaction(&transaction, spend_height, verifier)?;"""
new_fee = """        let fee = if bypass_missing_dependency {
            0
        } else {
            validation_state.apply_normal_transaction(&transaction, spend_height, verifier)?
        };"""

for needle in (old_direct, old_parent, old_fee):
    if needle not in source:
        raise SystemExit(f"mutation anchor missing: {needle!r}")

source = source.replace(old_direct, new_direct, 1)
source = source.replace(old_parent, new_parent, 1)
source = source.replace(old_fee, new_fee, 1)
path.write_text(source)
