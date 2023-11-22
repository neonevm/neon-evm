// SPDX-License-Identifier: MIT
pragma solidity 0.8.21;

import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";
import './interfaces/ISPLToken.sol';
import './interfaces/IMetaplex.sol';

/// @custom:oz-upgrades-unsafe-allow constructor
contract ERC20ForSPL is OwnableUpgradeable, UUPSUpgradeable {
    ISPLToken public constant SPL_TOKEN = ISPLToken(0xFf00000000000000000000000000000000000004);
    IMetaplex public constant METAPLEX = IMetaplex(0xff00000000000000000000000000000000000005);
    bytes32 public tokenMint;
    mapping(address => mapping(address => uint256)) private _allowances;

    event Transfer(address indexed from, address indexed to, uint256 amount);
    event Approval(address indexed owner, address indexed spender, uint256 amount);
    event ApprovalSolana(address indexed owner, bytes32 indexed spender, uint64 amount);
    event TransferSolana(address indexed from, bytes32 indexed to, uint64 amount);

    error EmptyToAddress();
    error EmptyFromAddress();
    error InvalidAllowance();
    error AmountExceedsBalance();
    error MissingMetaplex();
    error InvalidTokenMint();
    error AmountExceedsUint64();

    constructor() {
        _disableInitializers();
    }

    function initializeParent(bytes32 _tokenMint) public onlyInitializing {
        __Ownable_init(msg.sender);
        if (!SPL_TOKEN.getMint(_tokenMint).isInitialized) revert InvalidTokenMint();
        if (!METAPLEX.isInitialized(_tokenMint)) revert MissingMetaplex();

        tokenMint = _tokenMint;
    }

    function _authorizeUpgrade(address) internal override onlyOwner {}

    function name() public view returns (string memory) {
        return METAPLEX.name(tokenMint);
    }

    function symbol() public view returns (string memory) {
        return METAPLEX.symbol(tokenMint);
    }

    function decimals() public view returns (uint8) {
        return SPL_TOKEN.getMint(tokenMint).decimals;
    }

    function totalSupply() public view returns (uint256) {
        return SPL_TOKEN.getMint(tokenMint).supply;
    }

    function balanceOf(address who) public view returns (uint256) {
        return SPL_TOKEN.getAccount(solanaAccount(who)).amount;
    }

    function allowance(address owner, address spender) public view returns (uint256) {
        return _allowances[owner][spender];
    }

    function approve(address spender, uint256 amount) public returns (bool) {
        _approve(msg.sender, spender, amount);
        return true;
    }

    function transfer(address to, uint256 amount) public returns (bool) {
        _transfer(msg.sender, to, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) public returns (bool) {
        _spendAllowance(from, msg.sender, amount);
        _transfer(from, to, amount);
        return true;
    }

    function burn(uint256 amount) public returns (bool) {
        _burn(msg.sender, amount);
        return true;
    }

    function burnFrom(address from, uint256 amount) public returns (bool) {
        _spendAllowance(from, msg.sender, amount);
        _burn(from, amount);
        return true;
    }

    function approveSolana(bytes32 spender, uint64 amount) public returns (bool) {
        address from = msg.sender;
        bytes32 fromSolana = solanaAccount(from);

        if (amount > 0) {
            SPL_TOKEN.approve(fromSolana, spender, amount);
        } else {
            SPL_TOKEN.revoke(fromSolana);
        }

        emit Approval(from, address(0), amount);
        emit ApprovalSolana(from, spender, amount);
        return true;
    }

    function transferSolana(bytes32 to, uint64 amount) public returns (bool) {
        address from = msg.sender;
        bytes32 fromSolana = solanaAccount(from);

        SPL_TOKEN.transfer(fromSolana, to, uint64(amount));

        emit Transfer(from, address(0), amount);
        emit TransferSolana(from, to, amount);
        return true;
    }

    function claim(bytes32 from, uint64 amount) external returns (bool) {
        return claimTo(from, msg.sender, amount);
    }

    function claimTo(bytes32 from, address to, uint64 amount) public returns (bool) {
        bytes32 toSolana = solanaAccount(to);

        if (SPL_TOKEN.isSystemAccount(toSolana)) {
            SPL_TOKEN.initializeAccount(_salt(to), tokenMint);
        }

        SPL_TOKEN.transferWithSeed(_salt(msg.sender), from, toSolana, amount);
        emit Transfer(address(0), to, amount);
        return true;
    }

    function _approve(address owner, address spender, uint256 amount) internal {
        if (owner == address(0)) revert EmptyFromAddress();
        if (spender == address(0)) revert EmptyToAddress();

        _allowances[owner][spender] = amount;
        emit Approval(owner, spender, amount);
    }

    function _spendAllowance(address owner, address spender, uint256 amount) internal {
        uint256 currentAllowance = allowance(owner, spender);
        if (currentAllowance != type(uint256).max) {
            if (currentAllowance < amount) revert InvalidAllowance();
            _approve(owner, spender, currentAllowance - amount);
        }
    }

    function _burn(address from, uint256 amount) internal {
        if (from == address(0)) revert EmptyFromAddress();
        if (amount > type(uint64).max) revert AmountExceedsUint64();

        bytes32 fromSolana = solanaAccount(from);
        if (SPL_TOKEN.getAccount(fromSolana).amount < amount) revert AmountExceedsBalance();
        SPL_TOKEN.burn(tokenMint, fromSolana, uint64(amount));

        emit Transfer(from, address(0), amount);
    }

    function _transfer(address from, address to, uint256 amount) internal {
        if (from == address(0)) revert EmptyFromAddress();
        if (to == address(0)) revert EmptyToAddress();

        bytes32 fromSolana = solanaAccount(from);
        bytes32 toSolana = solanaAccount(to);

        if (amount > type(uint64).max) revert AmountExceedsUint64();
        if (SPL_TOKEN.getAccount(fromSolana).amount < amount) revert AmountExceedsBalance();

        if (SPL_TOKEN.isSystemAccount(toSolana)) {
            SPL_TOKEN.initializeAccount(_salt(to), tokenMint);
        }

        SPL_TOKEN.transfer(fromSolana, toSolana, uint64(amount));
        emit Transfer(from, to, amount);
    }

    function _salt(address account) internal pure returns (bytes32) {
        return bytes32(uint256(uint160(account)));
    }

    function solanaAccount(address account) public pure returns (bytes32) {
        return SPL_TOKEN.findAccount(_salt(account));
    }

    function getAccountDelegateData(address who) public view returns(bytes32, uint64) {
        ISPLToken.Account memory account = SPL_TOKEN.getAccount(solanaAccount(who));
        return (account.delegate, account.delegated_amount);
    }
}


contract ERC20ForSPLMintable is ERC20ForSPL {
    function initialize(
        string memory _name,
        string memory _symbol,
        uint8 _decimals
    ) public initializer {       
        ERC20ForSPL.initializeParent(_initialize(_name, _symbol, _decimals));
    }

    function findMintAccount() public pure returns (bytes32) {
        return SPL_TOKEN.findAccount(bytes32(0));
    }

    function mint(address to, uint256 amount) public onlyOwner {
        if (to == address(0)) revert EmptyToAddress();
        if (totalSupply() + amount > type(uint64).max) revert AmountExceedsUint64();

        bytes32 toSolana = solanaAccount(to);
        if (SPL_TOKEN.isSystemAccount(toSolana)) {
            SPL_TOKEN.initializeAccount(_salt(to), tokenMint);
        }

        SPL_TOKEN.mintTo(tokenMint, toSolana, uint64(amount));
        emit Transfer(address(0), to, amount);
    }

    function _initialize(
        string memory _name,
        string memory _symbol,
        uint8 _decimals
    ) private returns (bytes32) {
        bytes32 mintAddress = SPL_TOKEN.initializeMint(bytes32(0), _decimals);
        METAPLEX.createMetadata(mintAddress, _name, _symbol, "");
        return mintAddress;
    }
}