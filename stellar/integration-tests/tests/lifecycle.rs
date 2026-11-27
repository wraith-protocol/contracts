//! One deterministic pass through the whole Stellar protocol: a recipient
//! registers a meta-address, a sender pays a stealth address through
//! stealth-sender, the recipient finds the payment from the announcement alone
//! and withdraws it, and the same spending key then registers, resolves and
//! releases a `.wraith` name.
//!
//! Keys come from fixed seeds, so every address in the run is reproducible.
//! The stealth derivation mirrors `generateStealthAddress` and
//! `deriveStealthPrivateScalar` in `@wraith-protocol/sdk/chains/stellar`, and
//! the expected stealth address is pinned to the value the SDK produces for the
//! same seeds.
//!
//! Wallet-side calls (mint, `register_keys`, `send`) use `mock_auths` scoped to
//! one address and one invocation. The stealth account and the name owner are
//! ed25519 accounts, so their calls are authorized with real signatures: the
//! withdrawal only succeeds if the key recovered by the scan is the key that
//! controls the stealth account.
//!
//! Run with:
//!   cargo test --package integration-tests --test lifecycle

use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;

use curve25519_dalek::{EdwardsPoint, Scalar};
use ed25519_dalek::hazmat::{raw_sign, ExpandedSecretKey};
use ed25519_dalek::{Sha512, SigningKey, VerifyingKey};
use soroban_sdk::testutils::{Address as _, Events as _, MockAuth, MockAuthInvoke};
use soroban_sdk::token::{StellarAssetClient, TokenClient};
use soroban_sdk::xdr::{
    AccountEntry, AccountEntryExt, AccountId, AlphaNum4, AssetCode4, Hash, HashIdPreimage,
    HashIdPreimageSorobanAuthorization, InvokeContractArgs, LedgerEntry, LedgerEntryData,
    LedgerEntryExt, LedgerKey, LedgerKeyAccount, LedgerKeyTrustLine, Limits, PublicKey, ScAddress,
    ScBytes, ScMap, ScMapEntry, ScSymbol, ScVal, ScVec, SequenceNumber, SorobanAddressCredentials,
    SorobanAuthorizationEntry, SorobanAuthorizedFunction, SorobanAuthorizedInvocation,
    SorobanCredentials, Thresholds, TrustLineAsset, TrustLineEntry, TrustLineEntryExt,
    TrustLineFlags, Uint256, WriteXdr,
};
use soroban_sdk::{
    symbol_short, vec, Address, Bytes, BytesN, Env, FromVal, IntoVal, String, TryFromVal, Val, Vec,
};

use stealth_announcer::{StealthAnnouncerContract, METADATA_KIND_VIEW_TAG, STELLAR_V2_SCHEME_ID};
use stealth_registry::{StealthRegistryContract, StealthRegistryContractClient};
use stealth_sender::{StealthSenderContract, StealthSenderContractClient, WithdrawalEntry};
use wraith_names::{NamesError, WraithNamesContract, WraithNamesContractClient};

const SPENDING_SEED: [u8; 32] = [0x5e; 32];
const VIEWING_SEED: [u8; 32] = [0x7e; 32];
const EPHEMERAL_SEED: [u8; 32] = [0xe9; 32];

/// `generateStealthAddress(K, V, EPHEMERAL_SEED)` from @wraith-protocol/sdk
/// 1.4.5 for the seeds above.
const SDK_STEALTH_ADDRESS: &str = "GB5YUWXYVCF3DQ3YC2MXINXIBNC6F7HVBDXUGLFNT3KZVWUQPIBGTRGV";
const SDK_VIEW_TAG: u8 = 166;

const SEND_AMOUNT: i128 = 1_000;
const NAME: &str = "alice";

/// Runs one lifecycle step and names it in the panic message if it fails.
fn step<T>(name: &str, f: impl FnOnce() -> T) -> T {
    panic::catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|cause| {
        let detail = cause
            .downcast_ref::<std::string::String>()
            .map(std::string::String::as_str)
            .or_else(|| cause.downcast_ref::<&str>().copied())
            .unwrap_or("non-string panic payload");
        panic!("lifecycle step `{name}` failed: {detail}")
    })
}

// ── Keys and stealth derivation ──────────────────────────────────────────────

/// An ed25519 signing key held as a scalar, so seed-derived keys and the
/// derived stealth key sign the same way.
struct Key {
    secret: ExpandedSecretKey,
    public: VerifyingKey,
}

impl Key {
    fn from_seed(seed: &[u8; 32]) -> Self {
        let secret = ExpandedSecretKey::from(seed);
        let public = VerifyingKey::from(&secret);
        Self { secret, public }
    }

