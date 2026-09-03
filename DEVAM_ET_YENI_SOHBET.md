# OREGON — Yeni Sohbet Devam Dosyası

Tarih: 3 Eylül 2026
Repo: `zafersari82/Radium`
Aktif geliştirme branch'i: `oregon-v1-m3-utxo-chainstate`
Bu handoff hazırlanırken gerçek geliştirme HEAD'i: `0d2699033c5c6457c8582ca60ec96975bd524855`
Son commit: `test: add RED normal transaction cardinality rules`
Son CI: `33734396725` — **beklenen RED / FAILURE**

> YENİ AJAN İÇİN EN ÖNEMLİ KURAL: Bu paket ve GitHub branch'i bir RED TDD noktasındadır. CI'nın kırık olması regresyon demek değildir. Önce `0d269903...` head'ini ve RED testlerini incele, sonra yalnız bu testleri GREEN yapacak minimal implementation ile devam et. Main'e merge etme.

---

## 1. Projenin amacı

OREGON (OREG), Bitcoin/Radium kodunu kopyalamayan, Rust ile sıfırdan yazılan bağımsız ve ciddi bir Proof-of-Work L1/native coin projesidir. Amaç meme coin değil; uzun ömürlü, denetlenebilir, küçük ve açık consensus çekirdeği olan bir ağdır.

### Donmuş para politikası

- İsim: **OREGON**
- Ticker: **OREG**
- Consensus: **Proof of Work**
- Arz üst zarfı: **1,000,000 OREG**
- Founder allocation: **50,000 OREG (%5)**
- Mining envelope: **950,000 OREG**
- Integer subsidy schedule'ın üretebildiği toplam mining issuance: **949,999.97 OREG**
- Founder + mining scheduled issuance: **999,999.97 OREG**
- Ulaşılamayan slack: **0.03 OREG**
- 1 OREG = **100,000,000 base unit**
- İlk subsidy: **2.375 OREG**
- Subsidy başlangıcı: height 1
- Halving: **200,000 blok**
- Block target: **300 saniye / 5 dakika**
- Founder allocation: yalnız height 1 coinbase output 0'da; tam 50,000 OREG
- Founder tax/treasury/dev fee/admin mint yok
- Fee'lerin %100'ü miner'a
- Underclaim geçerli, overclaim consensus-invalid
- Founder output için özel vesting yok; normal coinbase maturity'ye tabi

### Donmuş coinbase maturity

- **120 blok**
- 5 dakikalık bloklarla yaklaşık 10 saat
- Founder output dahil bütün coinbase output'lara aynı kural

---

## 2. Donmuş genel mimari

Fresh Rust workspace:

```text
crates/
  oregon-primitives
  oregon-consensus
  oregon-pow
  oregon-utxo        # M3, şu an aktif
vendor/
  RandomX            # exact upstream gitlink
```

Uzun vadeli hedefte ayrıca chain/storage/mempool/p2p/sync/mining/rpc/node crate'leri gelecek.

Bitcoin/Radium sadece tarihsel referans; implementation kopyası değildir. `.cpp/.h` consensus implementation'ı Oregon Rust çekirdeğine taşınmamalı.

---

## 3. Tamamlanmış foundation (v0 primitives)

Accepted foundation checkpoint branch:
`oregon-v0-checkpoint-foundation-accepted-2026-09-02`

Accepted foundation SHA:
`033160dafd4c2a74cd6dcfa2bb7b628c3cab499c`

Temel primitive'ler:

- `Amount(u64)`, checked arithmetic, float yok
- `Hash256([u8;32])`
- BLAKE3 domain separation
- Canonical little-endian encoding
- Canonical varint
- Transaction model:
  - version u16
  - inputs
  - outputs
  - lock_time u64
- TxID witness'i de commit eder
- Merkle:
  - domain-separated leaf/node
  - odd leaf **duplicate edilmez**, unchanged promote edilir
