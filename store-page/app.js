/* One-click StoreCode page for the LNC staking contract (Terra Classic).
 *
 * No build step. CosmJS 0.32.4 via esm.sh (verified resolving 2026-10-24).
 * Nothing is sent on-chain until the user taps Upload AND approves in their wallet.
 * No seeds, mnemonics, or private keys are handled anywhere on this page.
 */
import { SigningCosmWasmClient } from "https://esm.sh/@cosmjs/cosmwasm-stargate@0.32.4";
import { GasPrice } from "https://esm.sh/@cosmjs/stargate@0.32.4";

const CHAIN_ID = "columbus-5";
const RPC = "https://terra-classic-rpc.publicnode.com";
const LCD = "https://terra-classic-lcd.publicnode.com";
const FINDER = "https://finder.terraclassic.community/mainnet";
const DESIGNATED_SENDER = "terra17ntyhh0242y2tjmq37zjgl0chysu84s9hkph69";
const EXPECTED_SHA256 = "9993d22a5fb506945f1658809fcce040a0e318d40c455c1423ca01e53117b4ec";
const WASM_FILE = "lnc_staking.wasm";
// 3 = AccessType.ACCESS_TYPE_EVERYBODY (cosmwasm.wasm.v1). The enum itself is
// not re-exported by @cosmjs/cosmwasm-stargate, so the numeric value is used
// with this comment as its provenance.
const PERMISSION_EVERYBODY = 3;

const $ = (id) => document.getElementById(id);
let wasmBytes = null;
let fileHashOk = false;
let connectedAddress = null;

function setStatus(text, isError = false) {
  const el = $("status");
  el.textContent = text;
  el.style.color = isError ? "#f87171" : "";
}

function refreshUploadButton() {
  $("uploadBtn").disabled = !(connectedAddress === DESIGNATED_SENDER && fileHashOk);
}