    /// Matches the SDK's `signWithScalar`, which uses SHA-256 of the scalar as
    /// the nonce prefix because a derived scalar has no seed.
    fn from_scalar(env: &Env, scalar: Scalar) -> Self {
        let hash_prefix = sha256(env, &[&scalar.to_bytes()]);
        let secret = ExpandedSecretKey {
            scalar,
            hash_prefix,
        };
        let public = VerifyingKey::from(&secret);
        Self { secret, public }
    }

    fn sign(&self, message: &[u8]) -> [u8; 64] {
        raw_sign::<Sha512>(&self.secret, message, &self.public).to_bytes()
    }

    fn account_id(&self) -> AccountId {
        account_id(self.public.to_bytes())
    }

    fn address(&self, env: &Env) -> Address {
        Address::try_from_val(env, &ScAddress::Account(self.account_id())).unwrap()
    }
}

fn account_id(public_key: [u8; 32]) -> AccountId {
    AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(public_key)))
}

fn sha256(env: &Env, parts: &[&[u8]]) -> [u8; 32] {
    let mut input = Bytes::new(env);
    for part in parts {
        input.extend_from_slice(part);
    }
    env.crypto().sha256(&input).to_array()
}

/// X25519 between an ed25519 seed and an ed25519 public key, as the SDK's
/// `computeSharedSecret` does after converting both to Montgomery form.
fn shared_secret(seed: &[u8; 32], public: &VerifyingKey) -> [u8; 32] {
    let scalar = SigningKey::from_bytes(seed).to_scalar_bytes();
    public.to_montgomery().mul_clamped(scalar).to_bytes()
}

fn hash_to_scalar(env: &Env, shared_secret: &[u8; 32]) -> Scalar {
    Scalar::from_bytes_mod_order(sha256(env, &[b"wraith:scalar:", shared_secret]))
}

fn view_tag(env: &Env, ephemeral: &[u8; 32], viewing: &VerifyingKey) -> u8 {
    sha256(
        env,
        &[
            b"wraith:stellar:view-tag:v2:",
            ephemeral,
            viewing.as_bytes(),
        ],
    )[0]
}

/// `K_spend + hash_scalar * G`.
fn stealth_public_key(spending: &VerifyingKey, hash_scalar: &Scalar) -> [u8; 32] {
    let point = spending.to_edwards() + EdwardsPoint::mul_base(hash_scalar);
    point.compress().to_bytes()
}

fn meta_address(env: &Env, spending: &VerifyingKey, viewing: &VerifyingKey) -> Bytes {
    let mut meta = Bytes::from_slice(env, spending.as_bytes());
    meta.extend_from_slice(viewing.as_bytes());
    meta
}

/// Splits a registered 64-byte meta-address back into its two public keys.
fn decode_meta_address(meta: &Bytes) -> (VerifyingKey, VerifyingKey) {
    assert_eq!(meta.len(), 64, "meta-address must be 64 bytes");
    let mut raw = [0u8; 64];
    meta.copy_into_slice(&mut raw);
    let spending = VerifyingKey::from_bytes(raw[..32].try_into().unwrap()).unwrap();
    let viewing = VerifyingKey::from_bytes(raw[32..].try_into().unwrap()).unwrap();
    (spending, viewing)
}

// ── Ledger fixtures ──────────────────────────────────────────────────────────

/// Writes a classic account entry whose master key is its only signer.
fn open_account(env: &Env, account: AccountId) {
    let key = LedgerKey::Account(LedgerKeyAccount {
        account_id: account.clone(),
    });
    let entry = LedgerEntry {
        last_modified_ledger_seq: 0,
        data: LedgerEntryData::Account(AccountEntry {
            account_id: account,
            balance: 0,
            seq_num: SequenceNumber(0),
            num_sub_entries: 0,
            inflation_dest: None,
            flags: 0,
            home_domain: Default::default(),
            thresholds: Thresholds([1, 1, 1, 1]),
            signers: Default::default(),
            ext: AccountEntryExt::V0,
        }),
        ext: LedgerEntryExt::V0,
    };
    env.host()
        .add_ledger_entry(&Rc::new(key), &Rc::new(entry), None)
        .unwrap();
}

