// LNC Staking Rewards — standalone app skeleton.
//
// NOT DEPLOYED: CONTRACT_ADDRESS is empty on purpose. The UI refuses to
// send any transaction and shows no balances until the owner sets the real
// contract address after deploy (see ../DEPLOY.md step 5). No fake data
// is ever displayed.

import { SigningCosmWasmClient } from "https://esm.sh/@cosmjs/cosmwasm-stargate@0.32.3";
import { GasPrice } from "https://esm.sh/@cosmjs/stargate@0.32.3";

const CONTRACT_ADDRESS = ""; // <-- set after deploy; empty = not deployed
const LNC = "terra1nuwdr5zaul2gnq88xapq8ythmaw4092kt8lxsl6jd36h47u8vpcqnrmdcm";
const CHAIN_ID = "columbus-5";
const RPC = "https://terra-classic-rpc.publicnode.com";
const LCD = "https://terra-classic-lcd.publicnode.com";
const DECIMALS = 1_000_000;

const $ = (id) => document.getElementById(id);
const log = (m) => { $("log").textContent += m + "\n"; };

let client = null;
let address = "";

function requireDeployed() {
  if (!CONTRACT_ADDRESS) {
    log("Contract not deployed yet — set CONTRACT_ADDRESS in app.js after deploy.");
    return false;
  }
  return true;
}

function toRaw(lNC) { return String(Math.round(parseFloat(lNC) * DECIMALS)); }
function toWhole(raw) { return (Number(raw) / DECIMALS).toLocaleString(); }

$("connect").onclick = async () => {
  if (!window.keplr) { log("Keplr not found in this browser."); return; }
  await window.keplr.enable(CHAIN_ID);
  const signer = window.keplr.getOfflineSigner(CHAIN_ID);
  const accounts = await signer.getAccounts();
  address = accounts[0].address;
  client = await SigningCosmWasmClient.connectWithSigner(RPC, signer, {
    gasPrice: GasPrice.fromString("0.15uluna"),
  });
  $("wallet").textContent = address;
  log("Connected: " + address);
};

$("refresh").onclick = async () => {
  if (!requireDeployed() || !client || !address) { log("Connect a wallet first."); return; }
  const pos = await client.queryContractSmart(CONTRACT_ADDRESS, { position: { owner: address } });
  $("staked").textContent = toWhole(pos.amount);
  const lnc = pos.pending.find((p) => p.reward_token === LNC);
  $("pending").textContent = lnc ? toWhole(lnc.amount) : "0";
};

$("stake").onclick = async () => {
  if (!requireDeployed() || !client || !address) { log("Connect a wallet first."); return; }
  const amount = toRaw($("stake-amount").value);
  if (!amount || amount === "0") { log("Enter a stake amount."); return; }
  // CW20 Send with {"stake":{}} — the contract stakes on receipt.
  const msg = { send: { contract: CONTRACT_ADDRESS, amount, msg: btoa(JSON.stringify({ stake: {} })) } };
  const res = await client.execute(address, LNC, msg, "auto");
  log("Staked. Tx: " + res.transactionHash);
};

$("unstake").onclick = async () => {
  if (!requireDeployed() || !client || !address) { log("Connect a wallet first."); return; }
  const amount = toRaw($("unstake-amount").value);
  if (!amount || amount === "0") { log("Enter an unstake amount."); return; }
  const res = await client.execute(address, CONTRACT_ADDRESS, { unstake: { amount } }, "auto");
  log("Unstaked. Tx: " + res.transactionHash);
};

$("claim").onclick = async () => {
  if (!requireDeployed() || !client || !address) { log("Connect a wallet first."); return; }
  const res = await client.execute(address, CONTRACT_ADDRESS, { claim_all: {} }, "auto");
  log("Claimed. Tx: " + res.transactionHash);
};

// LCD base (public, read-only) for reference; the signing client uses RPC above.
void LCD;
