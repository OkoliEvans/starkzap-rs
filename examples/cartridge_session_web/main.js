import { constants, getChecksumAddress, hash } from "starknet";
import SessionProvider from "@cartridge/controller/session";

const connectBtn = document.getElementById("connectBtn");
const copyBtn = document.getElementById("copyBtn");
const status = document.getElementById("status");
const output = document.getElementById("output");
const rpcUrl = document.getElementById("rpcUrl");
const chainId = document.getElementById("chainId");
const tokenAddress = document.getElementById("tokenAddress");
const entrypoint = document.getElementById("entrypoint");
const authMethod = document.getElementById("authMethod");

function setStatus(message, isError = false) {
  status.textContent = message;
  status.className = `status ${isError ? "error" : "ok"}`;
}

function getPolicies() {
  const method = entrypoint.value.trim();
  return {
    contracts: {
      [tokenAddress.value.trim()]: {
        name: "Policy Test Contract",
        description: "Allows the Rust backend to test the approved session",
        methods: [{ name: method, entrypoint: method }],
      },
    },
  };
}

function getPolicyArray() {
  const method = entrypoint.value.trim();
  const contract = getChecksumAddress(tokenAddress.value.trim());
  return [{ target: contract, method: hash.getSelectorFromName(method), authorized: true }];
}

async function connectAndExport() {
  output.value = "";
  setStatus("Opening Cartridge auth and session approval...");

  const provider = new SessionProvider({
    rpc: rpcUrl.value.trim(),
    chainId:
      chainId.value === "SN_MAIN"
        ? constants.StarknetChainId.SN_MAIN
        : constants.StarknetChainId.SN_SEPOLIA,
    policies: getPolicies(),
    redirectUrl: window.location.href,
    signupOptions: [authMethod.value],
  });

  const account = await provider.connect();
  if (!account) {
    throw new Error("Cartridge did not return an account.");
  }

  const signerRaw = localStorage.getItem("sessionSigner");
  if (!signerRaw) {
    throw new Error("No sessionSigner found in localStorage after connect().");
  }

  const signer = JSON.parse(signerRaw);
  if (!signer?.privKey) {
    throw new Error("sessionSigner is missing privKey.");
  }

  const sessionRaw = localStorage.getItem("session");
  if (!sessionRaw) {
    throw new Error("No session registration found in localStorage after connect().");
  }

  const session = JSON.parse(sessionRaw);
  const bundle = {
    rpcUrl: rpcUrl.value.trim(),
    chainId:
      chainId.value === "SN_MAIN"
        ? constants.StarknetChainId.SN_MAIN
        : constants.StarknetChainId.SN_SEPOLIA,
    signer,
    session,
    policies: getPolicyArray(),
  };
  const bundleB64 = btoa(JSON.stringify(bundle));

  output.value = [
    `CARTRIDGE_SESSION_BUNDLE_B64=${bundleB64}`,
    `CARTRIDGE_ACCOUNT_ADDRESS=${account.address}`,
    "CARTRIDGE_RUN_TRANSFER=1",
    "CARTRIDGE_TRANSFER_AMOUNT=0.001",
    "RECIPIENT_ADDRESS=0x...",
  ].join("\n");

  setStatus("Session exported successfully.");
}

connectBtn.addEventListener("click", async () => {
  try {
    await connectAndExport();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    setStatus(message, true);
    console.error(error);
  }
});

copyBtn.addEventListener("click", async () => {
  if (!output.value.trim()) {
    setStatus("Nothing to copy yet.", true);
    return;
  }
  await navigator.clipboard.writeText(output.value);
  setStatus("Copied env block.");
});
