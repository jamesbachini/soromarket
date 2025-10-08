// SoroMarket Frontend - Stellar Soroban Integration

const CONFIG = {
    contractId: 'CA4YXIMAQNIUYAZC3ZRPV5GQSXT4QPXIANL6UYS5CN7FHKKJKDMO7D4M',
    rpcUrl: 'https://soroban-testnet.stellar.org',
    networkPassphrase: StellarSdk.Networks.TESTNET,
    decimals: 1000000 // 6 decimal places
};

// Global variables
let keypair = null;
let rpc = null;
let contract = null;
let isInitialized = false;
let oddsUpdateInterval = null;
let stakeValueUpdateInterval = null;

// Initialize RPC and Contract
function initializeStellar() {
    const RpcServer = StellarSdk.SorobanRpc ? StellarSdk.SorobanRpc.Server : StellarSdk.rpc.Server;
    rpc = new RpcServer(CONFIG.rpcUrl);
    contract = new StellarSdk.Contract(CONFIG.contractId);
}

// Utility functions
function showStatus(message, type = 'info') {
    const statusEl = document.getElementById('status-message') || document.getElementById('admin-status-message');
    if (!statusEl) return;

    statusEl.textContent = message;
    statusEl.className = `status-message ${type} show`;

    setTimeout(() => {
        statusEl.classList.remove('show');
    }, 5000);
}

function formatAmount(amount) {
    // Convert BigInt to Number if needed
    const numAmount = typeof amount === 'bigint' ? Number(amount) : (typeof amount === 'number' ? amount : 0);
    return (numAmount / CONFIG.decimals).toFixed(2);
}

function parseAmount(amount) {
    return Math.floor(parseFloat(amount) * CONFIG.decimals);
}

function formatAddress(address) {
    if (!address) return 'Not Connected';
    return `${address.slice(0, 6)}…${address.slice(-4)}`;
}

function formatDateTime(timestamp) {
    // Convert BigInt to Number if needed
    const numTimestamp = typeof timestamp === 'bigint' ? Number(timestamp) : timestamp;
    return new Date(numTimestamp * 1000).toLocaleString();
}

// Create and fund a testnet account
async function createFundedAccount() {
    try {
        const kp = StellarSdk.Keypair.random();
        showStatus('Creating new testnet account...', 'info');

        // Fund the account using Friendbot
        const response = await fetch(`https://friendbot.stellar.org/?addr=${kp.publicKey()}`);
        if (!response.ok) {
            throw new Error('Failed to fund account');
        }

        // Wait for account to be funded
        await new Promise(resolve => setTimeout(resolve, 3000));

        // Save wallet to localStorage
        saveUserWallet(kp.secret());

        showStatus('Account created and funded!', 'success');
        return kp;
    } catch (error) {
        console.error('Error creating account:', error);
        showStatus('Failed to create account', 'error');
        throw error;
    }
}

// Admin key management
function saveAdminKey(secretKey) {
    localStorage.setItem('soromarket_admin_key', secretKey);
}

function getAdminKey() {
    return localStorage.getItem('soromarket_admin_key');
}

function clearAdminKey() {
    localStorage.removeItem('soromarket_admin_key');
}

// User wallet management
function saveUserWallet(secretKey) {
    localStorage.setItem('soromarket_user_wallet', secretKey);
}

function getUserWallet() {
    return localStorage.getItem('soromarket_user_wallet');
}

function clearUserWallet() {
    localStorage.removeItem('soromarket_user_wallet');
}

// Connect admin wallet with secret key
async function connectAdminWallet(secretKey) {
    try {
        if (!secretKey) {
            throw new Error('Secret key is required');
        }

        // Validate secret key format
        if (!secretKey.startsWith('S') || secretKey.length !== 56) {
            throw new Error('Invalid secret key format');
        }

        showStatus('Connecting admin wallet...', 'info');

        keypair = StellarSdk.Keypair.fromSecret(secretKey);

        // Save to localStorage
        saveAdminKey(secretKey);

        // Update UI
        const addressEl = document.getElementById('admin-wallet-address');
        const connectBtn = document.getElementById('connect-admin-wallet');
        const clearBtn = document.getElementById('clear-admin-key');

        if (addressEl) addressEl.textContent = formatAddress(keypair.publicKey());
        if (connectBtn) {
            connectBtn.textContent = 'Connected';
            connectBtn.style.display = 'none';
        }
        if (clearBtn) clearBtn.style.display = 'inline-flex';

        showStatus('Admin wallet connected!', 'success');

    } catch (error) {
        console.error('Error connecting admin wallet:', error);
        showStatus('Failed to connect admin wallet: ' + error.message, 'error');
        throw error;
    }
}

// Connect wallet (for demo purposes, creates a new testnet account)
async function connectWallet() {
    try {
        if (keypair) {
            showStatus('Wallet already connected', 'info');
            return;
        }

        // Check if this is admin mode
        if (window.isAdminMode) {
            // Check if admin key is already stored
            const storedKey = getAdminKey();
            if (storedKey) {
                await connectAdminWallet(storedKey);
                return;
            } else {
                // Show admin key modal
                showAdminKeyModal();
                return;
            }
        }

        // Check if user wallet is already stored
        const storedWallet = getUserWallet();
        if (storedWallet) {
            keypair = StellarSdk.Keypair.fromSecret(storedWallet);
            showStatus('Wallet restored from storage', 'success');
        } else {
            keypair = await createFundedAccount();
        }

        // Update UI
        const addressEl = document.getElementById('wallet-address') || document.getElementById('admin-wallet-address');
        const connectBtn = document.getElementById('connect-wallet') || document.getElementById('connect-admin-wallet');

        if (addressEl) addressEl.textContent = formatAddress(keypair.publicKey());
        if (connectBtn) connectBtn.textContent = 'Connected';

        // Show wallet section and load user data
        const walletSection = document.getElementById('wallet-section');
        const myStakesSection = document.getElementById('my-stakes-section');
        if (walletSection) walletSection.style.display = 'block';
        if (myStakesSection) myStakesSection.style.display = 'block';

        await loadUserBalance();
        await loadUserStakes();

    } catch (error) {
        console.error('Error connecting wallet:', error);
        showStatus('Failed to connect wallet', 'error');
    }
}

// Admin key modal functions
function showAdminKeyModal() {
    const modal = document.getElementById('admin-key-modal');
    if (modal) {
        modal.classList.add('show');
        document.getElementById('admin-secret-key').focus();
    }
}

function hideAdminKeyModal() {
    const modal = document.getElementById('admin-key-modal');
    if (modal) {
        modal.classList.remove('show');
        document.getElementById('admin-secret-key').value = '';
    }
}

function disconnectAdminWallet() {
    clearAdminKey();
    keypair = null;

    const addressEl = document.getElementById('admin-wallet-address');
    const connectBtn = document.getElementById('connect-admin-wallet');
    const clearBtn = document.getElementById('clear-admin-key');

    if (addressEl) addressEl.textContent = 'Connect Admin Wallet';
    if (connectBtn) {
        connectBtn.textContent = 'Connect';
        connectBtn.style.display = 'inline-flex';
    }
    if (clearBtn) clearBtn.style.display = 'none';

    showStatus('Admin wallet disconnected', 'info');
}

