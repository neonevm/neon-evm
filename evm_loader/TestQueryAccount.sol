// SPDX-License-Identifier: UNLICENSED
pragma solidity >=0.7.0;

contract TestQueryAccount {
    QueryAccount query;

    constructor() {
        query = new QueryAccount();
    }

    function test_metadata_ok() public returns (bool) {
        uint256 solana_address = 110178555362476360822489549210862241441608066866019832842197691544474470948129;

        uint256 golden_ownr = 3106054211088883198575105191760876350940303353676611666299516346430146937001;
        uint256 golden_len = 82;
        uint256 golden_lamp = 1461600;
        bool golden_exec = false;
        uint256 golden_repoch = 0;

        uint256 ownr = query.owner(solana_address);
        if (ownr != golden_ownr) {
            return false;
        }

        uint len = query.length(solana_address);
        if (len != golden_len) {
            return false;
        }

        uint256 lamp = query.lamports(solana_address);
        if (lamp != golden_lamp) {
            return false;
        }

        bool exec = query.executable(solana_address);
        if (exec != golden_exec) {
            return false;
        }

        uint256 repoch = query.rent_epoch(solana_address);
        if (repoch != golden_repoch) {
            return false;
        }

        return true;
    }

    function test_metadata_nonexistent_account() public returns (bool) {
        uint256 solana_address = 90000; // should not exist
        bool ok = false;

        try query.owner(solana_address) { ok = false; } catch { ok = true; /* expected exception */ }
        if (!ok) { return ok; }

        try query.length(solana_address) { ok = false; } catch { ok = true; /* expected exception */ }
        if (!ok) { return ok; }

        try query.lamports(solana_address) { ok = false; } catch { ok = true; /* expected exception */ }
        if (!ok) { return ok; }

        try query.executable(solana_address) { ok = false; } catch { ok = true; /* expected exception */ }
        if (!ok) { return ok; }

        try query.rent_epoch(solana_address) { ok = false; } catch { ok = true; /* expected exception */ }

        return ok;
    }

    function test_data_ok() public returns (bool) {
        uint256 solana_address = 110178555362476360822489549210862241441608066866019832842197691544474470948129;
        byte b0 = 0x71;
        byte b1 = 0x33;
        byte b2 = 0xc6;
        byte b3 = 0x12;

        // Test getting subset of data
        uint64 offset = 20;
        uint64 len = 4;
        bytes memory result = query.data(solana_address, offset, len);
        if (result.length != 4) {
            return false;
        }
        if (result[0] != b0) {
            return false;
        }
        if (result[1] != b1) {
            return false;
        }
        if (result[2] != b2) {
            return false;
        }
        if (result[3] != b3) {
            return false;
        }
        // Test getting full data
        offset = 0;
        len = 82;
        result = query.data(solana_address, offset, len);
        if (result.length != 82) {
            return false;
        }

        return true;
    }

    function test_data_nonexistent_account() public returns (bool) {
        uint256 solana_address = 90000; // hopefully does not exist
        uint64 offset = 0;
        uint64 len = 1;
        try query.data(solana_address, offset, len) { } catch {
            return true; // expected exception
        }
        return false;
    }

    function test_data_too_big_offset() public returns (bool) {
        uint256 solana_address = 110178555362476360822489549210862241441608066866019832842197691544474470948129;
        uint64 offset = 200; // data len is 82
        uint64 len = 1;
        try query.data(solana_address, offset, len) { } catch {
            return true; // expected exception
        }
        return false;
    }

    function test_data_too_big_length() public returns (bool) {
        uint256 solana_address = 110178555362476360822489549210862241441608066866019832842197691544474470948129;
        uint64 offset = 0;
        uint64 len = 200; // data len is 82
        try query.data(solana_address, offset, len) { } catch {
            return true; // expected exception
        }
        return false;
    }
}
