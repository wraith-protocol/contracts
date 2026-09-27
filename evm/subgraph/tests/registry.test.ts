/**
 * Matchstick unit tests for mappings/registry.ts:
 *   handleStealthMetaAddressSet, handleNonceIncremented.
 *
 * These tests invoke the real handler functions with mock event objects and
 * assert on the resulting StealthMetaAddress entity state.
 *
 * Fixture values (must match evm/test/conformance.test.ts):
 *   FIXTURE_SCHEME_ID            = 1
 *   FIXTURE_STEALTH_META_ADDRESS = 0x01..01 (33 bytes) + 0x02..02 (33 bytes) = 66 bytes
 *   FIXTURE_REGISTRANT           = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
 */
import {
  Address,
  BigInt,
  Bytes,
  ethereum,
} from "@graphprotocol/graph-ts";
import {
  assert,
  describe,
  test,
  beforeEach,
  afterEach,
  newMockEventWithParams,
  clearStore,
} from "matchstick-as";
import {
  StealthMetaAddressSet,
  NonceIncremented,
} from "../generated/ERC6538Registry/ERC6538Registry";
import {
  handleStealthMetaAddressSet,
  handleNonceIncremented,
} from "../mappings/registry";

// ── Fixture constants (must match evm/test/conformance.test.ts) ──────────────

const FIXTURE_SCHEME_ID = BigInt.fromI32(1);
const FIXTURE_STEALTH_META_ADDRESS = Bytes.fromHexString(
  "0x" + "01".repeat(33) + "02".repeat(33)
);
const FIXTURE_REGISTRANT = Address.fromString(
  "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
);

// Entity ID: registrant.toHexString() + '-' + schemeId.toString()
// This mirrors the id formula in registry.ts's handleStealthMetaAddressSet.
const ENTITY_ID =
  FIXTURE_REGISTRANT.toHexString() + "-" + FIXTURE_SCHEME_ID.toString();

// ── Helpers ───────────────────────────────────────────────────────────────────

function createStealthMetaAddressSetEvent(): StealthMetaAddressSet {
  let params: ethereum.EventParam[] = [
    new ethereum.EventParam(
      "registrant",
      ethereum.Value.fromAddress(FIXTURE_REGISTRANT)
    ),
    new ethereum.EventParam(
      "schemeId",
      ethereum.Value.fromUnsignedBigInt(FIXTURE_SCHEME_ID)
    ),
    new ethereum.EventParam(
      "stealthMetaAddress",
      ethereum.Value.fromBytes(FIXTURE_STEALTH_META_ADDRESS)
    ),
  ];
  return changetype<StealthMetaAddressSet>(newMockEventWithParams(params));
}

function createNonceIncrementedEvent(nonce: BigInt): NonceIncremented {
  let params: ethereum.EventParam[] = [
    new ethereum.EventParam(
      "registrant",
      ethereum.Value.fromAddress(FIXTURE_REGISTRANT)
    ),
    new ethereum.EventParam(
      "newNonce",
      ethereum.Value.fromUnsignedBigInt(nonce)
    ),
  ];
  return changetype<NonceIncremented>(newMockEventWithParams(params));
}

// ── handleStealthMetaAddressSet ───────────────────────────────────────────────

describe("handleStealthMetaAddressSet", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("creates a StealthMetaAddress entity with correct registrant and schemeId", () => {
    let event = createStealthMetaAddressSetEvent();
    handleStealthMetaAddressSet(event);

    assert.fieldEquals(
      "StealthMetaAddress",
      ENTITY_ID,
      "registrant",
      FIXTURE_REGISTRANT.toHexString()
    );
    assert.fieldEquals(
      "StealthMetaAddress",
      ENTITY_ID,
      "schemeId",
      FIXTURE_SCHEME_ID.toString()
    );
  });

  test("stores 66-byte stealthMetaAddress correctly", () => {
    let event = createStealthMetaAddressSetEvent();
    handleStealthMetaAddressSet(event);

    assert.fieldEquals(
      "StealthMetaAddress",
      ENTITY_ID,
      "stealthMetaAddress",
      FIXTURE_STEALTH_META_ADDRESS.toHexString()
    );
  });

  test("calling twice with different meta-address updates the entity (upsert behavior)", () => {
    // First registration
    let event1 = createStealthMetaAddressSetEvent();
    handleStealthMetaAddressSet(event1);

    // Update with a different meta-address
    let newMetaAddress = Bytes.fromHexString("0x" + "03".repeat(33) + "04".repeat(33));
    let params2: ethereum.EventParam[] = [
      new ethereum.EventParam(
        "registrant",
        ethereum.Value.fromAddress(FIXTURE_REGISTRANT)
      ),
      new ethereum.EventParam(
        "schemeId",
        ethereum.Value.fromUnsignedBigInt(FIXTURE_SCHEME_ID)
      ),
      new ethereum.EventParam(
        "stealthMetaAddress",
        ethereum.Value.fromBytes(newMetaAddress)
      ),
    ];
    let event2 = changetype<StealthMetaAddressSet>(
      newMockEventWithParams(params2)
    );
    handleStealthMetaAddressSet(event2);

    // Entity must reflect the new meta-address
    assert.fieldEquals(
      "StealthMetaAddress",
      ENTITY_ID,
      "stealthMetaAddress",
      newMetaAddress.toHexString()
    );
  });
});

// ── handleNonceIncremented ────────────────────────────────────────────────────
//
// registry.ts's handleNonceIncremented is intentionally a no-op (it does not
// write any entity).  Testing here confirms the handler is callable and does
// not crash — future schemes can attach nonce history by modifying it.

describe("handleNonceIncremented (no-op handler)", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("handleNonceIncremented runs without error and writes no entity", () => {
    let event = createNonceIncrementedEvent(BigInt.fromI32(1));
    handleNonceIncremented(event);

    // No StealthMetaAddress entity should have been created by the nonce handler.
    assert.notInStore("StealthMetaAddress", ENTITY_ID);
  });
});