/// Writes an authorized trustline so `account` can hold a classic asset.
fn open_trustline(env: &Env, account: AccountId, asset: TrustLineAsset) {
    let key = LedgerKey::Trustline(LedgerKeyTrustLine {
        account_id: account.clone(),
        asset: asset.clone(),
    });
    let entry = LedgerEntry {
        last_modified_ledger_seq: 0,
        data: LedgerEntryData::Trustline(TrustLineEntry {
            account_id: account,
            asset,
            balance: 0,
            limit: i64::MAX,
            flags: TrustLineFlags::AuthorizedFlag as u32,
            ext: TrustLineEntryExt::V0,
        }),
        ext: LedgerEntryExt::V0,
    };
    env.host()
        .add_ledger_entry(&Rc::new(key), &Rc::new(entry), None)
        .unwrap();
}

/// The classic asset behind a Stellar Asset Contract registered by
/// `register_stellar_asset_contract_v2`.
fn trustline_asset(env: &Env, token: &Address, issuer: &Address) -> TrustLineAsset {
    let code = TokenClient::new(env, token).symbol();
    let mut asset_code = [0u8; 4];
    code.copy_into_slice(&mut asset_code[..code.len() as usize]);
    let ScAddress::Account(issuer) = ScAddress::from(issuer) else {
        panic!("asset issuer must be an account");
    };
    TrustLineAsset::CreditAlphanum4(AlphaNum4 {
        asset_code: AssetCode4(asset_code),
        issuer,
    })
}

// ── Signed authorization ─────────────────────────────────────────────────────

fn invocation(
    env: &Env,
    contract: &Address,
    function: &str,
    args: Vec<Val>,
    sub_invocations: std::vec::Vec<SorobanAuthorizedInvocation>,
) -> SorobanAuthorizedInvocation {
    let args: std::vec::Vec<ScVal> = args
        .iter()
        .map(|arg| ScVal::try_from_val(env, &arg).unwrap())
        .collect();
    SorobanAuthorizedInvocation {
        function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
            contract_address: contract.into(),
            function_name: ScSymbol(function.try_into().unwrap()),
            args: args.try_into().unwrap(),
        }),
        sub_invocations: sub_invocations.try_into().unwrap(),
    }
}

/// Builds the authorization entry an ed25519 account signs for `root`, in the
/// form stellar-core expects: a signature over the hash of the
/// `ENVELOPE_TYPE_SOROBAN_AUTHORIZATION` preimage.
fn signed_auth(
    env: &Env,
    signer: &Key,
    nonce: i64,
    root: SorobanAuthorizedInvocation,
) -> SorobanAuthorizationEntry {
    let signature_expiration_ledger = env.ledger().sequence() + 100;
    let preimage = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
        network_id: Hash(env.ledger().network_id().to_array()),
        nonce,
        signature_expiration_ledger,
        invocation: root.clone(),
    });
    let payload = sha256(env, &[&preimage.to_xdr(Limits::none()).unwrap()]);
    let signature = signer.sign(&payload);

    let entry = ScMap(
        [
            ScMapEntry {
                key: ScVal::Symbol(ScSymbol("public_key".try_into().unwrap())),
                val: ScVal::Bytes(ScBytes(
                    signer.public.to_bytes().to_vec().try_into().unwrap(),
                )),
            },
            ScMapEntry {
                key: ScVal::Symbol(ScSymbol("signature".try_into().unwrap())),
                val: ScVal::Bytes(ScBytes(signature.to_vec().try_into().unwrap())),
            },
        ]
        .try_into()
        .unwrap(),
    );
    let signatures = ScVal::Vec(Some(ScVec([ScVal::Map(Some(entry))].try_into().unwrap())));

    SorobanAuthorizationEntry {
        credentials: SorobanCredentials::Address(SorobanAddressCredentials {
            address: ScAddress::Account(signer.account_id()),
            nonce,
            signature_expiration_ledger,
            signature: signatures,
        }),
        root_invocation: root,
    }
}

// ── The lifecycle ────────────────────────────────────────────────────────────

struct Announcement {
    scheme_id: u32,
    view_tag_bucket: u32,
    metadata_kind: u32,
    stealth_address: Address,
    ephemeral_pub_key: BytesN<32>,
    metadata: Bytes,
}