// Contract interaction functions
async function callContract(method, ...args) {
    try {
        if (!keypair) {
            throw new Error('Wallet not connected');
        }

        const account = await rpc.getAccount(keypair.publicKey());

        // Convert arguments to ScVal based on type
        const scArgs = args.map(arg => {
            if (arg instanceof StellarSdk.Address) {
                return StellarSdk.nativeToScVal(arg.toString(), { type: 'address' });
            } else if (typeof arg === 'string') {
                // Determine if it's a symbol or string based on length/content
                if (arg.length <= 32 && /^[a-zA-Z0-9_]+$/.test(arg)) {
                    return StellarSdk.nativeToScVal(arg, { type: 'symbol' });
                }
                return StellarSdk.nativeToScVal(arg, { type: 'string' });
            } else if (typeof arg === 'bigint') {
                return StellarSdk.nativeToScVal(arg, { type: 'u64' });
            } else if (typeof arg === 'number') {
                return StellarSdk.nativeToScVal(arg, { type: 'u64' });
            }
            return arg;
        });

        // Build transaction
        let tx = new StellarSdk.TransactionBuilder(account, {
            fee: StellarSdk.BASE_FEE,
            networkPassphrase: CONFIG.networkPassphrase
        })
        .addOperation(contract.call(method, ...scArgs))
        .setTimeout(30)
        .build();

        // Prepare and simulate
        tx = await rpc.prepareTransaction(tx);

        // Sign transaction
        tx.sign(keypair);

        // Send transaction
        const result = await rpc.sendTransaction(tx);

        if (result.status === 'ERROR') {
            throw new Error(result.errorResultXdr || 'Transaction failed');
        }

        // Wait for confirmation
        let getResponse = await rpc.getTransaction(result.hash);
        while (getResponse.status === 'NOT_FOUND') {
            await new Promise(resolve => setTimeout(resolve, 1000));
            getResponse = await rpc.getTransaction(result.hash);
        }

        if (getResponse.status === 'SUCCESS') {
            return getResponse.returnValue;
        } else {
            throw new Error('Transaction failed');
        }
    } catch (error) {
        console.error(`Error calling ${method}:`, error);
        throw error;
    }
}

async function readContract(method, ...args) {
    try {
        // Convert arguments to ScVal based on type
        const scArgs = args.map(arg => {
            if (arg instanceof StellarSdk.Address) {
                return StellarSdk.nativeToScVal(arg.toString(), { type: 'address' });
            } else if (typeof arg === 'string') {
                // Determine if it's a symbol or string based on length/content
                if (arg.length <= 32 && /^[a-zA-Z0-9_]+$/.test(arg)) {
                    return StellarSdk.nativeToScVal(arg, { type: 'symbol' });
                }
                return StellarSdk.nativeToScVal(arg, { type: 'string' });
            } else if (typeof arg === 'bigint') {
                // For read operations, u64 is fine for market IDs
                return StellarSdk.nativeToScVal(Number(arg), { type: 'u64' });
            } else if (typeof arg === 'number') {
                return StellarSdk.nativeToScVal(arg, { type: 'u64' });
            }
            return arg;
        });

        const result = await rpc.simulateTransaction(
            new StellarSdk.TransactionBuilder(
                new StellarSdk.Account('GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF', '0'),
                { fee: '0', networkPassphrase: CONFIG.networkPassphrase }
            )
            .addOperation(contract.call(method, ...scArgs))
            .setTimeout(30)
            .build()
        );

        if (result.error) {
            return null;
        }

        return result.result?.retval;
    } catch (error) {
        // Silently return null for expected errors (like market not found)
        // Only log unexpected errors
        if (!error.message?.includes('UnreachableCodeReached')) {
            console.error(`Error reading ${method}:`, error);
        }
        return null;
    }
}

// Contract specific functions
async function initializeContract() {
    try {
        showStatus('Initializing contract...', 'info');
        const adminAddress = StellarSdk.Address.fromString(keypair.publicKey());
        await callContract('initialize', adminAddress);
        showStatus('Contract initialized successfully!', 'success');
        await checkContractStatus();
    } catch (error) {
        console.error('Error initializing contract:', error);
        showStatus('Failed to initialize contract', 'error');
    }
}

async function checkContractStatus() {
    try {
        // Check if contract is initialized by calling get_admin
        const adminResult = await readContract('get_admin');
        isInitialized = adminResult !== null;

        // Get total liquidity
        const totalLiquidity = await readContract('total_liquidity');
        const liquidityAmount = totalLiquidity ? StellarSdk.scValToNative(totalLiquidity) : 0;

        // Update UI
        const totalLiquidityEls = document.querySelectorAll('#total-liquidity, #admin-total-liquidity');
        totalLiquidityEls.forEach(el => {
            if (el) el.textContent = `$${formatAmount(liquidityAmount)}`;
        });

        const initializedEl = document.getElementById('contract-initialized');
        if (initializedEl) {
            initializedEl.textContent = isInitialized ? 'Yes' : 'No';
        }

        // Hide initialize section if already initialized
        const initSection = document.getElementById('initialize-section');
        if (initSection && isInitialized) {
            initSection.style.display = 'none';
        }

        return isInitialized;
    } catch (error) {
        console.error('Error checking contract status:', error);
        return false;
    }
}

async function depositFunds(amount) {
    try {
        showStatus('Depositing funds...', 'info');
        const userAddress = StellarSdk.Address.fromString(keypair.publicKey());
        const amountMicros = parseAmount(amount);

        const account = await rpc.getAccount(keypair.publicKey());
        let tx = new StellarSdk.TransactionBuilder(account, {
            fee: StellarSdk.BASE_FEE,
            networkPassphrase: CONFIG.networkPassphrase
        })
        .addOperation(contract.call(
            'deposit',
            StellarSdk.nativeToScVal(userAddress.toString(), { type: 'address' }),
            StellarSdk.nativeToScVal(amountMicros, { type: 'i128' })
        ))
        .setTimeout(30)
        .build();

        tx = await rpc.prepareTransaction(tx);
        tx.sign(keypair);
        const result = await rpc.sendTransaction(tx);

        if (result.status === 'ERROR') {
            throw new Error(result.errorResultXdr || 'Transaction failed');
        }

        let getResponse = await rpc.getTransaction(result.hash);
        while (getResponse.status === 'NOT_FOUND') {
            await new Promise(resolve => setTimeout(resolve, 1000));
            getResponse = await rpc.getTransaction(result.hash);
        }

        if (getResponse.status === 'SUCCESS') {
            showStatus(`Deposited $${amount} successfully!`, 'success');
            await loadUserBalance();
        } else {
            throw new Error('Transaction failed');
        }
    } catch (error) {
        console.error('Error depositing funds:', error);
        showStatus('Failed to deposit funds', 'error');
    }
}

