import { ConnectKitButton, ConnectKitProvider } from "connectkit";

export default function WalletButton() {
  return (
    <ConnectKitProvider mode="dark">
      <ConnectKitButton />
    </ConnectKitProvider>
  );
}