async function sha256Hex(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

// ---- Step 2: verify the served wasm file (runs on page load) ----
async function verifyFile() {
  try {
    const res = await fetch(WASM_FILE);
    if (!res.ok) throw new Error(`Could not download ${WASM_FILE} (HTTP ${res.status}). Host all three files together.`);
    const buf = await res.arrayBuffer();
    wasmBytes = new Uint8Array(buf);
    const hash = await sha256Hex(wasmBytes);
    fileHashOk = hash === EXPECTED_SHA256;
    $("fileInfo").innerHTML = fileHashOk
      ? `<span class="ok">✅ File matches.</span> ${wasmBytes.length.toLocaleString()} bytes<br><span class="mono">${hash}</span>`
      : `<span class="bad">❌ FILE MISMATCH — upload disabled.</span><br>Got: <span class="mono">${hash}</span><br>Expected: <span class="mono">${EXPECTED_SHA256}</span>`;
  } catch (e) {
    fileHashOk = false;
    $("fileInfo").innerHTML = `<span class="bad">❌ File check failed: ${e.message}</span>`;
  }
  refreshUploadButton();
}

// ---- Step 1: wallet ----
const CHAIN_INFO = {
  chainId: CHAIN_ID,
  chainName: "Terra Classic",
  rpc: RPC,
  rest: LCD,
  bip44: { coinType: 330 },
  bech32Config: {
    bech32PrefixAccAddr: "terra",
    bech32PrefixAccPub: "terrapub",
    bech32PrefixValAddr: "terravaloper",
    bech32PrefixValPub: "terravaloperpub",
    bech32PrefixConsAddr: "terravalcons",
    bech32PrefixConsPub: "terravalconspub",
  },
  currencies: [{ coinDenom: "LUNC", coinMinimalDenom: "uluna", coinDecimals: 6 }],
  feeCurrencies: [{ coinDenom: "LUNC", coinMinimalDenom: "uluna", coinDecimals: 6, gasPriceStep: { low: 0.15, average: 0.15, high: 0.15 } }],
  stakeCurrency: { coinDenom: "LUNC", coinMinimalDenom: "uluna", coinDecimals: 6 },
};

async function getOfflineSigner() {
  // Keplr first (the supported path).
  if (window.keplr) {
    try {
      await window.keplr.enable(CHAIN_ID);
    } catch (_) {
      // Chain not added to Keplr yet — suggest it, then enable.
      await window.keplr.experimentalSuggestChain(CHAIN_INFO);
      await window.keplr.enable(CHAIN_ID);
    }
    return window.keplr.getOfflineSigner(CHAIN_ID);
  }
  // Station: best-effort only. Station's injected API varies by version and
  // was NOT verified for this page — if it exposes a Keplr-compatible offline
  // signer, use it; otherwise fall through to the Keplr-required message.
  const st = window.station;
  if (st) {
    if (typeof st.getOfflineSigner === "function") {
      if (typeof st.enable === "function") await st.enable(CHAIN_ID);
      return st.getOfflineSigner(CHAIN_ID);
    }
    if (st.keplr && typeof st.keplr.getOfflineSigner === "function") {
      if (typeof st.keplr.enable === "function") await st.keplr.enable(CHAIN_ID);
      return st.keplr.getOfflineSigner(CHAIN_ID);
    }
  }
  throw new Error("No compatible wallet found. Please install Keplr and try again. (Station support on this page is untested — see README.)");
}

$("connectBtn").addEventListener("click", async () => {
  try {
    setStatus("Connecting…");
    const signer = await getOfflineSigner();
    const accounts = await signer.getAccounts();
    const match = accounts.find((a) => a.address === DESIGNATED_SENDER);
    if (!match) {
      const shown = accounts.map((a) => a.address).join(", ") || "(none)";
      connectedAddress = null;
      $("walletInfo").innerHTML =
        `<span class="bad">❌ Connected wallet is not the designated sender.</span><br>` +
        `Connected: <span class="mono">${shown}</span><br>` +
        `Required: <span class="mono">${DESIGNATED_SENDER}</span><br>` +
        `Switch to the designated wallet and reconnect. Upload stays disabled.`;
      setStatus("Wrong wallet — upload disabled.", true);
    } else {
      connectedAddress = DESIGNATED_SENDER;
      $("walletInfo").innerHTML =
        `<span class="ok">✅ Connected as the designated sender.</span><br><span class="mono">${connectedAddress}</span>`;
      setStatus("Wallet connected. Ready when the file check passes.");
    }
  } catch (e) {
    connectedAddress = null;
    $("walletInfo").innerHTML = `<span class="bad">❌ Connect failed: ${e.message}</span>`;
    setStatus(`Connect failed: ${e.message}`, true);
  }
  refreshUploadButton();
});

// ---- Step 3: upload ----
$("uploadBtn").addEventListener("click", async () => {
  if (connectedAddress !== DESIGNATED_SENDER || !fileHashOk || !wasmBytes) {
    setStatus("Upload blocked: wallet or file check has not passed.", true);
    return;
  }
  try {
    setStatus("Connecting signing client… (approve the transaction in your wallet when prompted)");
    $("uploadBtn").disabled = true;
    const signer = await getOfflineSigner();
    const client = await SigningCosmWasmClient.connectWithSigner(RPC, signer, {
      gasPrice: GasPrice.fromString("0.15uluna"),
    });
    const onChainId = await client.getChainId();
    if (onChainId !== CHAIN_ID) throw new Error(`Connected RPC reports chain ${onChainId}, expected ${CHAIN_ID}. Aborted.`);

    // CosmJS 0.32 upload order: (sender, wasmCode, fee, memo, instantiatePermission).
    const res = await client.upload(connectedAddress, wasmBytes, "auto", "", {
      permission: PERMISSION_EVERYBODY,
    });

    // Cross-check the on-chain record against the served file.
    let chainHash = null;
    try {
      const info = await (await fetch(`${LCD}/cosmwasm/wasm/v1/code/${res.codeId}`)).json();
      chainHash = (info.code_info && info.code_info.data_hash || "").toLowerCase();
    } catch (_) { /* best-effort; the tx result below is still authoritative */ }
    const checksumMatch = res.checksum && res.checksum.toLowerCase() === EXPECTED_SHA256;
    const chainMatch = chainHash ? chainHash === EXPECTED_SHA256 : null;

    $("resultCard").style.display = "block";
    $("result").innerHTML =
      `Transaction: <a target="_blank" rel="noopener" href="${FINDER}/tx/${res.transactionHash}"><span class="mono">${res.transactionHash}</span></a><br>` +
      `New code_id: <b>${res.codeId}</b><br>` +
      `Tx-returned checksum (sha256 of uploaded bytes): <span class="mono">${res.checksum}</span> ` +
      (checksumMatch ? `<span class="ok">✅ matches served file</span>` : `<span class="bad">❌ DOES NOT MATCH served file — investigate before using this code_id</span>`) + `<br>` +
      (chainHash
        ? `On-chain data_hash: <span class="mono">${chainHash}</span> ` +
          (chainMatch ? `<span class="ok">✅ matches served file</span>` : `<span class="bad">❌ DOES NOT MATCH</span>`) + `<br>`
        : `On-chain data_hash lookup failed — verify manually on Finder with the tx link above.<br>`) +
      `Gas used: ${res.gasUsed} / wanted: ${res.gasWanted}<br>` +
      `<b>Next:</b> instantiate later with this code_id and instantiate permission EVERYBODY (anyone may instantiate).`;
    setStatus("Upload confirmed on-chain. Record the code_id above.");
  } catch (e) {
    setStatus(`Upload failed: ${e.message}`, true);
  } finally {
    refreshUploadButton();
  }
});

verifyFile();
