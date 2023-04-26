// SPDX-License-Identifier: MIT

pragma solidity >= 0.7.0;
pragma abicoder v2;

interface SPLToken {

    enum AccountState {
        Uninitialized,
        Initialized,
        Frozen
    }

    struct Account {
        bytes32 mint;
        bytes32 owner;
        uint64 amount;
        bytes32 delegate;
        uint64 delegated_amount;
        bytes32 close_authority;
        AccountState state;
    }

    struct Mint {
        uint64 supply;
        uint8 decimals;
        bool isInitialized;
        bytes32 freezeAuthority;
        bytes32 mintAuthority;
    }

    function findAccount(bytes32 salt) external pure returns(bytes32);

    function isSystemAccount(bytes32 account) external view returns(bool);

    // Return spl_token account data. This function checks the account is owned by correct spl_token.
    // Return default not initialized spl_token account data if corresponded Solana account doesn't exist.
    function getAccount(bytes32 account) external view returns(Account memory);

    // Return spl_token mint data. This function checks the mint is owned by correct spl_token.
    // Return default not initialized spl_token mint data if corresponded Solana account doesn't exist.
    function getMint(bytes32 account) external view returns(Mint memory);

    function initializeMint(bytes32 salt, uint8 decimals) external returns(bytes32);
    function initializeMint(bytes32 salt, uint8 decimals, bytes32 mint_authority, bytes32 freeze_authority) external returns(bytes32);

    function initializeAccount(bytes32 salt, bytes32 mint) external returns(bytes32);
    function initializeAccount(bytes32 salt, bytes32 mint, bytes32 owner) external returns(bytes32);

    function closeAccount(bytes32 account) external;

    function mintTo(bytes32 mint, bytes32 account, uint64 amount) external;
    function burn(bytes32 mint, bytes32 account, uint64 amount) external;

    function approve(bytes32 source, bytes32 target, uint64 amount) external;
    function revoke(bytes32 source) external;

    function transfer(bytes32 source, bytes32 target, uint64 amount) external;

    // transfer funds from spl-token accounts owned by Solana user.
    // This method uses PDA[ACCOUNT_SEED_VERSION, b"AUTH", msg.sender, seed] to authorize transfer
    function transferWithSeed(bytes32 seed, bytes32 source, bytes32 target, uint64 amount) external;

    function freeze(bytes32 mint, bytes32 account) external;
    function thaw(bytes32 mint, bytes32 account) external;
}
