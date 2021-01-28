// SPDX-License-Identifier: MIT

pragma solidity ^0.5.12;
pragma experimental ABIEncoderV2;

// pragma solidity >=0.6.0 <0.8.0;

// import "./Context.sol";
// import "./IERC20.sol";
import "./SafeMath.sol";


contract ERC20  {
    using SafeMath for uint256;
    
      struct AccountMeta {
        bool need_translate;
        bytes name;
        bool is_signer;
        bool is_writable;
    }

    // event Transfer(address indexed from, address indexed to, uint256 value);
    // event Approval(address indexed owner, address indexed spender, uint256 value);

    mapping (address => uint256) private _balances;
    mapping (address => mapping (address => uint256)) private _allowances;

    uint256 private _totalSupply;

    // string private _name;
    // string private _symbol;
    // uint8 private _decimals;
    
    // uint256 constant system_id = 0x0000000000000000000000000000000000000000000000000000000000000000;  // hex representation of "11111111111111111111111111111111"
    uint256 constant token_id = 0x06ddf6e1d765a193d9cbe146ceeb79ac1cb485ed5f5b37913a8cf5857eff00a9; // hex representation of "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    address constant solana = 0xfF00000000000000000000000000000000000000;
    uint256 mint_id = 0x24aa692447f4dc757b4c808eecaf43ce07c6bb1eadca96c0452944a4fdaeb6a5;
    uint256 erc20_acc     = 0x4698cc716e548a7f73aba9b0244545c266797502bbfa48c239ccbaf5fd18bc93; //5kag8i1weEyDKA9MeL1iM717zLk3zDBCzrsZyfN5HnXL
    address erc20_acc_eth = 0x4698cc716E548a7f73aBa9B0244545c266797502;
    
    // string str;
    
    // function setToken(uint256 _mint, string memory _str) public {
    //     // TODO: Move to initialize
    //     mint_id = _mint;
    //     str = _str;
    // }
        
    function reverse(uint amount) private pure returns(uint64) {
        uint64 val = 0;
        for(uint i = 0; i < 8; i++) {
            val = (val << 8) | (uint64(amount) & 0xff);
            amount >>= 8;
        }
        if (amount != 0) {
            revert();
        }
        return val;
    }
    

    // constructor (string memory name_, string memory symbol_) public {
    //     _name = name_;
    //     _symbol = symbol_;
    //     _decimals = 18;
    // }

    // function name() public view returns (string memory) {
    //     return _name;
    // }

    // function symbol() public view returns (string memory) {
    //     return _symbol;
    // }

    // function decimals() public view returns (uint8) {
    //     return _decimals;
    // }

    // function totalSupply() public view  returns (uint256) {
    //     return _totalSupply;
    // }

    function balanceOf(address account) public view  returns (uint256) {
        return _balances[account];
    }

    function transfer(address recipient, uint256 amount) public returns (bool) {
        _transfer(msg.sender, recipient, amount);
        return true;
    }

    // function allowance(address owner, address spender) public view  returns (uint256) {
    //     return _allowances[owner][spender];
    // }

    // function approve(address spender, uint256 amount) public  returns (bool) {
    //     _approve(msg.sender, spender, amount);
    //     return true;
    // }

    // function transferFrom(address sender, address recipient, uint256 amount) public  returns (bool) {
    //     _transfer(sender, recipient, amount);
    //     _approve(sender, msg.sender, _allowances[sender][msg.sender].sub(amount, "ERC20: transfer amount exceeds allowance"));
    //     return true;
    // }

    // function increaseAllowance(address spender, uint256 addedValue) public  returns (bool) {
    //     _approve(msg.sender, spender, _allowances[msg.sender][spender].add(addedValue));
    //     return true;
    // }

    // function decreaseAllowance(address spender, uint256 subtractedValue) public  returns (bool) {
    //     _approve(msg.sender, spender, _allowances[msg.sender][spender].sub(subtractedValue, "ERC20: decreased allowance below zero"));
    //     return true;
    // }

    function _transfer(address sender, address recipient, uint256 amount) internal  {
        require(sender != address(0), "ERC20: transfer from the zero address");
        require(recipient != address(0), "ERC20: transfer to the zero address");

        // _beforeTokenTransfer(sender, recipient, amount);

        _balances[sender] = _balances[sender].sub(amount, "ERC20: transfer amount exceeds balance");
        _balances[recipient] = _balances[recipient].add(amount);
        // emit Transfer(sender, recipient, amount);
    }

    function _mint(address account, uint256 amount) internal  {
        require(account != address(0), "ERC20: mint to the zero address");

        // _beforeTokenTransfer(address(0), account, amount);

        _totalSupply = _totalSupply.add(amount);
        _balances[account] = _balances[account].add(amount);
        // emit Transfer(address(0), account, amount);
    }

    // function _burn(address account, uint256 amount) internal  {
    //     require(account != address(0), "ERC20: burn from the zero address");

    //     // _beforeTokenTransfer(account, address(0), amount);

    //     _balances[account] = _balances[account].sub(amount, "ERC20: burn amount exceeds balance");
    //     _totalSupply = _totalSupply.sub(amount);
    //     // emit Transfer(account, address(0), amount);
    // }

    // function _approve(address owner, address spender, uint256 amount) internal  {
    //     require(owner != address(0), "ERC20: approve from the zero address");
    //     require(spender != address(0), "ERC20: approve to the zero address");

    //     _allowances[owner][spender] = amount;
    //     // emit Approval(owner, spender, amount);
    // }

    // function _setupDecimals(uint8 decimals_) internal  {
    //     _decimals = decimals_;
    // }

    // function _beforeTokenTransfer(address from, address to, uint256 amount) internal  { }

        
    function packMeta(bool is_signer, bool is_writable, uint256 account) pure private returns(bytes memory) {
        return abi.encodePacked(false, is_signer, is_writable, account);
    }
    function packMeta(bool is_signer, bool is_writable, address account) pure private returns(bytes memory) {
        return abi.encodePacked(true, is_signer, is_writable, uint256(account));
    }
    function packMeta(bool is_signer, bool is_writable, bytes memory account) pure private returns(bytes memory) {
        if(account.length != 32) {revert();}
        return abi.encodePacked(false, is_signer, is_writable, account);
    }
    // function packMeta(AccountMeta memory meta) pure private returns(bytes memory) {
    //     return abi.encodePacked(meta.need_translate, meta.is_signer, meta.is_writable, meta.account);
    // }

    // function makeBalanceAccount(address acc) public returns(bytes memory) {
    //     bool status;
    //     bytes memory result;
        
    //     bytes memory call_data = abi.encodePacked(uint8(1), true, false, uint256(acc), token_id, str);
    //     (status, result) = solana.call(call_data);
    //     if (!status) {
    //         revert();
    //     }
    //     return result;
    // }
    
    function transferExt(uint256 from, uint256 signer, uint amount) public {
        uint64 val = reverse(amount);
        bytes memory instruction_data = abi.encodePacked(
                    uint8(0),    // external call
                    token_id,    // token contract
                    uint16(4),   // accountMeta count
                        packMeta(false, true,  from),
                        packMeta(false, false, mint_id),
                        packMeta(false, true,  erc20_acc),
                        packMeta(true,  false, signer),
                    abi.encodePacked(
                        uint8(12),        // transferChecked
                        uint64(val),      // amount
                        uint64(9)         // decimals
                    )
                );
                    
        bool status;
        bytes memory result;
        (status, result) = solana.call(instruction_data);
        if (!status) {revert();}
    }

    // function _callSolana(AccountMeta[] memory accs, bytes memory instruction_data) private returns(bytes memory) { 
    //     bool status;
        
    //     string memory program_id = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

    //     bytes memory call_data = abi.encodeWithSignature(
    //         "(string, (bytes, bool, bool, bool)[], hex)",
    //         program_id,
    //         accs, 
    //         instruction_data);

    //     bytes memory result;
    //     (status, result) = solana.call(call_data);
    //     if (!status) {
    //         revert();
    //     }
    //     return result;
    // }

    // function transferFromSol(bytes memory from, bytes memory to, uint amount) public {
    //     uint8 instr_id = 0x0;

    //     AccountMeta[] memory accs = new AccountMeta[](2);
    //     accs[0] = AccountMeta(false, from, true, false);
    //     accs[1] = AccountMeta(false, to, false, true);

    //     _callSolana(accs, abi.encodePacked(instr_id, amount));
    // }
    
    function deposit ( uint256 from, uint256 signer, uint amount) public returns (bool)
    {
        // transferFromSol(from, abi.encodePacked(erc20_account), amount);
        transferExt(from, signer, amount);
        _balances[erc20_acc_eth] = _balances[erc20_acc_eth] + amount;
        // _mint(to, amount);
        return true;    
    }
    
}