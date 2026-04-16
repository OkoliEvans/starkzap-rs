import { CartridgeSessionAccount } from "@cartridge/controller-wasm/session";

const [bundleJson, callsJson] = process.argv.slice(2);

if (!bundleJson || !callsJson) {
  console.error("usage: node --experimental-wasm-modules cartridge_execute.mjs <bundle-json> <calls-json>");
  process.exit(1);
}

try {
  const bundle = JSON.parse(bundleJson);
  const calls = JSON.parse(callsJson);

  const account = CartridgeSessionAccount.newAsRegistered(
    bundle.rpcUrl,
    bundle.signer.privKey,
    bundle.session.address,
    bundle.session.ownerGuid,
    bundle.chainId,
    {
      policies: bundle.policies,
      expiresAt: Number(bundle.session.expiresAt),
      guardianKeyGuid: bundle.session.guardianKeyGuid ?? "0x0",
      metadataHash: bundle.session.metadataHash ?? "0x0",
      sessionKeyGuid: bundle.session.sessionKeyGuid,
    },
  );

  let result;
  try {
    result = await account.executeFromOutside(calls);
  } catch (outsideError) {
    result = await account.execute(calls);
  }

  process.stdout.write(JSON.stringify(result));
} catch (error) {
  const details =
    error && typeof error === "object"
      ? {
          name: error.name,
          message: error.message,
          code: error.code,
          data: error.data,
          stack: error.stack,
        }
      : { message: String(error) };
  process.stderr.write(JSON.stringify(details));
  process.exit(1);
}