- Block header tam **114 byte**
- Header fields:
  - version
  - previous_block
  - transaction_root
  - timestamp
  - difficulty_commitment [u8;32]
  - nonce u64

Foundation mutation testi odd-Merkle davranışını gerçekten yakalamıştır.

---

## 4. M1 — Consensus Core — ACCEPTED

Accepted recovery branch:
`oregon-v1-checkpoint-m1-consensus-core-accepted-2026-09-03`

Final M1 SHA:
`52ae48c216aed8881aba6a556a06f71e02e4b464`

M1'de tamamlananlar:

- Full 256-bit little-endian `Target`
- `POW_LIMIT`, `INITIAL_TARGET` parametre modeli
- Exact emission schedule
- Height 1 founder allocation / coinbase kuralları
- 200,000 block halving
- Per-block fixed-point ASERT
- Target block time 300s
- ASERT half-life 21,600s / 6 saat
- MTP-11
- Exact cumulative chain work
- Pre-PoW header context validation
- 1 MiB max block
- 100 KiB max transaction
- Merkle validation
- Tek coinbase / normal transaction'da null-outpoint yasağı

Mutation kanıtları:
- Halving off-by-one mutation test suite'i kırdı
- ASERT 21,600→21,601 mutation test suite'i kırdı

---

## 5. M2 — RandomX PoW — ACCEPTED

Accepted recovery branch:
`oregon-v1-m2-checkpoint-randomx-accepted-2026-09-03`

Acceptance record commit:
`ee83a3062e06b9447d091872fe77bd37eeee1f4d`

Reviewed code head:
`44d22ae112b9182cf054aa9faa3426a66770b7ae`

### RandomX sabitleri

- RandomX v2.0.1 exact upstream commit:
  `aaafe71322df6602c21a5c72937ac284724ae561`
- Oregon Argon salt:
  `OREGON-RANDOMX-V1`
- Upstream source repo değiştirilmez; salt build copy'de uygulanır
- Key epoch: **864 blok** (~3 gün)
- Activation delay: **24 blok** (~2 saat)
- Key domain:
  `OREGON/RANDOMX-KEY/V1\0 || key_block_id`
- PoW input:
  `OREGON/POW/V1\0 || canonical_114_byte_header`
- RandomX hash ve Target little-endian unsigned 256-bit olarak karşılaştırılır

### M2 güvenlik sertleştirmeleri

- Caller/miner key-block ID seçemez
- Validator candidate height'tan gerekli key height'ı kendi hesaplar
- Key block ID validated-chain source üzerinden gelir
- `LightEngine` immutable key binding taşır
- `FullEngine` tam RandomX dataset kullanır
- PoW validation, M1 pre-validation token'ı (`PrePowHeaderFacts`) olmadan çağrılamaz
- Token exact header ID + height + validated target/work taşır
- Header değiştirilirse token mismatch olur
- CI checkout action exact immutable SHA'ya pinli

### M2 önemli CI kanıtları

- Normal CI: `33729550987` SUCCESS
- x64 + ARM64 frozen vector: `33729550956` SUCCESS
- Full↔Light x64 + ARM64: `33729551071` SUCCESS
- Acceptance docs sonrası CI: `33729745858` SUCCESS
- Endian mutation run: `33726007274` EXPECTED FAILURE

Frozen RandomX hash:
`c33bcaf498accad910ed40a346ac3820700496b2ead640ead6892cb01332143c`

---

## 6. M3 — UTXO / CHAINSTATE — AKTİF

Plan:
`docs/superpowers/plans/2026-09-03-oregon-m3-utxo-chainstate.md`

Branch:
`oregon-v1-m3-utxo-chainstate`

Handoff gerçek HEAD:
`0d2699033c5c6457c8582ca60ec96975bd524855`

Bu milestone'un amacı:

- UTXO state transition
- coinbase maturity
- fee accounting
- same-block topological spends
- whole-block atomicity
- deterministic `BlockUndo`
- reorg disconnect

