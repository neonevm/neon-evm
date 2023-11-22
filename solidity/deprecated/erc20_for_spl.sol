// SPDX-License-Identifier: MIT

pragma solidity >= 0.7.0;
pragma abicoder v2;

import './SPLToken.sol';
import './Metaplex.sol';

SPLToken constant _splToken = SPLToken(0xFf00000000000000000000000000000000000004);
Metaplex constant _metaplex = Metaplex(0xff00000000000000000000000000000000000005);

contract ERC20ForSpl {

    bytes32 immutable public tokenMint;

    mapping(address => mapping(address => uint256)) private _allowances;

    event Transfer(address indexed from, address indexed to, uint256 amount);
    event Approval(address indexed owner, address indexed spender, uint256 amount);

    event ApprovalSolana(address indexed owner, bytes32 indexed spender, uint64 amount);
    event TransferSolana(address indexed from, bytes32 indexed to, uint64 amount);

    constructor(bytes32 _tokenMint) {
        require(_splToken.getMint(_tokenMint).isInitialized, "ERC20: invalid token mint");
        require(_metaplex.isInitialized(_tokenMint), "ERC20: missing MetaPlex metadata");

        tokenMint = _tokenMint;
    }

    function name() public view returns (string memory) {
        return _metaplex.name(tokenMint);
    }

    function symbol() public view returns (string memory) {
        return _metaplex.symbol(tokenMint);
    }

    function decimals() public view returns (uint8) {
        return _splToken.getMint(tokenMint).decimals;
    }

    function totalSupply() public view returns (uint256) {
        return _splToken.getMint(tokenMint).supply;
    }

    function balanceOf(address who) public view returns (uint256) {
        bytes32 account = _solanaAccount(who);
        return _splToken.getAccount(account).amount;
    }

    function allowance(address owner, address spender) public view returns (uint256) {
        return _allowances[owner][spender];
    }

    function approve(address spender, uint256 amount) public returns (bool) {
        address owner = msg.sender;

        _approve(owner, spender, amount);

        return true;
    }

    function transfer(address to, uint256 amount) public returns (bool) {
        address from = msg.sender;

        _transfer(from, to, amount);

        return true;
    }


    function transferFrom(address from, address to, uint256 amount) public returns (bool) {
        address spender = msg.sender;

        _spendAllowance(from, spender, amount);
        _transfer(from, to, amount);

        return true;
    }

    function burn(uint256 amount) public returns (bool) {
        address from = msg.sender;

        _burn(from, amount);

        return true;
    }


    function burnFrom(address from, uint256 amount) public returns (bool) {
        address spender = msg.sender;

        _spendAllowance(from, spender, amount);
        _burn(from, amount);

        return true;
    }

    
    function approveSolana(bytes32 spender, uint64 amount) public returns (bool) {
        address from = msg.sender;
        bytes32 fromSolana = _solanaAccount(from);

        if (amount > 0) {
            _splToken.approve(fromSolana, spender, amount);
        } else {
            _splToken.revoke(fromSolana);
        }

        emit Approval(from, address(0), amount);
        emit ApprovalSolana(from, spender, amount);

        return true;
    }

    function transferSolana(bytes32 to, uint64 amount) public returns (bool) {
        address from = msg.sender;
        bytes32 fromSolana = _solanaAccount(from);

        _splToken.transfer(fromSolana, to, uint64(amount));

        emit Transfer(from, address(0), amount);
        emit TransferSolana(from, to, amount);

        return true;
    }

    function claim(bytes32 from, uint64 amount) external returns (bool) {
        return claimTo(from, msg.sender, amount);
    }

    function claimTo(bytes32 from, address to, uint64 amount) public returns (bool) {
        bytes32 toSolana = _solanaAccount(to);

        if (_splToken.isSystemAccount(toSolana)) {
            _splToken.initializeAccount(_salt(to), tokenMint);
        }

        _splToken.transferWithSeed(_salt(msg.sender), from, toSolana, amount);

        emit Transfer(address(0), to, amount);

        return true;
    }

    function _approve(address owner, address spender, uint256 amount) internal {
        require(owner != address(0), "ERC20: approve from the zero address");
        require(spender != address(0), "ERC20: approve to the zero address");

        _allowances[owner][spender] = amount;
        emit Approval(owner, spender, amount);
    }

    function _spendAllowance(address owner, address spender, uint256 amount) internal {
        uint256 currentAllowance = allowance(owner, spender);
        if (currentAllowance != type(uint256).max) {
            require(currentAllowance >= amount, "ERC20: insufficient allowance");
            _approve(owner, spender, currentAllowance - amount);
        }
    }

    function _burn(address from, uint256 amount) internal {
        require(from != address(0), "ERC20: burn from the zero address");
        require(amount <= type(uint64).max, "ERC20: burn amount exceeds uint64 max");

        bytes32 fromSolana = _solanaAccount(from);

        require(_splToken.getAccount(fromSolana).amount >= amount, "ERC20: burn amount exceeds balance");
        _splToken.burn(tokenMint, fromSolana, uint64(amount));

        emit Transfer(from, address(0), amount);
    }

    function _transfer(address from, address to, uint256 amount) internal {
        require(from != address(0), "ERC20: transfer from the zero address");
        require(to != address(0), "ERC20: transfer to the zero address");

        bytes32 fromSolana = _solanaAccount(from);
        bytes32 toSolana = _solanaAccount(to);

        require(amount <= type(uint64).max, "ERC20: transfer amount exceeds uint64 max");
        require(_splToken.getAccount(fromSolana).amount >= amount, "ERC20: transfer amount exceeds balance");

        if (_splToken.isSystemAccount(toSolana)) {
            _splToken.initializeAccount(_salt(to), tokenMint);
        }

        _splToken.transfer(fromSolana, toSolana, uint64(amount));

        emit Transfer(from, to, amount);
    }

    function _salt(address account) internal pure returns (bytes32) {
        return bytes32(uint256(uint160(account)));
    }

    function _solanaAccount(address account) internal pure returns (bytes32) {
        return _splToken.findAccount(_salt(account));
    }
}

contract ERC20ForSplMintable is ERC20ForSpl {
    address immutable _admin;

    constructor(
        string memory _name,
        string memory _symbol,
        uint8 _decimals,
        address _mint_authority
    ) ERC20ForSpl(_initialize(_name, _symbol, _decimals)) {
        _admin = _mint_authority;
    }

    function findMintAccount() public pure returns (bytes32) {
        return _splToken.findAccount(bytes32(0));
    }

    function mint(address to, uint256 amount) public {
        require(msg.sender == _admin, "ERC20: must have minter role to mint");
        require(to != address(0), "ERC20: mint to the zero address");
        require(amount <= type(uint64).max, "ERC20: mint amount exceeds uint64 max");
        require(totalSupply() + amount <= type(uint64).max, "ERC20: total mint amount exceeds uint64 max");

        bytes32 toSolana = _solanaAccount(to);
        if (_splToken.isSystemAccount(toSolana)) {
            _splToken.initializeAccount(_salt(to), tokenMint);
        }

        _splToken.mintTo(tokenMint, toSolana, uint64(amount));

        emit Transfer(address(0), to, amount);
    }

    function _initialize(
        string memory _name,
        string memory _symbol,
        uint8 _decimals
    ) private returns (bytes32) {
        bytes32 mintAddress = _splToken.initializeMint(bytes32(0), _decimals);
        _metaplex.createMetadata(mintAddress, _name, _symbol, "");
        return mintAddress;
    }
}