async function withdrawFunds(amount) {
    try {
        showStatus('Withdrawing funds...', 'info');
        const userAddress = StellarSdk.Address.fromString(keypair.publicKey());
        const amountMicros = parseAmount(amount);

        const account = await rpc.getAccount(keypair.publicKey());
        let tx = new StellarSdk.TransactionBuilder(account, {
            fee: StellarSdk.BASE_FEE,
            networkPassphrase: CONFIG.networkPassphrase
        })
        .addOperation(contract.call(
            'withdraw',
            StellarSdk.nativeToScVal(userAddress.toString(), { type: 'address' }),
            StellarSdk.nativeToScVal(amountMicros, { type: 'i128' })
        ))
        .setTimeout(30)
        .build();

        tx = await rpc.prepareTransaction(tx);
        tx.sign(keypair);
        const result = await rpc.sendTransaction(tx);

        if (result.status === 'ERROR') {
            throw new Error(result.errorResultXdr || 'Transaction failed');
        }

        let getResponse = await rpc.getTransaction(result.hash);
        while (getResponse.status === 'NOT_FOUND') {
            await new Promise(resolve => setTimeout(resolve, 1000));
            getResponse = await rpc.getTransaction(result.hash);
        }

        if (getResponse.status === 'SUCCESS') {
            showStatus(`Withdrew $${amount} successfully!`, 'success');
            await loadUserBalance();
        } else {
            throw new Error('Transaction failed');
        }
    } catch (error) {
        console.error('Error withdrawing funds:', error);
        showStatus('Failed to withdraw funds', 'error');
    }
}

async function provideLiquidity(amount) {
    try {
        showStatus('Providing liquidity...', 'info');
        const providerAddress = StellarSdk.Address.fromString(keypair.publicKey());
        const amountMicros = parseAmount(amount);

        const account = await rpc.getAccount(keypair.publicKey());
        let tx = new StellarSdk.TransactionBuilder(account, {
            fee: StellarSdk.BASE_FEE,
            networkPassphrase: CONFIG.networkPassphrase
        })
        .addOperation(contract.call(
            'provide_liquidity',
            StellarSdk.nativeToScVal(providerAddress.toString(), { type: 'address' }),
            StellarSdk.nativeToScVal(amountMicros, { type: 'i128' })
        ))
        .setTimeout(30)
        .build();

        tx = await rpc.prepareTransaction(tx);
        tx.sign(keypair);
        const result = await rpc.sendTransaction(tx);

        if (result.status === 'ERROR') {
            throw new Error(result.errorResultXdr || 'Transaction failed');
        }

        let getResponse = await rpc.getTransaction(result.hash);
        while (getResponse.status === 'NOT_FOUND') {
            await new Promise(resolve => setTimeout(resolve, 1000));
            getResponse = await rpc.getTransaction(result.hash);
        }

        if (getResponse.status === 'SUCCESS') {
            showStatus(`Provided $${amount} liquidity successfully!`, 'success');
            await checkContractStatus();
        } else {
            throw new Error('Transaction failed');
        }
    } catch (error) {
        console.error('Error providing liquidity:', error);
        showStatus('Failed to provide liquidity', 'error');
    }
}

async function createMarket(title, startTime, homeOdds, drawOdds, awayOdds) {
    try {
        showStatus('Creating market...', 'info');

        // Sanitize title for Symbol type (max 32 chars, alphanumeric + underscore)
        const sanitizedTitle = title.split(' ').join('_').replace(/[^a-zA-Z0-9_]/g, '').slice(0, 32);

        const adminAddress = StellarSdk.Address.fromString(keypair.publicKey());
        const startTimestamp = Math.floor(new Date(startTime).getTime() / 1000);
        const homeOddsMicros = parseAmount(homeOdds);
        const drawOddsMicros = parseAmount(drawOdds);
        const awayOddsMicros = parseAmount(awayOdds);

        // Build transaction manually with correct types
        const account = await rpc.getAccount(keypair.publicKey());

        let tx = new StellarSdk.TransactionBuilder(account, {
            fee: StellarSdk.BASE_FEE,
            networkPassphrase: CONFIG.networkPassphrase
        })
        .addOperation(contract.call(
            'create_market',
            StellarSdk.nativeToScVal(adminAddress.toString(), { type: 'address' }),
            StellarSdk.nativeToScVal(sanitizedTitle, { type: 'symbol' }),
            StellarSdk.nativeToScVal(startTimestamp, { type: 'i64' }),
            StellarSdk.nativeToScVal(homeOddsMicros, { type: 'i128' }),
            StellarSdk.nativeToScVal(drawOddsMicros, { type: 'i128' }),
            StellarSdk.nativeToScVal(awayOddsMicros, { type: 'i128' })
        ))
        .setTimeout(30)
        .build();

        // Prepare and simulate
        tx = await rpc.prepareTransaction(tx);

        // Sign transaction
        tx.sign(keypair);

        // Send transaction
        const result = await rpc.sendTransaction(tx);

        if (result.status === 'ERROR') {
            throw new Error(result.errorResultXdr || 'Transaction failed');
        }

        // Wait for confirmation
        let getResponse = await rpc.getTransaction(result.hash);
        while (getResponse.status === 'NOT_FOUND') {
            await new Promise(resolve => setTimeout(resolve, 1000));
            getResponse = await rpc.getTransaction(result.hash);
        }

        if (getResponse.status === 'SUCCESS') {
            showStatus('Market created successfully!', 'success');
            await loadMarkets();
            await loadAdminMarkets();
        } else {
            throw new Error('Transaction failed');
        }
    } catch (error) {
        console.error('Error creating market:', error);
        showStatus('Failed to create market', 'error');
    }
}

async function placeStake(marketId, outcome, amount) {
    try {
        showStatus('Placing stake...', 'info');
        const userAddress = StellarSdk.Address.fromString(keypair.publicKey());
        const amountMicros = parseAmount(amount);

        const account = await rpc.getAccount(keypair.publicKey());
        let tx = new StellarSdk.TransactionBuilder(account, {
            fee: StellarSdk.BASE_FEE,
            networkPassphrase: CONFIG.networkPassphrase
        })
        .addOperation(contract.call(
            'place_stake',
            StellarSdk.nativeToScVal(userAddress.toString(), { type: 'address' }),
            StellarSdk.nativeToScVal(marketId, { type: 'u64' }),
            StellarSdk.nativeToScVal(outcome, { type: 'u32' }),
            StellarSdk.nativeToScVal(amountMicros, { type: 'i128' })
        ))
        .setTimeout(30)
        .build();

        tx = await rpc.prepareTransaction(tx);
        tx.sign(keypair);
        const result = await rpc.sendTransaction(tx);

        if (result.status === 'ERROR') {
            throw new Error(result.errorResultXdr || 'Transaction failed');
        }

        let getResponse = await rpc.getTransaction(result.hash);
        while (getResponse.status === 'NOT_FOUND') {
            await new Promise(resolve => setTimeout(resolve, 1000));
            getResponse = await rpc.getTransaction(result.hash);
        }

        if (getResponse.status === 'SUCCESS') {
            showStatus('Stake placed successfully!', 'success');
            await loadUserBalance();
            await loadUserStakes();
            await loadMarkets();
            closeStakingModal();
        } else {
            throw new Error('Transaction failed');
        }
    } catch (error) {
        console.error('Error placing stake:', error);
        showStatus('Failed to place stake', 'error');
    }
}

