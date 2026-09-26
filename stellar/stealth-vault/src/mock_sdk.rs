//! Minimal `soroban_sdk` stand-in used only when the crate is compiled under
//! Kani (`cfg(kani)`).
//!
//! The real SDK bottoms out in host functions that the model checker cannot
//! see through, so `stealth-vault/Cargo.toml` drops `soroban-sdk` from the
//! dependency graph for `cfg(kani)` and this module supplies the same surface
//! backed by plain Rust data structures. `stealth-vault/src/lib.rs` is compiled
//! verbatim against it — the proofs in `proofs/mod.rs` exercise the real
//! `claim` / `refund` / `refund_permissionless` bodies, not a transcription.
//!
//! Modelling choices, and why they do not weaken the proofs:
//!
//! * `require_auth` is a no-op. The proofs quantify over the *most permissive*
//!   caller, so any invariant that holds here also holds under real auth.
//! * `token::Client::transfer` records a payout instead of moving balances.
//!   The vault's invariants are about which payouts fire and when, not about
//!   SAC bookkeeping.
//! * `crypto().sha256` is a cheap fold rather than the real digest. No proof
//!   depends on hash semantics — they all start from an already-stored deposit.
//! * Events and metric emission are no-ops; they are observational only.
//!
//! The harness is deliberately heap-free: `Env` is zero-sized and all state
//! lives in one `static`, with each storage map a fixed-capacity slot array
//! sized to the keys the contract actually touches. CBMC reasons about symbolic
//! pointers and heap growth far more expensively than about a bounded array, and
//! the bound costs no coverage — it keeps the proofs inside the CI time budget.

use core::marker::PhantomData;

use crate::{DataKey, DepositEntry};

/// Address id reserved for `env.current_contract_address()`.
pub const CONTRACT_ADDRESS_ID: u32 = u32::MAX;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    pub id: u32,
}

