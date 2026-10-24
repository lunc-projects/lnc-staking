import { useCallback, useEffect, useState } from 'react';
import { useChain } from '@cosmos-kit/react';
import { CHAIN_NAME, CONTRACT_ADDRESS, FINDER, LNC, toRaw, toWhole, shortAddr } from './config.js';
import { queryConfig, queryPosition, queryRewards, stakeLnc, unstake, claimAll, adminSetRewardRate, adminFundPool, adminAddReward } from './lib/stakingClient.js';

const label = (addr) => (addr === LNC.address ? 'LNC' : shortAddr(addr));
const addrLink = (addr) => (
  <a target="_blank" rel="noopener noreferrer" href={`${FINDER}/address/${addr}`}>
    <span className="mono">{addr}</span>
  </a>
);

export default function App() {
  const chain = useChain(CHAIN_NAME);
  const connected = chain.status === 'Connected' && chain.address;
  const address = chain.address || '';

  const [staked, setStaked] = useState(null);
  const [pending, setPending] = useState([]);
  const [rewards, setRewards] = useState([]);
  const [isAdmin, setIsAdmin] = useState(false);
  const [log, setLog] = useState('');
  const [stakeAmount, setStakeAmount] = useState('');
  const [unstakeAmount, setUnstakeAmount] = useState('');
  const [rateToken, setRateToken] = useState(LNC.address);
  const [rateValue, setRateValue] = useState('');
  const [fundToken, setFundToken] = useState(LNC.address);
  const [fundAmount, setFundAmount] = useState('');
  const [addToken, setAddToken] = useState('');

  const pushLog = useCallback((m) => setLog((prev) => (prev ? `${prev}\n${m}` : m)), []);

  const refresh = useCallback(async () => {
    if (!connected) { pushLog('Connect a wallet first.'); return; }
    try {
      const pos = await queryPosition(address);
      setStaked(pos.amount);
      setPending((pos.pending || []).filter((p) => Number(p.amount) > 0));
      const rw = await queryRewards();
      setRewards(rw);
      const cfg = await queryConfig();
      const admin = cfg.admin === address;
      setIsAdmin(admin);
      pushLog(admin ? '🍬 Owner panel unlocked — you are the contract admin.' : 'Connected. (Owner panel is hidden — you are not the contract admin.)');
    } catch (e) {
      pushLog(`Refresh failed: ${e.message}`);
    }
  }, [connected, address, pushLog]);

  // Refresh automatically once a wallet connects (including a restored
  // WalletConnect session, where Cosmos Kit reconnects on load).
  useEffect(() => {
    if (connected) refresh();
    else { setStaked(null); setPending([]); setRewards([]); setIsAdmin(false); }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected]);

  const signingClient = async () => {
    if (!chain.getSigningCosmWasmClient) throw new Error('Connect a wallet first.');
    return chain.getSigningCosmWasmClient();
  };

  const run = async (action, fn) => {
    try {
      const client = await signingClient();
      const res = await fn(client);
      pushLog(`${action} ✅ Tx: ${res.transactionHash} — ${FINDER}/tx/${res.transactionHash}`);
      await refresh();
    } catch (e) {
      pushLog(`${action} failed: ${e.message}`);
    }
  };

  const rewardTokens = rewards.length ? rewards.map((r) => r.reward_token) : [LNC.address];

  return (
    <main>
      <div className="hero">
        <img src="logo.jpg" alt="LNC candy logo" />
        <h1>🍭 LNC Staking</h1>
        <p className="tagline">Stake LNC on Luna Classic. Sweet and simple. 🍬</p>
      </div>

      <div className="card">
        {connected ? (
          <>
            <p><span className="muted">Connected: </span><span className="mono">{address}</span></p>
            <button className="purple" onClick={() => chain.disconnect()}>Disconnect</button>
          </>
        ) : (
          <button className="pink" onClick={() => chain.connect()}>🍭 Connect wallet</button>
        )}
        <p className="muted">{connected ? 'Wallet connected — pick a wallet in the modal to switch.' : 'Connect with Keplr, Station, or Galaxy Station. On your phone, pick a WalletConnect wallet and approve in the wallet app.'}</p>
      </div>

      <div className="card">
        <h2>🍬 Your position</h2>
        <p>Staked: <span className="big">{connected ? (staked === null ? '…' : `${toWhole(staked)}`) : '—'}</span> LNC</p>
        <p>Pending rewards:</p>
        <ul>
          {connected ? (
            pending.length ? pending.map((p) => (
              <li key={p.reward_token}>{toWhole(p.amount)} {label(p.reward_token)}{p.reward_token !== LNC.address && (<><br />{addrLink(p.reward_token)}</>)}</li>
            )) : (<li className="muted">No pending rewards yet.</li>)
          ) : (<li className="muted">Connect your wallet to see your rewards.</li>)}
        </ul>
        <button className="mint" onClick={refresh}>🔄 Refresh</button>
      </div>

      <div className="card">
        <h2>🍭 Stake LNC</h2>
        <input type="number" min="0" step="any" placeholder="Amount in LNC" value={stakeAmount} onChange={(e) => setStakeAmount(e.target.value)} />
        <button className="pink" onClick={() => {
          if (!stakeAmount) { pushLog('Enter a stake amount.'); return; }
          run('Staked', (c) => stakeLnc(c, address, toRaw(stakeAmount)));
        }}>Stake my LNC 🍬</button>
      </div>

      <div className="card">
        <h2>↩️ Unstake</h2>
        <input type="number" min="0" step="any" placeholder="Amount in LNC" value={unstakeAmount} onChange={(e) => setUnstakeAmount(e.target.value)} />
        <button className="purple" onClick={() => {
          if (!unstakeAmount) { pushLog('Enter an unstake amount.'); return; }
          run('Unstaked', (c) => unstake(c, address, toRaw(unstakeAmount)));
        }}>Unstake</button>
        <p className="muted">Unstaking never blocks — your staked LNC always comes back, and rewards you&apos;ve already earned stay claimable.</p>
      </div>

      <div className="card">
        <h2>🎁 Rewards</h2>
        <button className="mint" onClick={() => run('Claimed', (c) => claimAll(c, address))}>Claim all rewards 🎉</button>
        <h3>Reward tokens</h3>
        <ul>
          {connected ? (
            rewards.length ? rewards.map((r) => {
              const rate = Number(r.rate_per_token_per_day);
              const pool = Number(r.pool_balance);
              return (
                <li key={r.reward_token}>
                  <b>{label(r.reward_token)}</b>
                  {r.reward_token !== LNC.address && (<><br />{addrLink(r.reward_token)}</>)}
                  <br />{rate === 0 ? '🍬 Rewards not started yet — rate is 0.' : `Rate: ${rate.toLocaleString()} raw units per staked LNC per day (≈ ${(rate / 1_000_000).toLocaleString()} ${label(r.reward_token)} per staked LNC per day).`}
                  <br />{pool === 0 ? 'Pool empty — nothing funded yet.' : `Pool: ${toWhole(r.pool_balance)} ${label(r.reward_token)} funded.`}
                </li>
              );
            }) : (<li className="muted">No reward tokens registered yet.</li>)
          ) : (<li className="muted">Connect your wallet to see the rewards on offer.</li>)}
        </ul>
      </div>

      {connected && isAdmin && (
        <div className="card admin">
          <h2>🍬 Owner panel — add rewards</h2>
          <p className="muted">Only the contract admin sees this panel.</p>

          <h3>1 · Set reward rate</h3>
          <label className="muted" htmlFor="rate-token">Reward token</label>
          <select id="rate-token" value={rateToken} onChange={(e) => setRateToken(e.target.value)}>
            {rewardTokens.map((t) => (<option key={t} value={t}>{label(t)} — {t}</option>))}
          </select>
          <label className="muted" htmlFor="rate-value">Rate — raw units per 1 whole staked token per day</label>
          <input id="rate-value" type="number" min="0" step="any" placeholder="e.g. 1000000" value={rateValue} onChange={(e) => setRateValue(e.target.value)} />
          <div className="note">
            What the number means: LNC has 6 decimals, so 1,000,000 raw units = 1 whole LNC.
            A rate of 1,000,000 means each staked LNC earns 1 LNC per day. A rate of 0 means
            that reward is off. Rewards only ever pay out of the funded pool — never more than what&apos;s in it.
          </div>
          <button className="pink" onClick={() => {
            if (!rateValue) { pushLog('Enter a rate.'); return; }
            run('Rate set', (c) => adminSetRewardRate(c, address, rateToken, rateValue));
          }}>Set rate</button>

          <h3>2 · Fund the pool</h3>
          <label className="muted" htmlFor="fund-token">Reward token</label>
          <select id="fund-token" value={fundToken} onChange={(e) => setFundToken(e.target.value)}>
            {rewardTokens.map((t) => (<option key={t} value={t}>{label(t)} — {t}</option>))}
          </select>
          <label className="muted" htmlFor="fund-amount">Amount</label>
          <input id="fund-amount" type="number" min="0" step="any" placeholder="Amount of the token" value={fundAmount} onChange={(e) => setFundAmount(e.target.value)} />
          <button className="mint" onClick={() => {
            if (!fundAmount) { pushLog('Enter a fund amount.'); return; }
            run('Pool funded', (c) => adminFundPool(c, address, fundToken, toRaw(fundAmount)));
          }}>Fund pool 🍬</button>
          <p className="muted">Funding sends your tokens straight into the contract&apos;s reward pool. The token must already be registered below (LNC is registered from the start).</p>

          <h3>3 · Add a reward token</h3>
          <label className="muted" htmlFor="add-token">Token contract address</label>
          <input id="add-token" type="text" placeholder="terra1…" value={addToken} onChange={(e) => setAddToken(e.target.value)} />
          <button className="purple" onClick={() => {
            if (!addToken.startsWith('terra1')) { pushLog('Enter a valid token contract address (terra1…).'); return; }
            run('Reward token added', (c) => adminAddReward(c, address, addToken));
          }}>Add reward token</button>
          <p className="muted">New tokens start at rate 0 — set their rate and fund them afterwards.</p>
        </div>
      )}

      {log && <p id="log">{log}</p>}
      <p className="muted" style={{ textAlign: 'center' }}>🍭 LNC on Luna Classic · No seed phrase or private key is ever asked for — you approve every transaction in your own wallet.</p>
      <p className="muted" style={{ textAlign: 'center' }}>Contract: {addrLink(CONTRACT_ADDRESS)}</p>
    </main>
  );
}
