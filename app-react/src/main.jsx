import './polyfills.js';
import React from 'react';
import { createRoot } from 'react-dom/client';
import { ChainProvider } from '@cosmos-kit/react';
import { wallets as keplrWallets } from '@cosmos-kit/keplr';
import { wallets as stationWallets } from '@cosmos-kit/station';
import { wallets as terraClassicWcWallets } from './wallets.js';
import { terraClassicChain, terraClassicAssets } from './chainDef.js';
import { GasPrice } from '@cosmjs/stargate';
import App from './App.jsx';
// REQUIRED: the Cosmos Kit wallet modal is built from @interchain-ui/react
// components whose entire stylesheet ships as a separate CSS file that the
// app must import. Without it the icons render at unconstrained (giant)
// sizes and the modal chrome is unstyled. (Same requirement as the FRG app.)
import '@interchain-ui/react/styles';
import './styles.css';

// WalletConnect (Reown) project ID — the dApp's own public client ID
// (it ships in the browser bundle). Required for every WalletConnect
// wallet (Keplr Mobile, Galaxy Station) to initialize its sign client;
// without it WC wallets fail with "projectId is not provided".
const WC_PROJECT_ID = '7e363f1ed5d676ecc4ae2d40c5b83fbf';

createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <ChainProvider
      chains={[terraClassicChain]}
      assetLists={[terraClassicAssets]}
      wallets={[...keplrWallets, ...stationWallets, ...terraClassicWcWallets]}
      walletConnectOptions={{
        signClient: {
          projectId: WC_PROJECT_ID,
          metadata: {
            name: 'LNC Staking',
            description: 'Stake LNC, earn rewards on Luna Classic',
            url: 'https://lunc-projects.github.io/lnc-staking/app/',
            icons: ['https://lunc-projects.github.io/lnc-staking/app/logo.jpg'],
            redirect: {
              native: 'https://lunc-projects.github.io/lnc-staking/app/',
              universal: 'https://lunc-projects.github.io/lnc-staking/app/',
            },
          },
        },
      }}
      signerOptions={{
        // Explicit gas price for the signing client: CosmJS throws when
        // 'auto' fees are used without one. 0.15uluna, as used by the
        // store page and the previous vanilla app.
        signingCosmwasm: () => ({ gasPrice: GasPrice.fromString('0.15uluna') }),
      }}
      modalTheme={{ defaultTheme: 'dark' }}
    >
      <App />
    </ChainProvider>
  </React.StrictMode>
);