async function settleMarket(marketId, outcome) {
    try {
        showStatus('Settling market...', 'info');
        const adminAddress = StellarSdk.Address.fromString(keypair.publicKey());

        const account = await rpc.getAccount(keypair.publicKey());
        let tx = new StellarSdk.TransactionBuilder(account, {
            fee: StellarSdk.BASE_FEE,
            networkPassphrase: CONFIG.networkPassphrase
        })
        .addOperation(contract.call(
            'settle_market',
            StellarSdk.nativeToScVal(adminAddress.toString(), { type: 'address' }),
            StellarSdk.nativeToScVal(marketId, { type: 'u64' }),
            StellarSdk.nativeToScVal(outcome, { type: 'u32' })
        ))
        .setTimeout(30)
        .build();

        tx = await rpc.prepareTransaction(tx);
        tx.sign(keypair);
        const result = await rpc.sendTransaction(tx);

        if (result.status === 'ERROR') {
            throw new Error(result.errorResultXdr || 'Transaction failed');
        }

        let getResponse = await rpc.getTransaction(result.hash);
        while (getResponse.status === 'NOT_FOUND') {
            await new Promise(resolve => setTimeout(resolve, 1000));
            getResponse = await rpc.getTransaction(result.hash);
        }

        if (getResponse.status === 'SUCCESS') {
            showStatus('Market settled successfully!', 'success');
            await loadMarkets();
            await loadAdminMarkets();
        } else {
            throw new Error('Transaction failed');
        }
    } catch (error) {
        console.error('Error settling market:', error);
        showStatus('Failed to settle market', 'error');
    }
}

async function archiveMarket(marketId) {
    try {
        showStatus('Archiving market...', 'info');
        const adminAddress = StellarSdk.Address.fromString(keypair.publicKey());

        const account = await rpc.getAccount(keypair.publicKey());
        let tx = new StellarSdk.TransactionBuilder(account, {
            fee: StellarSdk.BASE_FEE,
            networkPassphrase: CONFIG.networkPassphrase
        })
        .addOperation(contract.call(
            'archive_market',
            StellarSdk.nativeToScVal(adminAddress.toString(), { type: 'address' }),
            StellarSdk.nativeToScVal(marketId, { type: 'u64' })
        ))
        .setTimeout(30)
        .build();

        tx = await rpc.prepareTransaction(tx);
        tx.sign(keypair);
        const result = await rpc.sendTransaction(tx);

        if (result.status === 'ERROR') {
            throw new Error(result.errorResultXdr || 'Transaction failed');
        }

        let getResponse = await rpc.getTransaction(result.hash);
        while (getResponse.status === 'NOT_FOUND') {
            await new Promise(resolve => setTimeout(resolve, 1000));
            getResponse = await rpc.getTransaction(result.hash);
        }

        if (getResponse.status === 'SUCCESS') {
            showStatus('Market archived successfully!', 'success');
            await loadAdminMarkets();
        } else {
            throw new Error('Transaction failed');
        }
    } catch (error) {
        console.error('Error archiving market:', error);
        showStatus('Failed to archive market', 'error');
    }
}

// Data loading functions
async function loadUserBalance() {
    if (!keypair) return;

    try {
        const userAddress = StellarSdk.Address.fromString(keypair.publicKey());
        const balance = await readContract('get_balance', userAddress);
        const balanceAmount = balance ? StellarSdk.scValToNative(balance) : 0;

        const balanceEls = document.querySelectorAll('#user-balance, #balance-display');
        balanceEls.forEach(el => {
            if (el) el.textContent = `$${formatAmount(balanceAmount)}`;
        });
    } catch (error) {
        console.error('Error loading user balance:', error);
    }
}

async function loadMarkets() {
    try {
        const container = document.getElementById('markets-container');
        const noMarkets = document.getElementById('no-markets');

        if (!container) return;

        const markets = [];

        // Try to load markets sequentially until we hit one that doesn't exist
        // Markets are created with incrementing IDs starting from 1
        for (let i = 1; i <= 100; i++) {
            const market = await readContract('get_market', BigInt(i));

            // If readContract returns null, the market doesn't exist - stop searching
            if (!market) {
                break;
            }

            const marketData = StellarSdk.scValToNative(market);
            // MarketStatus enum is returned as array ['Active'], ['Settled'], or ['Archived']
            const status = Array.isArray(marketData.status) ? marketData.status[0] : marketData.status;
            if (status === 'Active') {
                // Get current odds from CPMM
                const currentOdds = await readContract('get_current_odds', BigInt(i));
                if (currentOdds) {
                    const oddsNative = StellarSdk.scValToNative(currentOdds);
                    marketData.current_odds_home = oddsNative[0];
                    marketData.current_odds_draw = oddsNative[1];
                    marketData.current_odds_away = oddsNative[2];
                } else {
                    // Fallback to static odds
                    marketData.current_odds_home = marketData.odds_home;
                    marketData.current_odds_draw = marketData.odds_draw;
                    marketData.current_odds_away = marketData.odds_away;
                }
                markets.push({ id: i, ...marketData, status });
            }
        }

        if (markets.length === 0) {
            container.style.display = 'none';
            if (noMarkets) noMarkets.style.display = 'block';
        } else {
            container.style.display = 'grid';
            if (noMarkets) noMarkets.style.display = 'none';

            container.innerHTML = markets.map(market => `
                <div class="market-card" data-market-id="${market.id}">
                    <div class="market-header">
                        <h3 class="market-title">${market.title}</h3>
                        <div class="market-time">${formatDateTime(market.start_time)}</div>
                        <span class="market-status ${market.status.toLowerCase()}">${market.status}</span>
                    </div>
                    <div class="outcomes">
                        <div class="outcome" data-outcome="0">
                            <span class="outcome-name">Home Win</span>
                            <span class="outcome-odds" data-market-id="${market.id}" data-outcome="0">$${formatAmount(market.current_odds_home)}</span>
                        </div>
                        <div class="outcome" data-outcome="1">
                            <span class="outcome-name">Draw</span>
                            <span class="outcome-odds" data-market-id="${market.id}" data-outcome="1">$${formatAmount(market.current_odds_draw)}</span>
                        </div>
                        <div class="outcome" data-outcome="2">
                            <span class="outcome-name">Away Win</span>
                            <span class="outcome-odds" data-market-id="${market.id}" data-outcome="2">$${formatAmount(market.current_odds_away)}</span>
                        </div>
                    </div>
                </div>
            `).join('');

            // Add click handlers for staking
            container.querySelectorAll('.outcome').forEach(outcome => {
                outcome.addEventListener('click', (e) => {
                    if (!keypair) {
                        showStatus('Please connect your wallet first', 'warning');
                        return;
                    }

                    const marketCard = e.target.closest('.market-card');
                    const marketId = marketCard.dataset.marketId;
                    const outcomeId = outcome.dataset.outcome;
                    const market = markets.find(m => m.id == marketId);

                    openStakingModal(market, outcomeId);
                });
            });
        }

        // Update active markets count
        const activeMarketsEl = document.getElementById('active-markets');
        if (activeMarketsEl) {
            activeMarketsEl.textContent = markets.length;
        }

        // Start real-time odds updates
        startOddsUpdates();
    } catch (error) {
        console.error('Error loading markets:', error);
    }
}