#[test]
fn stellar_stealth_payment_lifecycle() {
    let env = Env::default();

    let spending = Key::from_seed(&SPENDING_SEED);
    let viewing = Key::from_seed(&VIEWING_SEED);
    let ephemeral = Key::from_seed(&EPHEMERAL_SEED);

    let (announcer_id, registry, sender, names) = step("deploy contracts", || {
        let announcer_id = env.register(StealthAnnouncerContract, ());
        let registry =
            StealthRegistryContractClient::new(&env, &env.register(StealthRegistryContract, ()));
        let sender =
            StealthSenderContractClient::new(&env, &env.register(StealthSenderContract, ()));
        sender.init(&announcer_id, &None, &None, &0, &Address::generate(&env));
        let names = WraithNamesContractClient::new(&env, &env.register(WraithNamesContract, ()));
        names.init(&Address::generate(&env));
        (announcer_id, registry, sender, names)
    });

    let recipient_wallet = Address::generate(&env);
    let meta = meta_address(&env, &spending.public, &viewing.public);

    step("register meta-address", || {
        registry
            .mock_auths(&[MockAuth {
                address: &recipient_wallet,
                invoke: &MockAuthInvoke {
                    contract: &registry.address,
                    fn_name: "register_keys",
                    args: (&recipient_wallet, STELLAR_V2_SCHEME_ID, meta.clone()).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .register_keys(&recipient_wallet, &STELLAR_V2_SCHEME_ID, &meta);
        assert_eq!(
            registry.stealth_meta_address_of(&recipient_wallet, &STELLAR_V2_SCHEME_ID),
            meta
        );
    });

    let asset_admin = Address::generate(&env);
    let payer = Address::generate(&env);
    let (token, asset) = step("register test asset", || {
        let sac = env.register_stellar_asset_contract_v2(asset_admin.clone());
        let token = sac.address();
        StellarAssetClient::new(&env, &token)
            .mock_auths(&[MockAuth {
                address: &asset_admin,
                invoke: &MockAuthInvoke {
                    contract: &token,
                    fn_name: "mint",
                    args: (&payer, SEND_AMOUNT).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .mint(&payer, &SEND_AMOUNT);
        let asset = trustline_asset(&env, &token, &sac.issuer().address());
        (token, asset)
    });
    let token_client = TokenClient::new(&env, &token);

    // Sender side: everything here is derived from the registry entry and the
    // ephemeral key, never from the recipient's secrets.
    let (stealth_address, ephemeral_pub_key, tag) = step("derive stealth address", || {
        let (spending_pub, viewing_pub) = decode_meta_address(
            &registry.stealth_meta_address_of(&recipient_wallet, &STELLAR_V2_SCHEME_ID),
        );
        let hash_scalar = hash_to_scalar(&env, &shared_secret(&EPHEMERAL_SEED, &viewing_pub));
        let stealth_pub = stealth_public_key(&spending_pub, &hash_scalar);
        let address =
            Address::try_from_val(&env, &ScAddress::Account(account_id(stealth_pub))).unwrap();
        let tag = view_tag(&env, ephemeral.public.as_bytes(), &viewing_pub);

        assert_eq!(
            address.to_string(),
            String::from_str(&env, SDK_STEALTH_ADDRESS)
        );
        assert_eq!(tag, SDK_VIEW_TAG);
        (
            address,
            BytesN::from_array(&env, ephemeral.public.as_bytes()),
            tag,
        )
    });

    // A never-used stealth account has no trustline for a classic asset, and
    // only the stealth key could sign one, so a SAC transfer to it fails with
    // "trustline entry is missing". The SDK's classic payment builder avoids
    // this with a claimable balance; a contract transfer has no such path, so
    // the account is opened here with its trustline already in place.
    let ScAddress::Account(stealth_account) = ScAddress::from(&stealth_address) else {
        unreachable!("stealth addresses are ed25519 accounts");
    };
    open_account(&env, stealth_account.clone());
    open_trustline(&env, stealth_account, asset);

    let metadata = Bytes::from_slice(&env, &[tag, 0xab, 0xcd]);
    let announcement = step("send through stealth-sender", || {
        sender
            .mock_auths(&[MockAuth {
                address: &payer,
                invoke: &MockAuthInvoke {
                    contract: &sender.address,
                    fn_name: "send",
                    args: (
                        &payer,
                        &token,
                        SEND_AMOUNT,
                        STELLAR_V2_SCHEME_ID,
                        &stealth_address,
                        &ephemeral_pub_key,
                        metadata.clone(),
                    )
                        .into_val(&env),
                    sub_invokes: &[MockAuthInvoke {
                        contract: &token,
                        fn_name: "transfer",
                        args: (&payer, &stealth_address, SEND_AMOUNT).into_val(&env),
                        sub_invokes: &[],
                    }],
                },
            }])
            .send(
                &payer,
                &token,
                &SEND_AMOUNT,
                &STELLAR_V2_SCHEME_ID,
                &stealth_address,
                &ephemeral_pub_key,
                &metadata,
            );

        let announcements: std::vec::Vec<_> = env
            .events()
            .all()
            .iter()
            .filter(|(contract, _, _)| *contract == announcer_id)
            .collect();
        assert_eq!(announcements.len(), 1, "expected exactly one announcement");
        let (_, topics, data) = announcements[0].clone();
        assert_eq!(topics.len(), 4);
        let first_topic: Val = topics.get_unchecked(0);
        assert!(first_topic.shallow_eq(&symbol_short!("announce").to_val()));
        let (stealth_address, ephemeral_pub_key, metadata) = FromVal::from_val(&env, &data);
        Announcement {
            scheme_id: u32::from_val(&env, &topics.get_unchecked(1)),
            view_tag_bucket: u32::from_val(&env, &topics.get_unchecked(2)),
            metadata_kind: u32::from_val(&env, &topics.get_unchecked(3)),
            stealth_address,
            ephemeral_pub_key,
            metadata,
        }
    });

    step("check announcement", || {
        assert_eq!(announcement.scheme_id, STELLAR_V2_SCHEME_ID);
        assert_eq!(announcement.metadata_kind, METADATA_KIND_VIEW_TAG);
        assert_eq!(announcement.view_tag_bucket, tag as u32);
        assert_eq!(announcement.metadata, metadata);
        assert_eq!(announcement.ephemeral_pub_key, ephemeral_pub_key);
        assert_eq!(announcement.stealth_address, stealth_address);
        assert_eq!(token_client.balance(&stealth_address), SEND_AMOUNT);
        assert_eq!(token_client.balance(&payer), 0);
    });

    // Receiver side: only the announcement, the viewing seed and the spending
    // key are used to find the payment and rebuild its key.
    let stealth_key = step("scan announcement", || {
        let ephemeral = announcement.ephemeral_pub_key.to_array();
        assert_eq!(
            view_tag(&env, &ephemeral, &viewing.public) as u32,
            announcement.view_tag_bucket,
            "view tag prefilter rejected our own payment"
        );
        let ephemeral = VerifyingKey::from_bytes(&ephemeral).unwrap();
        let hash_scalar = hash_to_scalar(&env, &shared_secret(&VIEWING_SEED, &ephemeral));
        let stealth_pub = stealth_public_key(&spending.public, &hash_scalar);
        let found =
            Address::try_from_val(&env, &ScAddress::Account(account_id(stealth_pub))).unwrap();
        assert_eq!(found, announcement.stealth_address);

        let key = Key::from_scalar(&env, spending.secret.scalar + hash_scalar);
        assert_eq!(
            key.public.to_bytes(),
            stealth_pub,
            "stealth scalar does not match"
        );
        key
    });

    let destination = Address::generate(&env);
    step("withdraw from stealth account", || {
        let entries = vec![
            &env,
            WithdrawalEntry {
                token: token.clone(),
                to: destination.clone(),
                amount: SEND_AMOUNT,
            },
        ];
        let transfer = invocation(
            &env,
            &token,
            "transfer",
            (&stealth_address, &destination, SEND_AMOUNT).into_val(&env),
            std::vec![],
        );
        let root = invocation(
            &env,
            &sender.address,
            "withdraw_many",
            (&stealth_address, entries.clone()).into_val(&env),
            std::vec![transfer],
        );
        sender
            .set_auths(&[signed_auth(&env, &stealth_key, 1, root)])
            .withdraw_many(&stealth_address, &entries);

        assert_eq!(token_client.balance(&stealth_address), 0);
        assert_eq!(token_client.balance(&destination), SEND_AMOUNT);
    });

    // The name is owned by the spending key's own account, so the same key
    // that controls the meta-address controls the name.
    let owner = spending.address(&env);
    open_account(&env, spending.account_id());
    let name = String::from_str(&env, NAME);

    step("register name", || {
        let root = invocation(
            &env,
            &names.address,
            "register",
            (&owner, name.clone(), meta.clone()).into_val(&env),
            std::vec![],
        );
        names
            .set_auths(&[signed_auth(&env, &spending, 1, root)])
            .register(&owner, &name, &meta);
    });

    step("resolve name", || {
        assert_eq!(names.resolve(&name), meta);
        assert_eq!(names.name_of(&meta), name);
    });

    step("release name", || {
        let root = invocation(
            &env,
            &names.address,
            "release",
            (&owner, name.clone()).into_val(&env),
            std::vec![],
        );
        names
            .set_auths(&[signed_auth(&env, &spending, 2, root)])
            .release(&owner, &name);
        assert_eq!(names.try_resolve(&name), Err(Ok(NamesError::NameNotFound)));
        assert_eq!(names.try_name_of(&meta), Err(Ok(NamesError::NameNotFound)));
    });
}
