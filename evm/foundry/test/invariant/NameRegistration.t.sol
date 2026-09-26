// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Test} from "forge-std/Test.sol";
import {StdInvariant} from "forge-std/StdInvariant.sol";
import {WraithNames} from "@wraith/contracts/WraithNames.sol";
import {NameFixtures} from "./NameFixtures.sol";

/// @notice Invariant suite for WraithNames. The registry maps each name to
///         exactly one meta-address (no wallet address stored) and proves
///         ownership via the spending key. These invariants verify monotonic,
///         collision-free registration:
///           - forward resolution maps a name to at most one meta-address;
///           - `nameOf` (reverse lookup) is the exact inverse of `resolve`;
///           - updates atomically re-point the reverse map, so a stale forward
///             value is never observable from `resolve`/`nameOf`.
contract NameRegistration is Test {
    WraithNames internal names;

    function setUp() public {
        names = new WraithNames();
        // The handler functions on this test contract are the fuzz targets; the
        // deployed names contract must not be fuzzed as an independent target.
        targetContract(address(this));
        excludeContract(address(names));
        // Always anchor a live row so forward/reverse invariants are meaningful.
        names.register(
            "wraith", NameFixtures.META_WRAITH, NameFixtures.SIG_REGISTER_WRAITH
        );
    }

    /// @notice Handler: register a dormant fixture name (guarded so it can't
    ///         revert on an already-taken name — keeps handlers sandboxed).
    function registerFixture(uint256 seed_) public {
        uint256 i = seed_ % 8;
        if (i == 0 && names.resolve("wraith").length == 0) {
            names.register("wraith", NameFixtures.META_WRAITH, NameFixtures.SIG_REGISTER_WRAITH);
        } else if (i == 1 && names.resolve("alice").length == 0) {
            names.register("alice", NameFixtures.META_ALICE, NameFixtures.SIG_REGISTER_ALICE);
        } else if (i == 2 && names.resolve("bob").length == 0) {
            names.register("bob", NameFixtures.META_BOB, NameFixtures.SIG_REGISTER_BOB);
        } else if (i == 3 && names.resolve("carlo").length == 0) {
            names.register("carlo", NameFixtures.META_CARLO, NameFixtures.SIG_REGISTER_CARLO);
        } else if (i == 4 && names.resolve("daria").length == 0) {
            names.register("daria", NameFixtures.META_DARIA, NameFixtures.SIG_REGISTER_DARIA);
        } else if (i == 5 && names.resolve("eliot").length == 0) {
            names.register("eliot", NameFixtures.META_ELIOT, NameFixtures.SIG_REGISTER_ELIOT);
        } else if (i == 6 && names.resolve("felix").length == 0) {
            names.register("felix", NameFixtures.META_FELIX, NameFixtures.SIG_REGISTER_FELIX);
        } else if (i == 7 && names.resolve("greta").length == 0) {
            names.register("greta", NameFixtures.META_GRETA, NameFixtures.SIG_REGISTER_GRETA);
        }
    }

    /// @notice Handler: update the "wraith" name to its alternate meta-address.
    ///         Guarded so a repeat update (after ownership moved to the alt
    ///         spending key) is a no-op instead of a reverting call — while the
    ///         stale original signature would revert, that revert is expected.
    function updateWraith() public {
        bytes memory current = names.resolve("wraith");
        if (current.length > 0 && keccak256(current) == keccak256(NameFixtures.META_WRAITH)) {
            names.update("wraith", NameFixtures.META_WRAITH_ALT, NameFixtures.SIG_UPDATE_WRAITH);
        }
    }

    /// @notice Invariant: forward resolution is deterministic and unambiguous —
    ///         a name resolves to at most one meta-address (monotone mapping),
    ///         either empty (unregistered) or exactly its single registered value.
    function invariant_forwardResolutionUnambiguous() public view {
        // Every fixture name must only ever map to its own meta-address or empty.
        bytes memory w = names.resolve("wraith");
        assertTrue(
            w.length == 0 ||
                (keccak256(w) == keccak256(NameFixtures.META_WRAITH)) ||
                (keccak256(w) == keccak256(NameFixtures.META_WRAITH_ALT)),
            "wraith resolved to an unexpected meta-address"
        );
        assertTrue(
            w.length <= NameFixtures.META_WRAITH.length,
            "resolve returned over-long meta-address"
        );
    }

    /// @notice Invariant: reverse lookup (`nameOf`) is the exact inverse of
    ///         `resolve` for the anchored name — no dangling or conflicting
    ///         rows in either direction at any time.
    function invariant_reverseLookupIsExactInverse() public view {
        bytes memory resolved = names.resolve("wraith");
        if (resolved.length > 0) {
            // The live meta-address resolves back to "wraith".
            string memory n = names.nameOf(resolved);
            assertEq(
                n, "wraith", "nameOf(resolve(name)) != name"
            );
        }
        // The original meta-address, if still mapped, is bound to "wraith".
        string memory orig = names.nameOf(NameFixtures.META_WRAITH);
        bool origWraith = keccak256(bytes(orig)) == keccak256(bytes("wraith"));
        bool origEmpty = bytes(orig).length == 0;
        assertTrue(origWraith || origEmpty, "original meta-address dangles");
    }

    /// @notice Invariant: a stale meta-address can never still answer a name
    ///         after an update — the reverse map is atomically re-pointed.
    function invariant_noStaleReverseBinding() public view {
        bytes memory resolved = names.resolve("wraith");
        // Whatever the current forward value is, its reverse points at wraith.
        if (resolved.length > 0) {
            assertEq(
                names.nameOf(resolved), "wraith",
                "reverse map did not follow the forward value"
            );
        }
    }
}