async function updateLiveOdds() {
    try {
        const oddsElements = document.querySelectorAll('.outcome-odds[data-market-id]');

        // Group by market ID to avoid duplicate calls
        const marketIds = new Set();
        oddsElements.forEach(el => marketIds.add(el.dataset.marketId));

        for (const marketId of marketIds) {
            const currentOdds = await readContract('get_current_odds', BigInt(marketId));
            if (!currentOdds) continue;

            const oddsNative = StellarSdk.scValToNative(currentOdds);
            const [oddsHome, oddsDraw, oddsAway] = oddsNative;

            // Update each outcome's odds
            oddsElements.forEach(el => {
                if (el.dataset.marketId !== marketId) return;

                const outcome = parseInt(el.dataset.outcome);
                let newOdds;
                if (outcome === 0) newOdds = oddsHome;
                else if (outcome === 1) newOdds = oddsDraw;
                else if (outcome === 2) newOdds = oddsAway;

                const oldOdds = parseFloat(el.textContent.replace('$', ''));
                const newOddsFormatted = formatAmount(newOdds);

                if (Math.abs(oldOdds - parseFloat(newOddsFormatted)) > 0.001) {
                    // Odds changed - update with animation
                    el.textContent = `$${newOddsFormatted}`;
                    el.classList.add('odds-changed');
                    setTimeout(() => el.classList.remove('odds-changed'), 500);
                }
            });
        }
    } catch (error) {
        console.error('Error updating live odds:', error);
    }
}

function startOddsUpdates() {
    // Clear existing interval if any
    if (oddsUpdateInterval) {
        clearInterval(oddsUpdateInterval);
    }

    // Update odds every 5 seconds
    oddsUpdateInterval = setInterval(updateLiveOdds, 5000);
}

function stopOddsUpdates() {
    if (oddsUpdateInterval) {
        clearInterval(oddsUpdateInterval);
        oddsUpdateInterval = null;
    }
}

async function updateLiveStakeValues() {
    try {
        // Get all stake cards with value elements
        const valueElements = document.querySelectorAll('[data-stake-id$="-value"]');
        if (valueElements.length === 0) return;

        // Extract unique stake IDs and fetch their data
        const stakeIds = new Set();
        valueElements.forEach(el => {
            const match = el.dataset.stakeId.match(/^(\d+)-value$/);
            if (match) stakeIds.add(match[1]);
        });

        for (const stakeId of stakeIds) {
            try {
                // Get stake data
                const stakeResult = await readContract('get_stake', BigInt(stakeId));
                if (!stakeResult) continue;

                const stake = StellarSdk.scValToNative(stakeResult);

                // Get current market data
                const market = await readContract('get_market', BigInt(stake.market_id));
                if (!market) continue;

                const marketData = StellarSdk.scValToNative(market);
                const status = Array.isArray(marketData.status) ? marketData.status[0] : marketData.status;

                // Only update active markets
                if (status !== 'Active') continue;

                // Get current odds
                const currentOdds = await readContract('get_current_odds', BigInt(stake.market_id));
                if (!currentOdds) continue;

                const oddsNative = StellarSdk.scValToNative(currentOdds);
                const currentPrice = oddsNative[stake.outcome];

                // Update current odds display
                const oddsElement = document.querySelector(`[data-stake-id="${stakeId}-current-odds"]`);
                if (oddsElement) {
                    const newOddsText = `$${formatAmount(currentPrice)}`;
                    if (oddsElement.textContent !== newOddsText) {
                        oddsElement.textContent = newOddsText;
                        oddsElement.classList.add('odds-changed');
                        setTimeout(() => oddsElement.classList.remove('odds-changed'), 500);
                    }
                }

                // Get total liquidity
                const totalLiq = await readContract('total_liquidity');
                const totalLiquidity = totalLiq ? Number(StellarSdk.scValToNative(totalLiq)) : 0;

                // Get the reserve (total shares) for this outcome
                const reserve = stake.outcome === 0 ? marketData.reserve_home :
                               stake.outcome === 1 ? marketData.reserve_draw :
                               marketData.reserve_away;

                // Calculate current value: shares * (total_liq / reserve)
                // Capped at shares to prevent showing profit from own liquidity addition
                const shares = Number(stake.amount);
                const totalShares = Number(reserve);
                const proportionalValue = totalShares > 0 ? (shares * totalLiquidity) / totalShares : 0;
                const currentValue = proportionalValue < shares ? proportionalValue : shares;

                // Update current value display
                const valueElement = document.querySelector(`[data-stake-id="${stakeId}-value"]`);
                if (valueElement) {
                    const newValueText = `$${(currentValue / CONFIG.decimals).toFixed(2)}`;
                    if (valueElement.textContent !== newValueText) {
                        valueElement.textContent = newValueText;
                        valueElement.classList.add('odds-changed');
                        setTimeout(() => valueElement.classList.remove('odds-changed'), 500);
                    }

                    // Update profit/loss indicator
                    const entryValue = Number(stake.amount) * Number(stake.price) / CONFIG.decimals;
                    const displayValue = currentValue / CONFIG.decimals;

                    if (displayValue > entryValue) {
                        valueElement.classList.add('profit');
                        valueElement.classList.remove('loss');
                    } else if (displayValue < entryValue) {
                        valueElement.classList.add('loss');
                        valueElement.classList.remove('profit');
                    }
                }
            } catch (error) {
                // Skip individual stake errors
                continue;
            }
        }
    } catch (error) {
        console.error('Error updating live stake values:', error);
    }
}

function startStakeValueUpdates() {
    // Clear existing interval if any
    if (stakeValueUpdateInterval) {
        clearInterval(stakeValueUpdateInterval);
    }

    // Update stake values every 5 seconds
    stakeValueUpdateInterval = setInterval(updateLiveStakeValues, 5000);
}

function stopStakeValueUpdates() {
    if (stakeValueUpdateInterval) {
        clearInterval(stakeValueUpdateInterval);
        stakeValueUpdateInterval = null;
    }
}

async function loadAdminMarkets() {
    if (!window.isAdminMode) return;

    try {
        const container = document.getElementById('admin-markets-container');
        const noMarkets = document.getElementById('admin-no-markets');

        if (!container) return;

        const markets = [];

        // Try to load markets sequentially until we hit one that doesn't exist
        // Load all markets (including settled ones) for admin view
        for (let i = 1; i <= 100; i++) {
            const market = await readContract('get_market', BigInt(i));

            // If readContract returns null, the market doesn't exist - stop searching
            if (!market) {
                break;
            }

            const marketData = StellarSdk.scValToNative(market);
            // MarketStatus enum is returned as array ['Active'], ['Settled'], or ['Archived']
            const status = Array.isArray(marketData.status) ? marketData.status[0] : marketData.status;
            markets.push({ id: i, ...marketData, status });
        }

        if (markets.length === 0) {
            container.style.display = 'none';
            if (noMarkets) noMarkets.style.display = 'block';
        } else {
            container.style.display = 'block';
            if (noMarkets) noMarkets.style.display = 'none';

            container.innerHTML = markets.map(market => `
                <div class="admin-market-item">
                    <div class="admin-market-info">
                        <h4>Market ${market.id}: ${market.title}</h4>
                        <div class="admin-market-meta">
                            Status: ${market.status} | Start: ${formatDateTime(market.start_time)} | Stakers: ${market.staker_count}
                        </div>
                    </div>
                    <div class="admin-market-actions">
                        ${market.status === 'Active' ? `
                            <button class="btn btn-warning btn-sm" onclick="settleMarketPrompt(${market.id})">Settle</button>
                        ` : ''}
                        <button class="btn btn-danger btn-sm" onclick="archiveMarketPrompt(${market.id})">Archive</button>
                    </div>
                </div>
            `).join('');
        }
    } catch (error) {
        console.error('Error loading admin markets:', error);
    }
}

