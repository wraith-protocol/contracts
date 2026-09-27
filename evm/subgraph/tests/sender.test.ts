/**
 * Matchstick unit tests for mappings/sender.ts:
 *   handleSendETH, handleSendERC20, handleBatchSendETH, handleBatchSendERC20.
 *
 * WraithSender emits NO on-chain events; the subgraph uses call handlers.
 * These tests construct mock ethereum.Call objects with the same fixture values
 * used in evm/test/conformance.test.ts and assert on the resulting Send entity
 * fields — directly exercising the mapping code paths that read call.inputs.*,
 * call.from, and call.transaction.value.
 *
 * Fixture values (must match evm/test/conformance.test.ts):
 *   FIXTURE_SCHEME_ID          = 1
 *   FIXTURE_STEALTH_ADDRESS    = 0x1234567890AbcdEF1234567890aBcdef12345678
 *   FIXTURE_EPK                = 0xabab...ab (33 bytes)
 *   FIXTURE_METADATA           = 0xfe000000...00 (11 bytes)
 *   FIXTURE_TOKEN_AMOUNT       = 500 * 10^18
 *   FIXTURE_TOKEN_ADDRESS      = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48 (USDC-like)
 *   FIXTURE_SENDER             = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
 *   FIXTURE_ETH_AMOUNT         = 50000000000000000 (0.05 ETH, same as integration test)
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
  newMockCallWithIO,
  clearStore,
} from "matchstick-as";
import {
  SendETHCall,
  SendERC20Call,
  BatchSendETHCall,
  BatchSendERC20Call,
} from "../generated/WraithSender/WraithSender";
import {
  handleSendETH,
  handleSendERC20,
  handleBatchSendETH,
  handleBatchSendERC20,
} from "../mappings/sender";

// ── Fixture constants (must match evm/test/conformance.test.ts) ──────────────

const FIXTURE_SCHEME_ID = BigInt.fromI32(1);
const FIXTURE_STEALTH_ADDRESS = Address.fromString(
  "0x1234567890AbcdEF1234567890aBcdef12345678"
);
const FIXTURE_EPK = Bytes.fromHexString("0x" + "ab".repeat(33));
const FIXTURE_METADATA = Bytes.fromHexString("0xfe" + "00".repeat(10));
// 500 * 10^18 as a BigInt
const FIXTURE_TOKEN_AMOUNT = BigInt.fromString(
  "500000000000000000000"
);
// Token address used in sender tests (same type as used in conformance.test.ts)
const FIXTURE_TOKEN_ADDRESS = Address.fromString(
  "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
);
const FIXTURE_SENDER = Address.fromString(
  "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
);
// 0.05 ETH in wei — matches FIXTURE_ETH_AMOUNT used in the integration test
const FIXTURE_ETH_AMOUNT = BigInt.fromString("50000000000000000");

// ── Helper: produce an ID matching what sender.ts's sendId() generates ───────
// sendId = tx_hash.toHexString() + '-' + tx_index.toString()

function mockSendId(call: ethereum.Call): string {
  return (
    call.transaction.hash.toHexString() +
    "-" +
    call.transaction.index.toString()
  );
}

// ── handleSendETH ─────────────────────────────────────────────────────────────

describe("handleSendETH", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("creates a Send entity with correct schemeId and stealthAddress", () => {
    let call = changetype<SendETHCall>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "schemeId",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SCHEME_ID)
          ),
          new ethereum.EventParam(
            "stealthAddress",
            ethereum.Value.fromAddress(FIXTURE_STEALTH_ADDRESS)
          ),
          new ethereum.EventParam(
            "ephemeralPubKey",
            ethereum.Value.fromBytes(FIXTURE_EPK)
          ),
          new ethereum.EventParam(
            "metadata",
            ethereum.Value.fromBytes(FIXTURE_METADATA)
          ),
        ],
        []
      )
    );
    // Set msg.value (the ETH amount the subgraph reads as call.transaction.value)
    call.transaction.value = FIXTURE_ETH_AMOUNT;
    // Set caller (the subgraph reads call.from)
    call.from = FIXTURE_SENDER;

    handleSendETH(call);
    let id = mockSendId(call);

    assert.fieldEquals("Send", id, "schemeId", "1");
    assert.fieldEquals(
      "Send",
      id,
      "stealthAddresses",
      "[" + FIXTURE_STEALTH_ADDRESS.toHexString() + "]"
    );
  });

  test("stores sender (call.from) as the sender field", () => {
    let call = changetype<SendETHCall>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "schemeId",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SCHEME_ID)
          ),
          new ethereum.EventParam(
            "stealthAddress",
            ethereum.Value.fromAddress(FIXTURE_STEALTH_ADDRESS)
          ),
          new ethereum.EventParam(
            "ephemeralPubKey",
            ethereum.Value.fromBytes(FIXTURE_EPK)
          ),
          new ethereum.EventParam(
            "metadata",
            ethereum.Value.fromBytes(FIXTURE_METADATA)
          ),
        ],
        []
      )
    );
    call.transaction.value = FIXTURE_ETH_AMOUNT;
    call.from = FIXTURE_SENDER;

    handleSendETH(call);
    let id = mockSendId(call);

    assert.fieldEquals("Send", id, "sender", FIXTURE_SENDER.toHexString());
  });

  test("stores token as zero address for ETH send", () => {
    let call = changetype<SendETHCall>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "schemeId",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SCHEME_ID)
          ),
          new ethereum.EventParam(
            "stealthAddress",
            ethereum.Value.fromAddress(FIXTURE_STEALTH_ADDRESS)
          ),
          new ethereum.EventParam(
            "ephemeralPubKey",
            ethereum.Value.fromBytes(FIXTURE_EPK)
          ),
          new ethereum.EventParam(
            "metadata",
            ethereum.Value.fromBytes(FIXTURE_METADATA)
          ),
        ],
        []
      )
    );
    call.transaction.value = FIXTURE_ETH_AMOUNT;
    call.from = FIXTURE_SENDER;

    handleSendETH(call);
    let id = mockSendId(call);

    // For ETH sends the mapping uses Address.zero() as the token field.
    assert.fieldEquals(
      "Send",
      id,
      "token",
      "0x0000000000000000000000000000000000000000"
    );
  });

  test("totalAmount equals call.transaction.value", () => {
    let call = changetype<SendETHCall>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "schemeId",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SCHEME_ID)
          ),
          new ethereum.EventParam(
            "stealthAddress",
            ethereum.Value.fromAddress(FIXTURE_STEALTH_ADDRESS)
          ),
          new ethereum.EventParam(
            "ephemeralPubKey",
            ethereum.Value.fromBytes(FIXTURE_EPK)
          ),
          new ethereum.EventParam(
            "metadata",
            ethereum.Value.fromBytes(FIXTURE_METADATA)
          ),
        ],
        []
      )
    );
    call.transaction.value = FIXTURE_ETH_AMOUNT;
    call.from = FIXTURE_SENDER;

    handleSendETH(call);
    let id = mockSendId(call);

    assert.fieldEquals(
      "Send",
      id,
      "totalAmount",
      FIXTURE_ETH_AMOUNT.toString()
    );
  });
});

// ── handleSendERC20 ───────────────────────────────────────────────────────────

describe("handleSendERC20", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("creates a Send entity with correct token, amount, schemeId, stealthAddress", () => {
    let call = changetype<SendERC20Call>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "token",
            ethereum.Value.fromAddress(FIXTURE_TOKEN_ADDRESS)
          ),
          new ethereum.EventParam(
            "amount",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_TOKEN_AMOUNT)
          ),
          new ethereum.EventParam(
            "schemeId",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SCHEME_ID)
          ),
          new ethereum.EventParam(
            "stealthAddress",
            ethereum.Value.fromAddress(FIXTURE_STEALTH_ADDRESS)
          ),
          new ethereum.EventParam(
            "ephemeralPubKey",
            ethereum.Value.fromBytes(FIXTURE_EPK)
          ),
          new ethereum.EventParam(
            "metadata",
            ethereum.Value.fromBytes(FIXTURE_METADATA)
          ),
        ],
        []
      )
    );
    call.from = FIXTURE_SENDER;

    handleSendERC20(call);
    let id = mockSendId(call);

    assert.fieldEquals(
      "Send",
      id,
      "token",
      FIXTURE_TOKEN_ADDRESS.toHexString()
    );
    assert.fieldEquals("Send", id, "schemeId", "1");
    assert.fieldEquals(
      "Send",
      id,
      "totalAmount",
      FIXTURE_TOKEN_AMOUNT.toString()
    );
    assert.fieldEquals(
      "Send",
      id,
      "stealthAddresses",
      "[" + FIXTURE_STEALTH_ADDRESS.toHexString() + "]"
    );
  });

  test("sender field equals call.from", () => {
    let call = changetype<SendERC20Call>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "token",
            ethereum.Value.fromAddress(FIXTURE_TOKEN_ADDRESS)
          ),
          new ethereum.EventParam(
            "amount",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_TOKEN_AMOUNT)
          ),
          new ethereum.EventParam(
            "schemeId",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SCHEME_ID)
          ),
          new ethereum.EventParam(
            "stealthAddress",
            ethereum.Value.fromAddress(FIXTURE_STEALTH_ADDRESS)
          ),
          new ethereum.EventParam(
            "ephemeralPubKey",
            ethereum.Value.fromBytes(FIXTURE_EPK)
          ),
          new ethereum.EventParam(
            "metadata",
            ethereum.Value.fromBytes(FIXTURE_METADATA)
          ),
        ],
        []
      )
    );
    call.from = FIXTURE_SENDER;

    handleSendERC20(call);
    let id = mockSendId(call);

    assert.fieldEquals("Send", id, "sender", FIXTURE_SENDER.toHexString());
  });
});
