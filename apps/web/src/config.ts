import { QueryClient } from "@tanstack/react-query";
import { createConfig, http } from "wagmi";
import { injected } from "wagmi/connectors";
import { defineChain } from "viem";

const chainId = Number(import.meta.env.VITE_CHAIN_ID || 84532);
const rpcUrl = import.meta.env.VITE_RPC_URL || "https://sepolia.base.org";

export const targetChain = defineChain({
  id: chainId,
  name: import.meta.env.VITE_CHAIN_NAME || "Base Sepolia",
  nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
  rpcUrls: { default: { http: [rpcUrl] } },
  testnet: true,
});

export const wagmiConfig = createConfig({
  chains: [targetChain],
  connectors: [injected()],
  transports: { [targetChain.id]: http(rpcUrl) },
});

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 4_000, refetchOnWindowFocus: true, retry: 1 },
  },
});

export const claimAddress = (import.meta.env.VITE_CLAIM_ADDRESS ||
  "0x0000000000000000000000000000000000000000") as `0x${string}`;
export const campaignReady = !/^0x0{40}$/.test(claimAddress);
