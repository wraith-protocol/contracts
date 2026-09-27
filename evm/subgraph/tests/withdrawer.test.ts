/**
 * Matchstick unit tests for mappings/withdrawer.ts:
 *   handleWithdrawETH, handleWithdrawERC20, handleWithdrawETHDirect,
 *   handleWithdrawERC20Direct.
 *
 * WraithWithdrawer emits NO on-chain events; the subgraph uses call handlers.
 * These tests construct mock Call objects and assert on the resulting Withdrawal
 * entity — directly exercising the mapping's field accesses so a rename in the
 * mapping code (e.g. call.inputs.destination → call.inputs.dest) surfaces as
 * a compile or runtime failure here.
 *
 * Fixture values (must match evm/test/conformance.test.ts):
 *   FIXTURE_SPONSOR_FEE  = 0.001 ETH  = 1000000000000000 wei
 *   FIXTURE_DESTINATION  = 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
 *   FIXTURE_SPONSOR      = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
 *   FIXTURE_TOKEN_ADDRESS = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48
 */
import {
  Address,
  BigInt,
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
  WithdrawETHCall,
  WithdrawERC20Call,
  WithdrawETHDirectCall,
  WithdrawERC20DirectCall,
} from "../generated/WraithWithdrawer/WraithWithdrawer";
import {
  handleWithdrawETH,
  handleWithdrawERC20,
  handleWithdrawETHDirect,
  handleWithdrawERC20Direct,
} from "../mappings/withdrawer";

// ── Fixture constants (must match evm/test/conformance.test.ts) ──────────────

// 0.001 ETH in wei — matches FIXTURE_SPONSOR_FEE = ethers.parseEther('0.001')
const FIXTURE_SPONSOR_FEE = BigInt.fromString("1000000000000000");
const FIXTURE_DESTINATION = Address.fromString(
  "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
);
const FIXTURE_SPONSOR = Address.fromString(
  "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
);
const FIXTURE_TOKEN_ADDRESS = Address.fromString(
  "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
);

// ── Helper to produce entity ID (matches withdrawer.ts) ──────────────────────
function withdrawalId(call: ethereum.Call): string {
  return (
    call.transaction.hash.toHexString() +
    "-" +
    call.transaction.index.toString()
  );
}

// ── handleWithdrawETH ─────────────────────────────────────────────────────────

describe("handleWithdrawETH", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("stores destination and sponsorFee from call inputs", () => {
    let call = changetype<WithdrawETHCall>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "destination",
            ethereum.Value.fromAddress(FIXTURE_DESTINATION)
          ),
          new ethereum.EventParam(
            "sponsorFee",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SPONSOR_FEE)
          ),
        ],
        []
      )
    );
    call.from = FIXTURE_SPONSOR;

    handleWithdrawETH(call);
    let id = withdrawalId(call);

    assert.fieldEquals(
      "Withdrawal",
      id,
      "destination",
      FIXTURE_DESTINATION.toHexString()
    );
    assert.fieldEquals(
      "Withdrawal",
      id,
      "sponsorFee",
      FIXTURE_SPONSOR_FEE.toString()
    );
  });

  test("stores sponsor (call.from)", () => {
    let call = changetype<WithdrawETHCall>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "destination",
            ethereum.Value.fromAddress(FIXTURE_DESTINATION)
          ),
          new ethereum.EventParam(
            "sponsorFee",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SPONSOR_FEE)
          ),
        ],
        []
      )
    );
    call.from = FIXTURE_SPONSOR;

    handleWithdrawETH(call);
    let id = withdrawalId(call);

    assert.fieldEquals("Withdrawal", id, "sponsor", FIXTURE_SPONSOR.toHexString());
  });

  test("token is zero address for ETH withdrawal", () => {
    let call = changetype<WithdrawETHCall>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "destination",
            ethereum.Value.fromAddress(FIXTURE_DESTINATION)
          ),
          new ethereum.EventParam(
            "sponsorFee",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SPONSOR_FEE)
          ),
        ],
        []
      )
    );
    call.from = FIXTURE_SPONSOR;

    handleWithdrawETH(call);
    let id = withdrawalId(call);

    assert.fieldEquals(
      "Withdrawal",
      id,
      "token",
      "0x0000000000000000000000000000000000000000"
    );
  });
});

// ── handleWithdrawERC20 ───────────────────────────────────────────────────────

describe("handleWithdrawERC20", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("stores token, destination, and sponsorFee from call inputs", () => {
    let call = changetype<WithdrawERC20Call>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "token",
            ethereum.Value.fromAddress(FIXTURE_TOKEN_ADDRESS)
          ),
          new ethereum.EventParam(
            "destination",
            ethereum.Value.fromAddress(FIXTURE_DESTINATION)
          ),
          new ethereum.EventParam(
            "sponsorFee",
            ethereum.Value.fromUnsignedBigInt(FIXTURE_SPONSOR_FEE)
          ),
        ],
        []
      )
    );
    call.from = FIXTURE_SPONSOR;

    handleWithdrawERC20(call);
    let id = withdrawalId(call);

    assert.fieldEquals(
      "Withdrawal",
      id,
      "token",
      FIXTURE_TOKEN_ADDRESS.toHexString()
    );
    assert.fieldEquals(
      "Withdrawal",
      id,
      "destination",
      FIXTURE_DESTINATION.toHexString()
    );
    assert.fieldEquals(
      "Withdrawal",
      id,
      "sponsorFee",
      FIXTURE_SPONSOR_FEE.toString()
    );
  });
});

// ── handleWithdrawETHDirect ───────────────────────────────────────────────────

describe("handleWithdrawETHDirect", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("stores destination; sponsorFee is zero for direct withdrawal", () => {
    let call = changetype<WithdrawETHDirectCall>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "destination",
            ethereum.Value.fromAddress(FIXTURE_DESTINATION)
          ),
        ],
        []
      )
    );
    call.from = FIXTURE_SPONSOR;

    handleWithdrawETHDirect(call);
    let id = withdrawalId(call);

    assert.fieldEquals(
      "Withdrawal",
      id,
      "destination",
      FIXTURE_DESTINATION.toHexString()
    );
    // Direct withdrawal has no sponsor fee — mapping stores BigInt.zero()
    assert.fieldEquals("Withdrawal", id, "sponsorFee", "0");
  });
});

// ── handleWithdrawERC20Direct ─────────────────────────────────────────────────

describe("handleWithdrawERC20Direct", () => {
  beforeEach(() => {
    clearStore();
  });

  afterEach(() => {
    clearStore();
  });

  test("stores token and destination; sponsorFee is zero", () => {
    let call = changetype<WithdrawERC20DirectCall>(
      newMockCallWithIO(
        [
          new ethereum.EventParam(
            "token",
            ethereum.Value.fromAddress(FIXTURE_TOKEN_ADDRESS)
          ),
          new ethereum.EventParam(
            "destination",
            ethereum.Value.fromAddress(FIXTURE_DESTINATION)
          ),
        ],
        []
      )
    );
    call.from = FIXTURE_SPONSOR;

    handleWithdrawERC20Direct(call);
    let id = withdrawalId(call);

    assert.fieldEquals(
      "Withdrawal",
      id,
      "token",
      FIXTURE_TOKEN_ADDRESS.toHexString()
    );
    assert.fieldEquals(
      "Withdrawal",
      id,
      "destination",
      FIXTURE_DESTINATION.toHexString()
    );
    assert.fieldEquals("Withdrawal", id, "sponsorFee", "0");
  });
});
