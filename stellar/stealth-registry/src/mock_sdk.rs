#[cfg(kani)]
extern crate alloc;

use alloc::rc::Rc;
use core::cell::RefCell;
use core::marker::PhantomData;

pub use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Address {
    pub id: u32,
}

impl Address {
    pub fn require_auth(&self) {
        // Mock authorization: no-op under Kani
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bytes {
    pub data: [u8; 64],
    pub len: usize,
}

impl Bytes {
    pub fn len(&self) -> u32 {
        self.len as u32
    }

    pub fn from_slice(data: &[u8]) -> Self {
        let mut buf = [0u8; 64];
        let len = data.len();
        if len <= 64 {
            buf[..len].copy_from_slice(data);
        }
        Bytes { data: buf, len }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataKey {
    MetaAddress(Address, u32),
}

#[derive(Clone, PartialEq, Eq)]
pub struct StorageEntry {
    pub key: DataKey,
    pub value: Bytes,
    pub expiry_ledger: u32,
}

pub struct EnvState {
    pub storage: Vec<StorageEntry>,
    pub ledger_sequence: u32,
}

#[derive(Clone)]
pub struct Env {
    pub state: Rc<RefCell<EnvState>>,
}

impl Env {
    pub fn new(ledger_sequence: u32) -> Self {
        Env {
            state: Rc::new(RefCell::new(EnvState {
                storage: Vec::new(),
                ledger_sequence,
            })),
        }
    }

    pub fn storage(&self) -> Storage {
        Storage { env: self.clone() }
    }

    pub fn events(&self) -> Events {
        Events { _env: self.clone() }
    }
}

pub struct Storage {
    env: Env,
}

impl Storage {
    pub fn persistent(&self) -> PersistentStorage {
        PersistentStorage {
            env: self.env.clone(),
        }
    }

    pub fn instance(&self) -> InstanceStorage {
        InstanceStorage {
            _env: self.env.clone(),
        }
    }
}

pub struct PersistentStorage {
    env: Env,
}

impl PersistentStorage {
    pub fn set(&self, key: &DataKey, val: &Bytes) {
        let mut state = self.env.state.borrow_mut();
        if let Some(entry) = state.storage.iter_mut().find(|e| &e.key == key) {
            entry.value = val.clone();
        } else {
            state.storage.push(StorageEntry {
                key: key.clone(),
                value: val.clone(),
                expiry_ledger: 0,
            });
        }
    }

    pub fn get(&self, key: &DataKey) -> Option<Bytes> {
        let state = self.env.state.borrow();
        state
            .storage
            .iter()
            .find(|e| &e.key == key)
            .map(|e| e.value.clone())
    }

    pub fn has(&self, key: &DataKey) -> bool {
        let state = self.env.state.borrow();
        state.storage.iter().any(|e| &e.key == key)
    }

    pub fn remove(&self, key: &DataKey) {
        let mut state = self.env.state.borrow_mut();
        state.storage.retain(|e| &e.key != key);
    }

    pub fn extend_ttl(&self, key: &DataKey, threshold: u32, extend_to: u32) {
        let mut state = self.env.state.borrow_mut();
        let ledger_seq = state.ledger_sequence;
        if let Some(entry) = state.storage.iter_mut().find(|e| &e.key == key) {
            let current_expiry = entry.expiry_ledger;
            let threshold_expiry = ledger_seq.saturating_add(threshold);
            if current_expiry < threshold_expiry {
                entry.expiry_ledger = ledger_seq.saturating_add(extend_to);
            }
        }
    }
}

pub struct InstanceStorage {
    _env: Env,
}

impl InstanceStorage {
    pub fn extend_ttl(&self, _threshold: u32, _extend_to: u32) {
        // Mock, no-op
    }
}

pub struct Events {
    _env: Env,
}

impl Events {
    pub fn publish<T, V>(&self, _topics: T, _value: V) {
        // Mock, no-op
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
    ($env:expr, $($x:expr),* $(,)?) => {
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
    pub const STEALTH_REGISTRY: Symbol = Symbol;
}

pub mod metric_names {
    use super::Symbol;
    pub const REGISTER_COUNT: Symbol = Symbol;
    pub const REMOVE_COUNT: Symbol = Symbol;
}

pub mod dimension_names {
    use super::Symbol;
    pub const SCHEME_ID: Symbol = Symbol;
}

pub fn emit_metric(
    _env: &Env,
    _contract: Symbol,
    _metric_name: Symbol,
    _value: i128,
    _dimensions: VecMock<(Symbol, Val)>,
) {
    // Mock, no-op
}
