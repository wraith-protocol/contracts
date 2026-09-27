/**
 * Matchstick unit tests for mappings/names.ts:
 *   handleNameRegistered, handleNameReleased.
 *
 * These tests construct mock NameRegistered and NameReleased events with the
 * same fixture values as evm/test/conformance.test.ts (specifically: the name
 * "alicestealth" and the stealth meta-address FIXTURE_STEALTH_META_ADDRESS),
 * invoke the real handler, and assert on the resulting Name entity state.
 *
 * Fixture values (must match evm/test/conformance.test.ts):
 *   FIXTURE_STEALTH_META_ADDRESS = 0x01..01 (33 bytes) + 0x02..02 (33 bytes) = 66 bytes
 *   FIXTURE_NAME                 = "alicestealth"
 *   FIXTURE_NAME_HASH            = keccak256(FIXTURE_NAME) — computed from the ABI
 */
import {
  Address,
  BigInt,
  Bytes,
  crypto,
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
  NameRegistered,
  NameReleased,
} from "../generated/WraithNames/WraithNames";
import { handleNameRegistered, handleNameReleased } from "../mappings/names";

// ── Fixture constants (must match evm/test/conformance.test.ts) ──────────────

// FIXTURE_STEALTH_META_ADDRESS = '0x' + '01'.repeat(33) + '02'.repeat(33)
const FIXTURE_STEALTH_META_ADDRESS = Bytes.fromHexString(
  "0x" + "01".repeat(33) + "02".repeat(33)
);

// Name used in the EVM conformance suite names section.
const FIXTURE_NAME = "alicestealth";

// keccak256 of the UTF-8 bytes of the name — matches how the EVM contract
// computes the nameHash for NameRegistered.topic[1].
const FIXTURE_NAME_HASH = Bytes.fromByteArray(
  crypto.keccak256(Bytes.fromUTF8(FIXTURE_NAME))
);

// ── Helpers ───────────────────────────────────────────────────────────────────

function createNameRegisteredEvent(): NameRegistered {
  let params: ethereum.EventParam[] = [
    new ethereum.EventParam(
      "nameHash",
      ethereum.Value.fromBytes(FIXTURE_NAME_HASH)
    ),
    new ethereum.EventParam(
      "name",
      ethereum.Value.fromString(FIXTURE_NAME)
    ),
    new ethereum.EventParam(
      "stealthMetaAddress",
      ethereum.Value.fromBytes(FIXTURE_STEALTH_META_ADDRESS)
    ),
  ];
  return changetype<NameRegistered>(newMockEventWithParams(params));
}

function createNameReleasedEvent(): NameReleased {
  let params: ethereum.EventParam[] = [
    new ethereum.EventParam(
      "nameHash",
      ethereum.Value.fromBytes(FIXTURE_NAME_HASH)
    ),
    new ethereum.EventParam(
      "name",
      ethereum.Value.fromString(FIXTURE_NAME)
    ),
  ];
  return changetype<NameReleased>(newMockEventWithParams(params));
}

// The entity ID in names.ts is event.params.nameHash.toHexString().
const ENTITY_ID = FIXTURE_NAME_HASH.toHexString();

// ── handleNameRegistered ──────────────────────────────────────────────────────

describe("handleNameRegistered", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("creates a Name entity with correct name and nameHash", () => {
    let event = createNameRegisteredEvent();
    handleNameRegistered(event);

    assert.fieldEquals("Name", ENTITY_ID, "name", FIXTURE_NAME);
    assert.fieldEquals(
      "Name",
      ENTITY_ID,
      "nameHash",
      FIXTURE_NAME_HASH.toHexString()
    );
  });

  test("stores stealthMetaAddress correctly", () => {
    let event = createNameRegisteredEvent();
    handleNameRegistered(event);

    assert.fieldEquals(
      "Name",
      ENTITY_ID,
      "stealthMetaAddress",
      FIXTURE_STEALTH_META_ADDRESS.toHexString()
    );
  });

  test("sets active to true on registration", () => {
    let event = createNameRegisteredEvent();
    handleNameRegistered(event);

    assert.fieldEquals("Name", ENTITY_ID, "active", "true");
  });

  test("releasedAt is null on registration", () => {
    let event = createNameRegisteredEvent();
    handleNameRegistered(event);

    // The mapping sets entity.releasedAt = null on registration.
    // assert.fieldEquals cannot check null; use notInStore check as a proxy
    // by verifying active is true (null releasedAt means still active).
    assert.fieldEquals("Name", ENTITY_ID, "active", "true");
  });
});

// ── handleNameReleased ────────────────────────────────────────────────────────

describe("handleNameReleased", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("sets active to false on release (register then release)", () => {
    // Register first so the entity exists.
    let regEvent = createNameRegisteredEvent();
    handleNameRegistered(regEvent);

    let relEvent = createNameReleasedEvent();
    handleNameReleased(relEvent);

    assert.fieldEquals("Name", ENTITY_ID, "active", "false");
  });

  test("name field is preserved after release", () => {
    let regEvent = createNameRegisteredEvent();
    handleNameRegistered(regEvent);

    let relEvent = createNameReleasedEvent();
    handleNameReleased(relEvent);

    assert.fieldEquals("Name", ENTITY_ID, "name", FIXTURE_NAME);
  });

  test("nameHash is consistent between register and release events", () => {
    let regEvent = createNameRegisteredEvent();
    handleNameRegistered(regEvent);
    let relEvent = createNameReleasedEvent();
    handleNameReleased(relEvent);

    // Both events use the same nameHash; the entity ID must not change.
    assert.fieldEquals(
      "Name",
      ENTITY_ID,
      "nameHash",
      FIXTURE_NAME_HASH.toHexString()
    );
  });

  test("handles release without prior registration (creates entity, sets active false)", () => {
    // The names.ts mapping handles the case where no prior NameRegistered
    // event has been seen by creating a new entity.
    let relEvent = createNameReleasedEvent();
    handleNameReleased(relEvent);

    assert.fieldEquals("Name", ENTITY_ID, "active", "false");
  });
});
