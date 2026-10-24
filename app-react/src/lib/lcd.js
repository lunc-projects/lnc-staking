// Plain-fetch LCD helpers — no SDK, nothing to break in a browser.
// All contract reads go through here via base64-encoded smart queries.
import { LCD_URL } from '../config.js';

export function b64(obj) {
  return btoa(JSON.stringify(obj));
}

export async function lcdSmart(contract, msg) {
  const res = await fetch(`${LCD_URL}/cosmwasm/wasm/v1/contract/${contract}/smart/${b64(msg)}`);
  if (!res.ok) throw new Error(`LCD query failed (${res.status}) for ${contract}`);
  const data = await res.json();
  return data.data;
}
