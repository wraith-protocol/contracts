/**
 * Matchstick unit tests for mappings/announcer.ts (handleAnnouncement).
 *
 * Fixture values are intentionally identical to those in
 * evm/test/conformance.test.ts so both test layers guard the same wire values.
 *
 * FIXTURE_SCHEME_ID  = 1
 * FIXTURE_EPK        = 0xabab...ab (33 bytes)
 * FIXTURE_METADATA   = 0xfe000000...00 (11 bytes, first byte is view tag 0xfe)
 * FIXTURE_STEALTH_ADDRESS = 0x1234567890AbcdEF1234567890aBcdef12345678
 * FIXTURE_CALLER          = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
 */
import {
  Address,
  BigInt,
  Bytes,
  ethereum,
  log,
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
import { Announcement } from "../generated/ERC5564Announcer/ERC5564Announcer";
import { handleAnnouncement } from "../mappings/announcer";

// ── Fixture constants (must match evm/test/conformance.test.ts) ──────────────

const FIXTURE_SCHEME_ID = BigInt.fromI32(1);
const FIXTURE_STEALTH_ADDRESS = Address.fromString(
  "0x1234567890AbcdEF1234567890aBcdef12345678"
);
const FIXTURE_CALLER = Address.fromString(
  "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
);
// 33 bytes all 0xab — matches FIXTURE_EPK = '0x' + 'ab'.repeat(33)
const FIXTURE_EPK = Bytes.fromHexString(
  "0x" + "ab".repeat(33)
);
// 11 bytes: 0xfe + 10 * 0x00 — matches FIXTURE_METADATA
const FIXTURE_METADATA = Bytes.fromHexString(
  "0xfe" + "00".repeat(10)
);

// ── Helper to build a typed Announcement mock event ──────────────────────────

function createAnnouncementEvent(): Announcement {
  let params: ethereum.EventParam[] = [
    new ethereum.EventParam(
      "schemeId",
      ethereum.Value.fromUnsignedBigInt(FIXTURE_SCHEME_ID)
    ),
    new ethereum.EventParam(
      "stealthAddress",
      ethereum.Value.fromAddress(FIXTURE_STEALTH_ADDRESS)
    ),
    new ethereum.EventParam(
      "caller",
      ethereum.Value.fromAddress(FIXTURE_CALLER)
    ),
    new ethereum.EventParam(
      "ephemeralPubKey",
      ethereum.Value.fromBytes(FIXTURE_EPK)
    ),
    new ethereum.EventParam(
      "metadata",
      ethereum.Value.fromBytes(FIXTURE_METADATA)
    ),
  ];
  return changetype<Announcement>(newMockEventWithParams(params));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("handleAnnouncement", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("creates an Announcement entity with correct schemeId", () => {
    let event = createAnnouncementEvent();
    handleAnnouncement(event);

    // The entity ID is tx_hash-logIndex (default values from newMockEvent).
    let id =
      event.transaction.hash.toHexString() + "-" + event.logIndex.toString();

    assert.fieldEquals("Announcement", id, "schemeId", "1");
  });

  test("stores stealthAddress correctly", () => {
    let event = createAnnouncementEvent();
    handleAnnouncement(event);
    let id =
      event.transaction.hash.toHexString() + "-" + event.logIndex.toString();

    assert.fieldEquals(
      "Announcement",
      id,
      "stealthAddress",
      FIXTURE_STEALTH_ADDRESS.toHexString()
    );
  });

  test("stores caller correctly", () => {
    let event = createAnnouncementEvent();
    handleAnnouncement(event);
    let id =
      event.transaction.hash.toHexString() + "-" + event.logIndex.toString();

    assert.fieldEquals(
      "Announcement",
      id,
      "caller",
      FIXTURE_CALLER.toHexString()
    );
  });

  test("stores ephemeralPubKey correctly", () => {
    let event = createAnnouncementEvent();
    handleAnnouncement(event);
    let id =
      event.transaction.hash.toHexString() + "-" + event.logIndex.toString();

    assert.fieldEquals(
      "Announcement",
      id,
      "ephemeralPubKey",
      FIXTURE_EPK.toHexString()
    );
  });

  test("stores metadata correctly — first byte is view tag 0xfe", () => {
    let event = createAnnouncementEvent();
    handleAnnouncement(event);
    let id =
      event.transaction.hash.toHexString() + "-" + event.logIndex.toString();

    assert.fieldEquals(
      "Announcement",
      id,
      "metadata",
      FIXTURE_METADATA.toHexString()
    );
  });

  test("logIndex is stored correctly", () => {
    let event = createAnnouncementEvent();
    handleAnnouncement(event);
    let id =
      event.transaction.hash.toHexString() + "-" + event.logIndex.toString();

    assert.fieldEquals(
      "Announcement",
      id,
      "logIndex",
      event.logIndex.toString()
    );
  });
});
