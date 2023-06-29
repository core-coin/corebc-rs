//SPDX-License-Identifier: Unlicense
pragma solidity ^1.0.0;

contract Greeter {
    string private greeting;

    constructor(string memory _greeting) public {
        greeting = _greeting;
    }

    function greet() public view returns (string memory) {
        return greeting;
    }

    function setGreeting(string memory _greeting) public {
        greeting = _greeting;
    }
}