Bu milestone **BIP340/KeyCommitV1 gerçek kriptografisini uygulamıyor**. Production state engine her normal input için zorunlu `SpendVerifier` trait ister. Tests'te `AcceptAll/RejectAll` test-only verifier kullanılabilir; production permissive verifier oluşturma.

### Mevcut `oregon-utxo` tasarımı

`UtxoEntry`:

- `output: TxOutput`
- `creation_height: u64`
- `is_coinbase: bool`

`COINBASE_MATURITY = 120`

`UtxoState`:

- HashMap<OutPoint,UtxoEntry>
- normal transaction apply öncesinde bütün input/output/authorization kontrollerini yapmalı
- validation fail olduğunda partial mutation yapmamalı
- duplicate input reject
- missing UTXO reject
- immature coinbase reject
- outputs > inputs reject
- checked arithmetic
- output collision reject

### M3 Task durumu

Plan 6 task'tır:

1. UTXO Entry / errors / mandatory verifier
2. Normal transaction transition + fee accounting
3. Coinbase metadata + 120 maturity
4. Atomic block connect + same-block order + coinbase fee binding
5. Deterministic disconnect / undo
6. Mutation/security acceptance + checkpoint

Branch history Task 1–4 üzerinde ilerlemiştir; ancak şu anda yeni bir RED cardinality hardening halkasında durmaktadır. Head commit mesajı özellikle:

`test: add RED normal transaction cardinality rules`

Son CI:
`33734396725` → **FAILURE bekleniyor**.

### ŞU ANKİ TAM RED TESTLER

Dosya:
`crates/oregon-consensus/tests/block_skeleton.rs`

Mevcut ek iki RED test:

- `skeleton_rejects_normal_transaction_without_inputs`
  - beklenen error: `ConsensusError::EmptyNormalTransactionInputs(1)`

- `skeleton_rejects_normal_transaction_without_outputs`
  - beklenen error: `ConsensusError::EmptyNormalTransactionOutputs(1)`

Yani yeni ajan ilk olarak şunu yapmalı:

1. `oregon-v1-m3-utxo-chainstate` branch'inin **0d269903...** head'inde olduğunu doğrula.
2. CI run `33734396725` logunu oku ve failure'ın bu yeni RED cardinality error/API'lerinden geldiğini doğrula.
3. `oregon-consensus/src/error.rs` ve block skeleton validator'ı incele.
4. Minimal GREEN yap:
   - normal transaction (`block.transactions[1..]`) en az 1 input taşımalı
   - en az 1 output taşımalı
   - error index transaction'ın block index'i olmalı
   - coinbase bu iki normal-tx cardinality kuralından etkilenmemeli
   - skeleton structural checks fee/accounting bilmeden çalışmalı
5. Existing `validate_non_genesis_block_structure` ile yeni skeleton API arasında duplicated rule bırakma. Tek authoritative skeleton function olsun; fee-bound validation onu çağırıp sonra `validate_coinbase(... fees ...)` yapmalı.
6. Run full CI: workspace tests + rustfmt + clippy `-D warnings`.
7. GREEN olmadan Task 5'e geçme.

### Daha önce M3 sırasında yakalanmış tasarım noktaları

- `validate_non_genesis_block_structure` coinbase fee doğrulamasını çok erken yapıyordu; bu nedenle skeleton/fee-bound ayrımı başlatıldı.
- Block connect live state'i final validation'dan önce değiştirmemeli; clone/overlay üzerinde çalışmalı.
- Same-block parent→child spend geçerli.
- Child-before-parent `InvalidBlockOrder` olmalı.
- Aynı seed UTXO iki ayrı tx tarafından harcanırsa bütün block invalid ve live state unchanged.
- Coinbase miner claim exact accumulated fees'e bağlanmalı.
- Output collision sessiz HashMap overwrite olmamalı.
- Undo üretimi HashMap iteration sırasına bırakılmamalı; deterministic olmalı.