impl Address {
    pub fn require_auth(&self) {
        // Mock authorization: no-op under Kani.
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bytes {
    pub data: [u8; 64],
    pub len: usize,
}

impl Bytes {
    pub fn new(_env: &Env) -> Self {
        Bytes {
            data: [0u8; 64],
            len: 0,
        }
    }

    pub fn len(&self) -> u32 {
        self.len as u32
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn from_slice(_env: &Env, data: &[u8]) -> Self {
        let mut buf = [0u8; 64];
        let len = if data.len() <= 64 { data.len() } else { 64 };
        buf[..len].copy_from_slice(&data[..len]);
        Bytes { data: buf, len }
    }

    pub fn append(&mut self, other: &Bytes) {
        let mut i = 0;
        while i < other.len && self.len < 64 {
            self.data[self.len] = other.data[i];
            self.len += 1;
            i += 1;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BytesN<const N: usize> {
    pub data: [u8; N],
}

impl<const N: usize> BytesN<N> {
    pub fn from_array(_env: &Env, data: &[u8; N]) -> Self {
        BytesN { data: *data }
    }

    pub fn get(&self, index: u32) -> Option<u8> {
        let index = index as usize;
        if index < N {
            Some(self.data[index])
        } else {
            None
        }
    }
}

impl<const N: usize> From<BytesN<N>> for Bytes {
    fn from(value: BytesN<N>) -> Bytes {
        let mut buf = [0u8; 64];
        let len = if N <= 64 { N } else { 64 };
        buf[..len].copy_from_slice(&value.data[..len]);
        Bytes { data: buf, len }
    }
}

/// Stand-in for `soroban_sdk::crypto::Hash<32>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hash32 {
    pub data: [u8; 32],
}

impl From<Hash32> for BytesN<32> {
    fn from(value: Hash32) -> BytesN<32> {
        BytesN { data: value.data }
    }
}

/// Values the mock storage can hold, one variant per key shape the contract uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoredValue {
    Addr(Address),
    Bool(bool),
    U32(u32),
    Deposit(DepositEntry),
}

/// Round-trip between a contract-level type and `StoredValue`.
pub trait Storable: Sized {
    fn to_stored(&self) -> StoredValue;
    fn from_stored(value: &StoredValue) -> Option<Self>;
}

impl Storable for Address {
    fn to_stored(&self) -> StoredValue {
        StoredValue::Addr(self.clone())
    }

    fn from_stored(value: &StoredValue) -> Option<Self> {
        match value {
            StoredValue::Addr(address) => Some(address.clone()),
            _ => None,
        }
    }
}

impl Storable for bool {
    fn to_stored(&self) -> StoredValue {
        StoredValue::Bool(*self)
    }

    fn from_stored(value: &StoredValue) -> Option<Self> {
        match value {
            StoredValue::Bool(flag) => Some(*flag),
            _ => None,
        }
    }
}

impl Storable for u32 {
    fn to_stored(&self) -> StoredValue {
        StoredValue::U32(*self)
    }

    fn from_stored(value: &StoredValue) -> Option<Self> {
        match value {
            StoredValue::U32(value) => Some(*value),
            _ => None,
        }
    }
}

impl Storable for DepositEntry {
    fn to_stored(&self) -> StoredValue {
        StoredValue::Deposit(self.clone())
    }

    fn from_stored(value: &StoredValue) -> Option<Self> {
        match value {
            StoredValue::Deposit(entry) => Some(entry.clone()),
            _ => None,
        }
    }
}

/// The contract always passes `&DataKey`; this keeps the two-parameter
/// `get::<_, V>(...)` call shape of the real SDK working.
pub trait AsKey {
    fn as_key(&self) -> &DataKey;
}

impl AsKey for DataKey {
    fn as_key(&self) -> &DataKey {
        self
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct StorageEntry {
    pub key: DataKey,
    pub value: StoredValue,
    pub expiry_ledger: u32,
}

/// Fixed-capacity, allocation-free key/value map.
pub struct Slots<const N: usize> {
    pub slots: [Option<StorageEntry>; N],
}

impl<const N: usize> Slots<N> {
    pub const fn new() -> Self {
        Slots {
            slots: [const { None }; N],
        }
    }

    pub fn get(&self, key: &DataKey) -> Option<&StorageEntry> {
        for slot in self.slots.iter() {
            if let Some(entry) = slot {
                if &entry.key == key {
                    return Some(entry);
                }
            }
        }
        None
    }

    pub fn set(&mut self, key: &DataKey, value: StoredValue) {
        for slot in self.slots.iter_mut() {
            if let Some(entry) = slot {
                if &entry.key == key {
                    entry.value = value;
                    return;
                }
            }
        }
        for slot in self.slots.iter_mut() {
            if slot.is_none() {
                *slot = Some(StorageEntry {
                    key: key.clone(),
                    value,
                    expiry_ledger: 0,
                });
                return;
            }
        }
        panic!("mock storage capacity exceeded");
    }

    pub fn remove(&mut self, key: &DataKey) {
        for slot in self.slots.iter_mut() {
            if let Some(entry) = slot {
                if &entry.key == key {
                    *slot = None;
                    return;
                }
            }
        }
    }

    pub fn extend_ttl(
        &mut self,
        key: &DataKey,
        ledger_sequence: u32,
        threshold: u32,
        extend_to: u32,
    ) {
        for slot in self.slots.iter_mut() {
            if let Some(entry) = slot {
                if &entry.key == key {
                    let threshold_expiry = ledger_sequence.saturating_add(threshold);
                    if entry.expiry_ledger < threshold_expiry {
                        entry.expiry_ledger = ledger_sequence.saturating_add(extend_to);
                    }
                    return;
                }
            }
        }
    }

    /// Number of occupied slots.
    pub fn len(&self) -> usize {
        let mut count = 0;
        for slot in self.slots.iter() {
            if slot.is_some() {
                count += 1;
            }
        }
        count
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<const N: usize> Default for Slots<N> {
    fn default() -> Self {
        Slots::new()
    }
}

/// Instance keys the contract writes: `Admin`, `Announcer`, `GracePeriod`.
pub const INSTANCE_SLOTS: usize = 3;
/// The proofs seed exactly one deposit, which is all any single invocation reads.
pub const PERSISTENT_SLOTS: usize = 1;

pub struct EnvState {
    pub instance: Slots<INSTANCE_SLOTS>,
    pub persistent: Slots<PERSISTENT_SLOTS>,
    /// Number of `token::Client::transfer` calls made so far.
    pub payouts: u32,
    pub ledger_sequence: u32,
}

/// The whole modelled ledger. Soroban invocations are single-threaded, and Kani
/// harnesses run one at a time, so a single `static` is sound here and costs
/// CBMC nothing compared with a reference-counted cell.
static mut STATE: EnvState = EnvState {
    instance: Slots::new(),
    persistent: Slots::new(),
    payouts: 0,
    ledger_sequence: 0,
};

fn state() -> &'static mut EnvState {
    // SAFETY: single-threaded model, one harness at a time.
    unsafe { &mut *core::ptr::addr_of_mut!(STATE) }
}

/// Zero-sized handle onto `STATE`.
#[derive(Clone)]
pub struct Env;

impl Env {
    /// Resets the modelled ledger and sets the current sequence.
    pub fn new(ledger_sequence: u32) -> Self {
        let state = state();
        state.instance = Slots::new();
        state.persistent = Slots::new();
        state.payouts = 0;
        state.ledger_sequence = ledger_sequence;
        Env
    }

    pub fn storage(&self) -> Storage {
        Storage
    }

    pub fn events(&self) -> Events {
        Events
    }

    pub fn ledger(&self) -> Ledger {
        Ledger
    }

    pub fn crypto(&self) -> Crypto {
        Crypto
    }

    pub fn current_contract_address(&self) -> Address {
        Address {
            id: CONTRACT_ADDRESS_ID,
        }
    }

    pub fn invoke_contract<T: Default>(
        &self,
        _contract: &Address,
        _func: &Symbol,
        _args: VecMock<Val>,
    ) -> T {
        // Cross-contract calls are modelled as successful no-ops: Soroban
        // serialises them, so they cannot re-enter this contract.
        T::default()
    }

    /// Number of `transfer` calls made so far. Used by the proofs to show that
    /// a deposit pays out at most once.
    pub fn payout_count(&self) -> u32 {
        state().payouts
    }

    /// Number of live entries in persistent storage.
    pub fn persistent_len(&self) -> usize {
        state().persistent.len()
    }

    /// Seed persistent storage with a single deposit.
    pub fn put_deposit(&self, deposit_id: &BytesN<32>, entry: &DepositEntry) {
        state().persistent.set(
            &DataKey::Deposit(deposit_id.clone()),
            StoredValue::Deposit(entry.clone()),
        );
    }
}

pub struct Ledger;

impl Ledger {
    pub fn sequence(&self) -> u32 {
        state().ledger_sequence
    }
}

pub struct Crypto;

impl Crypto {
    /// Cheap deterministic fold standing in for SHA-256. No proof depends on
    /// the digest's cryptographic properties.
    pub fn sha256(&self, input: &Bytes) -> Hash32 {
        let mut out = [0u8; 32];
        let mut i = 0;
        while i < 32 {
            out[i] = input.data[i] ^ input.data[i + 32] ^ (input.len as u8);
            i += 1;
        }
        Hash32 { data: out }
    }
}

pub struct Storage;

impl Storage {
    pub fn persistent(&self) -> PersistentStorage {
        PersistentStorage
    }

    pub fn instance(&self) -> InstanceStorage {
        InstanceStorage
    }
}

pub struct PersistentStorage;

impl PersistentStorage {
    pub fn set<K: AsKey, V: Storable>(&self, key: &K, value: &V) {
        state().persistent.set(key.as_key(), value.to_stored());
    }

    pub fn get<K: AsKey, V: Storable>(&self, key: &K) -> Option<V> {
        state()
            .persistent
            .get(key.as_key())
            .and_then(|entry| V::from_stored(&entry.value))
    }

    pub fn has<K: AsKey>(&self, key: &K) -> bool {
        state().persistent.get(key.as_key()).is_some()
    }

    pub fn remove<K: AsKey>(&self, key: &K) {
        state().persistent.remove(key.as_key());
    }

    pub fn extend_ttl<K: AsKey>(&self, key: &K, threshold: u32, extend_to: u32) {
        let state = state();
        let ledger_sequence = state.ledger_sequence;
        state
            .persistent
            .extend_ttl(key.as_key(), ledger_sequence, threshold, extend_to);
    }
}

pub struct InstanceStorage;

impl InstanceStorage {
    pub fn set<K: AsKey, V: Storable>(&self, key: &K, value: &V) {
        state().instance.set(key.as_key(), value.to_stored());
    }

    pub fn get<K: AsKey, V: Storable>(&self, key: &K) -> Option<V> {
        state()
            .instance
            .get(key.as_key())
            .and_then(|entry| V::from_stored(&entry.value))
    }

    pub fn has<K: AsKey>(&self, key: &K) -> bool {
        state().instance.get(key.as_key()).is_some()
    }

    pub fn remove<K: AsKey>(&self, key: &K) {
        state().instance.remove(key.as_key());
    }

    pub fn extend_ttl(&self, _threshold: u32, _extend_to: u32) {
        // Instance TTL is not modelled; no proof depends on it.
    }
}

pub struct Events;

impl Events {
    pub fn publish<T, V>(&self, _topics: T, _value: V) {
        // Observational only; no-op under Kani.
    }
}

pub mod token {
    use super::{state, Address, Env};

    pub struct Client;

    impl Client {
        pub fn new(_env: &Env, _asset: &Address) -> Client {
            Client
        }

        /// Counts the payout instead of moving balances. Soroban runs the SAC
        /// call to completion before returning, so it cannot re-enter the vault.
        pub fn transfer(&self, _from: &Address, _to: &Address, _amount: &i128) {
            state().payouts += 1;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Val;

pub trait IntoVal<E, V> {
    fn into_val(&self, env: &E) -> V;
}

impl IntoVal<Env, Val> for u32 {
    fn into_val(&self, _env: &Env) -> Val {
        Val
    }
}

impl IntoVal<Env, Val> for Address {
    fn into_val(&self, _env: &Env) -> Val {
        Val
    }
}

impl IntoVal<Env, Val> for Bytes {
    fn into_val(&self, _env: &Env) -> Val {
        Val
    }
}

impl<const N: usize> IntoVal<Env, Val> for BytesN<N> {
    fn into_val(&self, _env: &Env) -> Val {
        Val
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VecMock<T> {
    _phantom: PhantomData<T>,
}

impl<T> VecMock<T> {
    pub fn new(_env: &Env) -> Self {
        VecMock {
            _phantom: PhantomData,
        }
    }
}

#[macro_export]
macro_rules! mock_vec {
    ($env:expr $(, $x:expr)* $(,)?) => {
        $crate::mock_sdk::VecMock::new($env)
    };
}

#[macro_export]
macro_rules! mock_symbol_short {
    ($str:expr) => {
        $crate::mock_sdk::Symbol
    };
}

pub mod contract_ids {
    use super::Symbol;
    pub const STEALTH_VAULT: Symbol = Symbol;
}

pub mod metric_names {
    use super::Symbol;
    pub const DEPOSIT_COUNT: Symbol = Symbol;
    pub const DEPOSIT_VOLUME: Symbol = Symbol;
    pub const CLAIM_COUNT: Symbol = Symbol;
    pub const REFUND_COUNT: Symbol = Symbol;
}

pub mod dimension_names {
    use super::Symbol;
    pub const ASSET_ADDRESS: Symbol = Symbol;
}

pub fn emit_metric(
    _env: &Env,
    _contract: Symbol,
    _metric_name: Symbol,
    _value: i128,
    _dimensions: VecMock<(Symbol, Val)>,
) {
    // Observational only; no-op under Kani.
}