async function loadUserStakes() {
    if (!keypair) return;

    try {
        const container = document.getElementById('my-stakes-container');
        if (!container) return;

        const userAddress = keypair.publicKey();
        const userStakes = [];
        const marketCache = {};

        // Iterate through markets to find user's stakes
        for (let marketId = 1; marketId <= 100; marketId++) {
            try {
                const market = await readContract('get_market', BigInt(marketId));
                if (!market) break;

                const marketData = StellarSdk.scValToNative(market);
                const status = Array.isArray(marketData.status) ? marketData.status[0] : marketData.status;
                marketCache[marketId] = { ...marketData, status, id: marketId };

                // Get all stakes for this market
                const stakesResult = await readContract('get_market_stakes', BigInt(marketId));
                if (!stakesResult) continue;

                const stakes = StellarSdk.scValToNative(stakesResult);

                // Filter stakes for current user
                for (const stake of stakes) {
                    const stakeAddress = typeof stake.staker === 'string' ? stake.staker : stake.staker.toString();
                    if (stakeAddress === userAddress) {
                        userStakes.push({
                            ...stake,
                            market: marketCache[marketId]
                        });
                    }
                }
            } catch (error) {
                // Market doesn't exist or error reading, continue
                continue;
            }
        }

        if (userStakes.length === 0) {
            container.innerHTML = '<div class="no-markets"><div class="no-markets-content"><div class="no-markets-icon">🎫</div><h3>No Stakes Yet</h3><p>Your staking history will appear here after placing stakes</p></div></div>';
        } else {
            const outcomeNames = ['Home Win', 'Draw', 'Away Win'];
            container.innerHTML = userStakes.map(stake => `
                <div class="stake-card" data-stake-id="${stake.id}">
                    <div class="stake-header">
                        <h4 class="stake-title">${stake.market.title}</h4>
                        <span class="market-status ${stake.market.status.toLowerCase()}">${stake.market.status}</span>
                    </div>
                    <div class="stake-details-grid">
                        <div class="stake-detail">
                            <span class="stake-label">Outcome</span>
                            <span class="stake-value">${outcomeNames[stake.outcome]}</span>
                        </div>
                        <div class="stake-detail">
                            <span class="stake-label">Entry Odds</span>
                            <span class="stake-value">$${formatAmount(stake.price)}</span>
                        </div>
                        <div class="stake-detail">
                            <span class="stake-label">Current Odds</span>
                            <span class="stake-value" data-stake-id="${stake.id}-current-odds">Calculating...</span>
                        </div>
                        <div class="stake-detail">
                            <span class="stake-label">Settlement Value</span>
                            <span class="stake-value" data-stake-id="${stake.id}-settlement">Calculating...</span>
                        </div>
                        <div class="stake-detail">
                            <span class="stake-label">Cashout Value</span>
                            <span class="stake-value profit" data-stake-id="${stake.id}-cashout">Calculating...</span>
                        </div>
                    </div>
                    ${stake.market.status === 'Active' ? `
                        <button class="btn btn-warning cash-out-btn" onclick="cashOutStake(${stake.id})" style="margin-top: 10px; width: 100%;">
                            Cash Out (5% fee)
                        </button>
                    ` : ''}
                </div>
            `).join('');

            // Calculate and update current values
            userStakes.forEach(async stake => {
                if (stake.market.status === 'Active') {
                    await updateStakeCurrentValue(stake);
                }
            });

            // Start real-time stake value updates
            startStakeValueUpdates();
        }
    } catch (error) {
        console.error('Error loading user stakes:', error);
    }
}

async function updateStakeCurrentValue(stake) {
    try {
        // Get current market data
        const market = await readContract('get_market', BigInt(stake.market_id));
        if (!market) return;

        const marketData = StellarSdk.scValToNative(market);

        // Get current odds for the market
        const currentOdds = await readContract('get_current_odds', BigInt(stake.market_id));
        if (!currentOdds) return;

        const oddsNative = StellarSdk.scValToNative(currentOdds);
        const currentPrice = oddsNative[stake.outcome];

        // Update current odds display
        const oddsElement = document.querySelector(`[data-stake-id="${stake.id}-current-odds"]`);
        if (oddsElement) {
            oddsElement.textContent = `$${formatAmount(currentPrice)}`;
        }

        // Get the reserve (USD amount staked on this outcome)
        const reserve = stake.outcome === 0 ? marketData.reserve_home :
                       stake.outcome === 1 ? marketData.reserve_draw :
                       marketData.reserve_away;

        // Calculate current value with exit slippage (mirrors contract logic)
        const shares = Number(stake.amount);
        const reserveNum = Number(reserve);
        const totalReserve = Number(marketData.reserve_home) + Number(marketData.reserve_draw) + Number(marketData.reserve_away);

        // Price before exit
        const priceBeforeExit = totalReserve > 0 ? (reserveNum * CONFIG.decimals) / totalReserve : 0;

        // Estimated payout
        const estimatedPayout = (shares * priceBeforeExit) / CONFIG.decimals;

        // Price after removing estimated payout
        const priceAfterExit = reserveNum <= estimatedPayout ? 0 :
            ((reserveNum - estimatedPayout) * CONFIG.decimals) / (totalReserve - estimatedPayout);

        // Average exit price (with slippage)
        const avgExitPrice = (priceBeforeExit + priceAfterExit) / 2;

        // Settlement value: fixed $1 per share (what you get if you win)
        const settlementValue = shares;

        // Cashout value: current market value (before 5% fee)
        const cashoutValueBeforeFee = (shares * avgExitPrice) / CONFIG.decimals;
        const cashoutFee = cashoutValueBeforeFee * 0.05;
        const cashoutValueAfterFee = cashoutValueBeforeFee - cashoutFee;

        // Update Settlement Value
        const settlementElement = document.querySelector(`[data-stake-id="${stake.id}-settlement"]`);
        if (settlementElement) {
            settlementElement.textContent = `$${(settlementValue / CONFIG.decimals).toFixed(2)}`;
        }

        // Update Cashout Value
        const cashoutElement = document.querySelector(`[data-stake-id="${stake.id}-cashout"]`);
        if (cashoutElement) {
            cashoutElement.textContent = `$${(cashoutValueAfterFee / CONFIG.decimals).toFixed(2)}`;

            // Calculate entry value for comparison (amount originally paid)
            const entryValue = (shares * Number(stake.price)) / CONFIG.decimals;
            const displayValue = cashoutValueAfterFee / CONFIG.decimals;

            if (displayValue > entryValue) {
                cashoutElement.classList.add('profit');
                cashoutElement.classList.remove('loss');
            } else if (displayValue < entryValue) {
                cashoutElement.classList.add('loss');
                cashoutElement.classList.remove('profit');
            }
        }
    } catch (error) {
        console.error('Error updating stake value:', error);
    }
}