---

## 7. M3'ten sonra yapılacaklar

Cardinality GREEN sonrası plan sırasından sapma:

- Task 4 atomik block connection'ı fresh reviewer gate ile tamamen kapat
- Task 5 `disconnect_block(BlockUndo)` RED→GREEN
- connect→disconnect exact state equality
- same-block spend chain undo correctness
- tampered undo reject
- Task 6 mutation tests:
  1. maturity off-by-one
  2. duplicate-input check kaldırma
  3. block overlay early-commit
- Fresh final CI
- M2 accepted → M3 diff security review
- `docs/checkpoints/OREGON_V1_M3_UTXO_CHAINSTATE.md`
- recovery branch:
  `oregon-v1-checkpoint-m3-utxo-chainstate-accepted-2026-09-03`
- Main'e merge YOK

---

## 8. Kesin çalışma yöntemi

Kullanıcı bütün mikro-onayları önceden verdi. Normal güvenli teknik kararlar için her adımda kullanıcıya tekrar onay sorma; çalışmaya devam et. Ancak private key/seed gibi geri dönüşsüz sır veya mainnet launch secret gerektiren bir aşamaya gelirse dur.

Her consensus görevi:

1. RED test
2. Gerçek CI failure kanıtı
3. Minimal GREEN implementation
4. Fresh CI
5. Reviewer gate
6. Checkpoint

Başarı iddiası fresh test kanıtı olmadan yapılmaz.

Main branch'e merge etme; kullanıcı açıkça istemedikçe recovery/development branch üzerinde ilerle.

Mutation branch'teki kötü kodu gerçek execution branch'e taşımama.

---

## 9. Yeni sohbette kullanıcıya söylenecek ilk şey

Kullanıcı ZIP'i yüklediğinde uzun açıklama yapma. Şunu söyleyip işe başla:

> "Paketi ve DEVAM_ET_YENI_SOHBET.md dosyasını okudum. Oregon M1 ve M2 accepted; M3 branch'i 0d269903... head'inde bilinçli RED durumda. İlk iş normal transaction input/output cardinality RED testlerini minimal GREEN yapıp CI ile doğrulayacağım; main'e merge etmeyeceğim."

Sonra gerçekten dosyaları/branch'i doğrula ve çalışmaya başla.

---

## 10. Önemli dosyalar

- `docs/superpowers/specs/2026-09-02-oregon-pow-consensus-v1-design.md`
- `docs/superpowers/plans/2026-09-03-oregon-m1-consensus-core.md`
- `docs/superpowers/plans/2026-09-03-oregon-m2-randomx-pow.md`
- `docs/superpowers/plans/2026-09-03-oregon-m3-utxo-chainstate.md`
- `docs/checkpoints/OREGON_V0_PROTOCOL_FOUNDATION.md`
- `docs/checkpoints/OREGON_V1_M2_RANDOMX_POW_BRIDGE.md`
- `crates/oregon-primitives/`
- `crates/oregon-consensus/`
- `crates/oregon-pow/`
- `crates/oregon-utxo/`
- `.github/workflows/oregon-rust.yml`
- `.github/workflows/oregon-randomx-vector.yml`
- `.github/workflows/oregon-randomx-full-light.yml`

---

## 11. Bu handoff'un doğruluk çıpası

Gerçek development branch:
`oregon-v1-m3-utxo-chainstate`

Gerçek development HEAD bu handoff hazırlanırken:
`0d2699033c5c6457c8582ca60ec96975bd524855`

Eğer yeni sohbette GitHub branch head'i bundan ilerideyse, **ZIP'e körü körüne dönme**. Önce GitHub'daki daha yeni commitleri incele; user'ın başka bir oturumda ilerlemiş olması mümkündür. Daima en yeni doğrulanmış development branch'i baz al.
