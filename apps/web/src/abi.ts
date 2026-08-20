export const migrationClaimAbi = [
  {
    type: "function",
    name: "claim",
    stateMutability: "nonpayable",
    inputs: [
      {
        name: "data",
        type: "tuple",
        components: [
          { name: "standard", type: "uint8" },
          { name: "tokenId", type: "uint256" },
          { name: "amount", type: "uint256" },
          { name: "sourceOwner", type: "address" },
          { name: "recipient", type: "address" },
          { name: "leafIndex", type: "uint256" },
        ],
      },
      { name: "proof", type: "bytes32[]" },
    ],
    outputs: [],
  },
  {
    type: "function",
    name: "claimBatch",
    stateMutability: "nonpayable",
    inputs: [
      {
        name: "data",
        type: "tuple[]",
        components: [
          { name: "standard", type: "uint8" },
          { name: "tokenId", type: "uint256" },
          { name: "amount", type: "uint256" },
          { name: "sourceOwner", type: "address" },
          { name: "recipient", type: "address" },
          { name: "leafIndex", type: "uint256" },
        ],
      },
      { name: "proof", type: "bytes32[]" },
      { name: "proofFlags", type: "bool[]" },
    ],
    outputs: [],
  },
  {
    type: "function",
    name: "claimDelegated",
    stateMutability: "nonpayable",
    inputs: [
      {
        name: "data",
        type: "tuple",
        components: [
          { name: "standard", type: "uint8" },
          { name: "tokenId", type: "uint256" },
          { name: "amount", type: "uint256" },
          { name: "sourceOwner", type: "address" },
          { name: "recipient", type: "address" },
          { name: "leafIndex", type: "uint256" },
        ],
      },
      { name: "proof", type: "bytes32[]" },
      { name: "nonce", type: "uint256" },
      { name: "deadline", type: "uint256" },
      { name: "signature", type: "bytes" },
    ],
    outputs: [],
  },
  {
    type: "function",
    name: "isClaimed",
    stateMutability: "view",
    inputs: [
      { name: "version", type: "uint64" },
      { name: "leafIndex", type: "uint256" },
    ],
    outputs: [{ name: "", type: "bool" }],
  },
  {
    type: "function",
    name: "claimedCount",
    stateMutability: "view",
    inputs: [],
    outputs: [{ name: "", type: "uint256" }],
  },
  {
    type: "function",
    name: "nonces",
    stateMutability: "view",
    inputs: [{ name: "owner", type: "address" }],
    outputs: [{ name: "", type: "uint256" }],
  },
] as const;
