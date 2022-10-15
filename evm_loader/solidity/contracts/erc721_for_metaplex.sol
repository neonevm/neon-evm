// SPDX-License-Identifier: MIT

pragma solidity >= 0.8.0;

import './SPLToken.sol';
import './Metaplex.sol';

interface IERC165 {
    function supportsInterface(bytes4 interfaceId) external view returns (bool);
}

interface IERC721Receiver {
    function onERC721Received(address _operator, address _from, uint256 _tokenId, bytes memory _data) external returns(bytes4);
}

interface IERC721 is IERC165 {
    event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);
    event Approval(address indexed owner, address indexed approved, uint256 indexed tokenId);
    event ApprovalForAll(address indexed owner, address indexed operator, bool approved);


    function balanceOf(address owner) external view returns (uint256 balance);
    function ownerOf(uint256 tokenId) external view returns (address owner);

    function safeTransferFrom(address from, address to, uint256 tokenId, bytes calldata data) external;
    function safeTransferFrom(address from, address to, uint256 tokenId) external;

    function transferFrom(address from, address to, uint256 tokenId) external;

    function approve(address to, uint256 tokenId) external;
    function setApprovalForAll(address operator, bool _approved) external;
    function getApproved(uint256 tokenId) external view returns (address operator);
    function isApprovedForAll(address owner, address operator) external view returns (bool);
}

interface IERC721Metadata is IERC721 {
    function name() external view returns (string memory);
    function symbol() external view returns (string memory);
    function tokenURI(uint256 tokenId) external view returns (string memory);
}