async function cashOutStake(stakeId) {
    if (!keypair) {
        showStatus('Please connect your wallet first', 'warning');
        return;
    }

    try {
        showStatus('Processing cash out...', 'info');

        const userAddress = StellarSdk.Address.fromString(keypair.publicKey());
        await callContract('cash_out', userAddress, BigInt(stakeId));

        showStatus('Successfully cashed out!', 'success');

        // Reload user data
        await Promise.all([
            loadUserBalance(),
            loadUserStakes()
        ]);
    } catch (error) {
        console.error('Error cashing out:', error);
        showStatus('Failed to cash out stake', 'error');
    }
}

// UI Functions
let currentStake = null;

async function openStakingModal(market, outcomeId) {
    const modal = document.getElementById('staking-modal');
    const modalTitle = document.getElementById('modal-title');
    const modalMatchTitle = document.getElementById('modal-match-title');
    const modalMatchTime = document.getElementById('modal-match-time');
    const selectedOutcome = document.getElementById('selected-outcome');
    const selectedOdds = document.getElementById('selected-odds');

    const outcomes = ['Home Win', 'Draw', 'Away Win'];
    const odds = [market.odds_home, market.odds_draw, market.odds_away];

    // Get current CPMM odds for the market
    let currentMarketOdds = null;
    try {
        const currentOddsResult = await readContract('get_current_odds', BigInt(market.id));
        if (currentOddsResult) {
            const oddsNative = StellarSdk.scValToNative(currentOddsResult);
            currentMarketOdds = oddsNative;
        }
    } catch (error) {
        console.error('Error fetching current odds:', error);
    }

    currentStake = {
        marketId: market.id,
        outcome: parseInt(outcomeId),
        odds: odds[outcomeId],
        currentOdds: currentMarketOdds,
        reserves: {
            home: market.reserve_home || 0,
            draw: market.reserve_draw || 0,
            away: market.reserve_away || 0
        }
    };

    modalTitle.textContent = 'Place Stake';
    modalMatchTitle.textContent = market.title;
    modalMatchTime.textContent = formatDateTime(market.start_time);
    selectedOutcome.textContent = outcomes[outcomeId];
    selectedOdds.textContent = `$${formatAmount(odds[outcomeId])}`;

    modal.classList.add('show');

    // Reset form
    document.getElementById('stake-amount').value = '';
    await updatePayout();
}

function closeStakingModal() {
    const modal = document.getElementById('staking-modal');
    modal.classList.remove('show');
    currentStake = null;
}

async function updatePayout() {
    const stakeAmount = parseFloat(document.getElementById('stake-amount').value) || 0;
    const currentPriceEl = document.getElementById('current-price');
    const quotePriceEl = document.getElementById('quote-price');
    const slippagePercentEl = document.getElementById('slippage-percent');
    const potentialPayout = document.getElementById('potential-payout');
    const potentialProfit = document.getElementById('potential-profit');

    if (currentStake && stakeAmount > 0) {
        // Get current market data to calculate CPMM price
        let currentPrice = 0;
        let quotePrice = 0;
        let slippagePercent = 0;

        try {
            // Fetch fresh market data
            const market = await readContract('get_market', BigInt(currentStake.marketId));
            if (market) {
                const marketData = StellarSdk.scValToNative(market);

                // Get reserves
                const reserveHome = Number(marketData.reserve_home) || 0;
                const reserveDraw = Number(marketData.reserve_draw) || 0;
                const reserveAway = Number(marketData.reserve_away) || 0;
                const totalReserve = reserveHome + reserveDraw + reserveAway;

                // Get current reserve for selected outcome
                let currentReserve = 0;
                if (currentStake.outcome === 0) currentReserve = reserveHome;
                else if (currentStake.outcome === 1) currentReserve = reserveDraw;
                else if (currentStake.outcome === 2) currentReserve = reserveAway;

                // Calculate current price (before stake)
                if (totalReserve > 0 && currentReserve > 0) {
                    currentPrice = (currentReserve * CONFIG.decimals) / totalReserve;
                } else {
                    // Fallback to initial odds
                    const oddsNum = typeof currentStake.odds === 'bigint' ? Number(currentStake.odds) : currentStake.odds;
                    currentPrice = oddsNum;
                }

                // Calculate quote price (middle price with slippage)
                const stakeAmountMicros = stakeAmount * CONFIG.decimals;
                const newReserve = currentReserve + stakeAmountMicros;
                const newTotal = totalReserve + stakeAmountMicros;

                // Price after stake
                const priceAfter = newTotal > 0 ? (newReserve * CONFIG.decimals) / newTotal : currentPrice;

                // Quote price is the average of before and after (what user actually gets)
                quotePrice = (currentPrice + priceAfter) / 2;

                // Calculate slippage percentage
                if (currentPrice > 0) {
                    slippagePercent = ((priceAfter - currentPrice) / currentPrice) * 100;
                }
            } else {
                // Fallback to static odds if market data unavailable
                const oddsNum = typeof currentStake.odds === 'bigint' ? Number(currentStake.odds) : currentStake.odds;
                currentPrice = oddsNum;
                quotePrice = oddsNum;
            }
        } catch (error) {
            console.error('Error calculating quote price:', error);
            // Fallback to static odds
            const oddsNum = typeof currentStake.odds === 'bigint' ? Number(currentStake.odds) : currentStake.odds;
            currentPrice = oddsNum;
            quotePrice = oddsNum;
        }

        // Calculate payout and profit using quote price
        const payout = stakeAmount * CONFIG.decimals / quotePrice;
        const profit = payout - stakeAmount;

        // Update UI
        currentPriceEl.textContent = `$${(currentPrice / CONFIG.decimals).toFixed(4)}`;
        quotePriceEl.textContent = `$${(quotePrice / CONFIG.decimals).toFixed(4)}`;
        slippagePercentEl.textContent = `${slippagePercent.toFixed(2)}%`;
        potentialPayout.textContent = `$${payout.toFixed(2)}`;
        potentialProfit.textContent = `$${profit.toFixed(2)}`;
    } else {
        // Reset all values
        currentPriceEl.textContent = '$0.00';
        quotePriceEl.textContent = '$0.00';
        slippagePercentEl.textContent = '0.00%';
        potentialPayout.textContent = '$0.00';
        potentialProfit.textContent = '$0.00';
    }
}

// Admin helper functions
function settleMarketPrompt(marketId) {
    const outcome = prompt('Enter winning outcome (0=Home Win, 1=Draw, 2=Away Win):');
    if (outcome !== null && ['0', '1', '2'].includes(outcome)) {
        settleMarket(marketId, parseInt(outcome));
    }
}

function archiveMarketPrompt(marketId) {
    if (confirm('Are you sure you want to archive this market?')) {
        archiveMarket(marketId);
    }
}

function validateOdds() {
    const homeOdds = parseFloat(document.getElementById('home-odds').value) || 0;
    const drawOdds = parseFloat(document.getElementById('draw-odds').value) || 0;
    const awayOdds = parseFloat(document.getElementById('away-odds').value) || 0;

    const sum = homeOdds + drawOdds + awayOdds;
    const oddsSum = document.getElementById('odds-sum');
    const validation = document.getElementById('odds-validation');

    if (oddsSum) oddsSum.textContent = sum.toFixed(2);

    if (validation) {
        if (Math.abs(sum - 0.99) < 0.01) {
            validation.textContent = '✓ Valid';
            validation.className = 'validation-message valid';
        } else {
            validation.textContent = '✗ Must sum to $0.99';
            validation.className = 'validation-message invalid';
        }
    }

    return Math.abs(sum - 0.99) < 0.01;
}