contract ERC721ForMetaplex is IERC165, IERC721, IERC721Metadata {
    SPLToken constant _splToken = SPLToken(0xFf00000000000000000000000000000000000004);
    Metaplex constant _metaplex = Metaplex(0xff00000000000000000000000000000000000005);

    string private _name = "Metaplex";
    string private _symbol = "MPL";

    // Mapping from token ID to owner address
    mapping(uint256 => address) private _owners;

    // Mapping owner address to token count
    mapping(address => uint256) private _balances;

    // Mapping from token ID to approved address
    mapping(uint256 => address) private _tokenApprovals;

    // Mapping from owner to operator approvals
    mapping(address => mapping(address => bool)) private _operatorApprovals;


    function supportsInterface(bytes4 interfaceId) external pure override returns (bool) {
        return
            interfaceId == type(IERC721).interfaceId ||
            interfaceId == type(IERC721Metadata).interfaceId ||
            interfaceId == type(IERC165).interfaceId;
    }

    function name() public view override returns (string memory) {
        return _name;
    }

    function symbol() public view override returns (string memory) {
        return _symbol;
    }

    function tokenURI(uint256 tokenId) public view override returns (string memory) {
        require(_metaplex.isNFT(bytes32(tokenId)), "ERC721: invalid token ID");

        return _metaplex.uri(bytes32(tokenId));
    }

    function claim(bytes32 from, uint64 amount) external returns (bool) {
        require(amount == 1, "ERC721: invalid spl token amount");

        SPLToken.Account memory account = _splToken.getAccount(from);
        require(account.state == SPLToken.AccountState.Initialized, "ERC721: invalid spl token account state");
        require(_metaplex.isNFT(account.mint), "ERC721: spl token is not NFT");

        bytes32 seed = keccak256(abi.encode(account.mint, msg.sender));
        bytes32 toSolana = _splToken.findAccount(seed);
        if (!_splToken.exists(toSolana)) {
            _splToken.initializeAccount(seed, account.mint);
        }

        // spl-token transaction will be signed by tx.origin
        // this is only allowed in top level contract
        (bool status, ) = address(_splToken).delegatecall(
            abi.encodeWithSignature("transfer(bytes32,bytes32,uint64)", from, toSolana, amount)
        );
        require(status, "ERC721: claim failed");


        uint256 tokenId = uint256(account.mint);
        _balances[msg.sender] += 1;
        _owners[tokenId] = msg.sender;

        emit Transfer(address(0), msg.sender, tokenId);

        return true;
    }

    function mint(bytes32 seed, address to, string memory uri) public returns (uint256) {
        require(to != address(0), "ERC721: mint to the zero address");

        bytes32 mintId = _splToken.initializeMint(seed, 0);
        
        bytes32 tokenSeed = keccak256(abi.encode(mintId, to));
        bytes32 account = _splToken.initializeAccount(tokenSeed, mintId);

        _splToken.mintTo(account, 1);

        _metaplex.createMetadata(mintId, _name, _symbol, uri);
        _metaplex.createMasterEdition(mintId, 0);


        uint256 tokenId = uint256(mintId);

        _balances[to] += 1;
        _owners[tokenId] = to;

        emit Transfer(address(0), to, tokenId);

        return tokenId;
    }

    function safeMint(bytes32 seed, address to, string memory uri) public returns (uint256) {
        return safeMint(seed, to, uri, "");
    }
    
    function safeMint(bytes32 seed, address to, string memory uri, bytes memory data) public returns (uint256) {
        uint256 tokenId = mint(seed, to, uri);

        require(
            _checkOnERC721Received(address(0), to, tokenId, data),
            "ERC721: transfer to non ERC721Receiver implementer"
        );

        return tokenId;
    }

    function balanceOf(address owner) public view override returns (uint256) {
        require(owner != address(0), "ERC721: address zero is not a valid owner");
        return _balances[owner];
    }

    function ownerOf(uint256 tokenId) public view override returns (address) {
        address owner = _owners[tokenId];
        require(owner != address(0), "ERC721: invalid token ID");

        return owner;
    }

    function safeTransferFrom(address from, address to, uint256 tokenId) public override {
        safeTransferFrom(from, to, tokenId, "");
    }

    function safeTransferFrom(address from, address to, uint256 tokenId, bytes memory data) public override {
        require(_isApprovedOrOwner(msg.sender, tokenId), "ERC721: caller is not token owner nor approved");

        _transfer(from, to, tokenId);
        require(_checkOnERC721Received(from, to, tokenId, data), "ERC721: transfer to non ERC721Receiver implementer");
    }

    function transferFrom(address from, address to, uint256 tokenId) public override {
        require(_isApprovedOrOwner(msg.sender, tokenId), "ERC721: caller is not token owner nor approved");

        _transfer(from, to, tokenId);
    }

    function transferSolanaFrom(address from, bytes32 to, uint256 tokenId) public {
        require(_isApprovedOrOwner(msg.sender, tokenId), "ERC721: caller is not token owner nor approved");
        require(ownerOf(tokenId) == from, "ERC721: transfer from incorrect owner");
        require(to != bytes32(0), "ERC721: transfer to the zero address");

        delete _tokenApprovals[tokenId];
        delete _owners[tokenId];
        _balances[from] -= 1;


        bytes32 fromSolana = _splToken.findAccount(keccak256(abi.encode(tokenId, from)));

        _splToken.transfer(fromSolana, to, 1);
        _splToken.closeAccount(fromSolana);

        emit Transfer(from, address(0), tokenId);
    }

    function approve(address to, uint256 tokenId) public override {
        address owner = ownerOf(tokenId);

        require(to != owner, "ERC721: approval to current owner");

        require(
            msg.sender == owner || isApprovedForAll(owner, msg.sender),
            "ERC721: approve caller is not token owner nor approved for all"
        );

        _tokenApprovals[tokenId] = to;
        emit Approval(owner, to, tokenId);
    }

    function setApprovalForAll(address operator, bool approved) public override {
        address owner = msg.sender;

        require(owner != operator, "ERC721: approve to caller");

        _operatorApprovals[owner][operator] = approved;
        emit ApprovalForAll(owner, operator, approved);
    }

    function getApproved(uint256 tokenId) public view override returns (address) {
        _requireMinted(tokenId);

        return _tokenApprovals[tokenId];
    }

    function isApprovedForAll(address owner, address operator) public view override returns (bool) {
        return _operatorApprovals[owner][operator];
    }


    function _isApprovedOrOwner(address spender, uint256 tokenId) internal view virtual returns (bool) {
        address owner = ownerOf(tokenId);

        return (spender == owner || isApprovedForAll(owner, spender) || getApproved(tokenId) == spender);
    }

    function _requireMinted(uint256 tokenId) internal view {
        require(_owners[tokenId] != address(0), "ERC721: invalid token ID");
    }

    function _transfer(address from, address to, uint256 tokenId) internal virtual {
        require(ownerOf(tokenId) == from, "ERC721: transfer from incorrect owner");
        require(to != address(0), "ERC721: transfer to the zero address");

        // Clear approvals from the previous owner
        delete _tokenApprovals[tokenId];

        _balances[from] -= 1;
        _balances[to] += 1;
        _owners[tokenId] = to;


        bytes32 fromSeed = keccak256(abi.encode(tokenId, from));
        bytes32 fromSolana = _splToken.findAccount(fromSeed);

        bytes32 toSeed = keccak256(abi.encode(tokenId, to));
        bytes32 toSolana = _splToken.findAccount(toSeed);

        if (!_splToken.exists(toSolana)) {
            _splToken.initializeAccount(toSeed, bytes32(tokenId));
        }

        _splToken.transfer(fromSolana, toSolana, 1);
        _splToken.closeAccount(fromSolana);

        emit Transfer(from, to, tokenId);
    }

    function _checkOnERC721Received(address from, address to, uint256 tokenId, bytes memory data) private returns (bool) {
        if (to.code.length > 0) {
            try IERC721Receiver(to).onERC721Received(msg.sender, from, tokenId, data) returns (bytes4 retval) {
                return retval == IERC721Receiver.onERC721Received.selector;
            } catch (bytes memory reason) {
                if (reason.length == 0) {
                    revert("ERC721: transfer to non ERC721Receiver implementer");
                } else {
                    /// @solidity memory-safe-assembly
                    assembly {
                        revert(add(32, reason), mload(reason))
                    }
                }
            }
        } else {
            return true;
        }
    }
}