// Admin initialization
async function initializeAdmin() {
    // Check for stored admin key and auto-connect
    const storedKey = getAdminKey();
    if (storedKey) {
        try {
            await connectAdminWallet(storedKey);
        } catch (error) {
            console.error('Failed to connect with stored key:', error);
            clearAdminKey();
        }
    }

    // Add odds validation listeners
    const oddsInputs = document.querySelectorAll('#home-odds, #draw-odds, #away-odds');
    oddsInputs.forEach(input => {
        input.addEventListener('input', validateOdds);
    });

    // Load admin data
    checkContractStatus();
    loadAdminMarkets();
}

// Initialize application
document.addEventListener('DOMContentLoaded', function() {
    initializeStellar();

    // Connect wallet handlers
    const connectBtn = document.getElementById('connect-wallet');
    const connectAdminBtn = document.getElementById('connect-admin-wallet');
    const clearAdminKeyBtn = document.getElementById('clear-admin-key');

    if (connectBtn) connectBtn.addEventListener('click', connectWallet);
    if (connectAdminBtn) connectAdminBtn.addEventListener('click', connectWallet);
    if (clearAdminKeyBtn) clearAdminKeyBtn.addEventListener('click', disconnectAdminWallet);

    // Admin key modal handlers
    const adminKeyModal = document.getElementById('admin-key-modal');
    const closeKeyModalBtn = document.getElementById('close-key-modal');
    const cancelKeyModalBtn = document.getElementById('cancel-key-modal');
    const connectWithKeyBtn = document.getElementById('connect-with-key');
    const adminSecretKeyInput = document.getElementById('admin-secret-key');

    if (closeKeyModalBtn) closeKeyModalBtn.addEventListener('click', hideAdminKeyModal);
    if (cancelKeyModalBtn) cancelKeyModalBtn.addEventListener('click', hideAdminKeyModal);
    if (adminKeyModal) {
        adminKeyModal.addEventListener('click', (e) => {
            if (e.target === adminKeyModal) {
                hideAdminKeyModal();
            }
        });
    }

    if (connectWithKeyBtn) {
        connectWithKeyBtn.addEventListener('click', async () => {
            const secretKey = adminSecretKeyInput.value.trim();
            if (!secretKey) {
                showStatus('Please enter your secret key', 'error');
                return;
            }

            try {
                await connectAdminWallet(secretKey);
                hideAdminKeyModal();
            } catch (error) {
                // Error already handled in connectAdminWallet
            }
        });
    }

    // Allow Enter key to connect in admin modal
    if (adminSecretKeyInput) {
        adminSecretKeyInput.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                connectWithKeyBtn.click();
            }
        });
    }

    // Deposit/Withdraw handlers
    const depositBtn = document.getElementById('deposit-btn');
    const withdrawBtn = document.getElementById('withdraw-btn');

    if (depositBtn) {
        depositBtn.addEventListener('click', () => {
            const amount = document.getElementById('deposit-amount').value;
            if (amount && parseFloat(amount) > 0) {
                depositFunds(amount);
                document.getElementById('deposit-amount').value = '';
            }
        });
    }

    if (withdrawBtn) {
        withdrawBtn.addEventListener('click', () => {
            const amount = document.getElementById('withdraw-amount').value;
            if (amount && parseFloat(amount) > 0) {
                withdrawFunds(amount);
                document.getElementById('withdraw-amount').value = '';
            }
        });
    }

    // Admin handlers
    const initializeBtn = document.getElementById('initialize-contract');
    const provideLiquidityBtn = document.getElementById('provide-liquidity');
    const createMarketBtn = document.getElementById('create-market');
    const settleMarketBtn = document.getElementById('settle-market');
    const archiveMarketBtn = document.getElementById('archive-market');
    const contractIdDisplay = document.getElementById('contract-id-display');

    if (contractIdDisplay) contractIdDisplay.innerText = CONFIG.contractId;

    if (initializeBtn) {
        initializeBtn.addEventListener('click', initializeContract);
    }

    if (provideLiquidityBtn) {
        provideLiquidityBtn.addEventListener('click', () => {
            const amount = document.getElementById('liquidity-amount').value;
            if (amount && parseFloat(amount) > 0) {
                provideLiquidity(amount);
                document.getElementById('liquidity-amount').value = '';
            }
        });
    }

    if (createMarketBtn) {
        createMarketBtn.addEventListener('click', () => {
            const title = document.getElementById('market-title').value;
            const startTime = document.getElementById('market-start-time').value;
            const homeOdds = document.getElementById('home-odds').value;
            const drawOdds = document.getElementById('draw-odds').value;
            const awayOdds = document.getElementById('away-odds').value;

            if (title && startTime && validateOdds()) {
                createMarket(title, startTime, homeOdds, drawOdds, awayOdds);

                // Reset form
                document.getElementById('market-title').value = '';
                document.getElementById('market-start-time').value = '';
                document.getElementById('home-odds').value = '';
                document.getElementById('draw-odds').value = '';
                document.getElementById('away-odds').value = '';
                validateOdds();
            } else {
                showStatus('Please fill all fields and ensure odds sum to $0.99', 'error');
            }
        });
    }

    if (settleMarketBtn) {
        settleMarketBtn.addEventListener('click', () => {
            const marketId = document.getElementById('settle-market-id').value;
            const outcome = document.getElementById('winning-outcome').value;

            if (marketId && outcome !== '') {
                settleMarket(parseInt(marketId), parseInt(outcome));
                document.getElementById('settle-market-id').value = '';
                document.getElementById('winning-outcome').value = '';
            }
        });
    }

    if (archiveMarketBtn) {
        archiveMarketBtn.addEventListener('click', () => {
            const marketId = document.getElementById('archive-market-id').value;

            if (marketId) {
                archiveMarket(parseInt(marketId));
                document.getElementById('archive-market-id').value = '';
            }
        });
    }

    // Staking modal handlers
    const closeModal = document.getElementById('close-modal');
    const placeStakeBtn = document.getElementById('place-stake-btn');
    const stakeAmountInput = document.getElementById('stake-amount');

    if (closeModal) {
        closeModal.addEventListener('click', closeStakingModal);
    }

    if (placeStakeBtn) {
        placeStakeBtn.addEventListener('click', () => {
            const amount = document.getElementById('stake-amount').value;
            if (currentStake && amount && parseFloat(amount) > 0) {
                placeStake(currentStake.marketId, currentStake.outcome, amount);
            }
        });
    }

    if (stakeAmountInput) {
        stakeAmountInput.addEventListener('input', updatePayout);
    }

    // Close modal when clicking outside
    const modal = document.getElementById('staking-modal');
    if (modal) {
        modal.addEventListener('click', (e) => {
            if (e.target === modal) {
                closeStakingModal();
            }
        });
    }

    // Load initial data
    checkContractStatus();
    loadMarkets();

    // Auto-connect user wallet if stored (only on main page, not admin)
    if (!window.isAdminMode) {
        const storedWallet = getUserWallet();
        if (storedWallet) {
            connectWallet();
        }
    }
});