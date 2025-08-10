
let CONFIG = {
  rpcUrl: 'https://soroban-testnet.stellar.org',
  networkPassphrase: StellarSdk.Networks.TESTNET,
  contracts: {},
  tokenContract: '',
  refreshInterval: 30000 // 30 seconds
};

async function loadContractAddresses() {
  try {
    const response = await fetch('./contract-addresses.json');
    if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
    const contractData = await response.json();
    CONFIG.contracts = contractData.contracts;
    CONFIG.tokenContract = contractData.tokenContract;
    CONFIG.liquidityParam = contractData.liquidityParam;
    if (contractData.network === 'testnet') {
      CONFIG.networkPassphrase = StellarSdk.Networks.TESTNET;
      CONFIG.rpcUrl = 'https://soroban-testnet.stellar.org';
    } else if (contractData.network === 'mainnet') {
      CONFIG.networkPassphrase = StellarSdk.Networks.PUBLIC;
      CONFIG.rpcUrl = 'https://soroban-mainnet.stellar.org';
    }
    console.log('Contract addresses loaded:', CONFIG.contracts);
    console.log('Token contract:', CONFIG.tokenContract);
    return true;
  } catch (error) {
    console.error('Failed to load contract addresses:', error);
    showMessage('Failed to load contract configuration. Using fallback addresses.', 'warning');
  }
}

let rpc;
let keypair;
let isLoading = false;
let marketData = {};
let userPositions = {};
let marketOutcomes = {};

// Initialize market data structures based on loaded contracts
function initializeMarketData() {
  const candidates = Object.keys(CONFIG.contracts);
  
  marketData = {};
  userPositions = {};
  marketOutcomes = {};
  
  candidates.forEach(candidate => {
    marketData[candidate] = { 
      trueShares: 0, falseShares: 0, trueTotal: 0, falseTotal: 0, 
      state: null, probabilities: null, loaded: false 
    };
    userPositions[candidate] = { 
      yesShares: 0, noShares: 0, claimed: false 
    };
    marketOutcomes[candidate] = null; // null = undecided, true = won, false = lost
  });
}

(async function init() {
  try {
    showMessage('Initializing SoroMarket...', 'info');
    
    // Load contract addresses first
    showMessage('Loading contract configuration...', 'info');
    await loadContractAddresses();
    
    // Initialize market data structures with loaded contracts
    initializeMarketData();
    
    // Initialize RPC with loaded config
    const RpcServer = StellarSdk.SorobanRpc?.Server || StellarSdk.rpc.Server;
    rpc = new RpcServer(CONFIG.rpcUrl);
    
    // Initialize wallet
    const savedSecret = localStorage.getItem('soroMarketSecret');
    if (savedSecret) {
      keypair = StellarSdk.Keypair.fromSecret(savedSecret);
      console.log('Loaded existing keypair from localStorage:', keypair.publicKey());
    } else {
      keypair = await createFundedAccount();
      localStorage.setItem('soroMarketSecret', keypair.secret());
      console.log('Created new keypair and saved to localStorage:', keypair.publicKey());
    }
    
    updateWalletInfo();
    await loadMarketData();
    setupEventListeners();
    updateAllUserBalances();
    
    // Start periodic updates
    setInterval(() => {
      loadMarketData();
      updateAllUserBalances();
      updateWalletInfo();
    }, CONFIG.refreshInterval);

    showMessage('Welcome to SoroMarket! Ready to trade 2028 election outcomes.', 'success');

  } catch (error) {
    console.error('Initialization error:', error);
    showMessage('Failed to initialize. Please refresh the page.', 'error');
  }
})();


async function createFundedAccount() {
  const kp = StellarSdk.Keypair.random();
  try {
    await fetch(`https://friendbot.stellar.org/?addr=${kp.publicKey()}`);
    setTimeout(async () => {
        showMessage('Minting testnet USDC to your new wallet', 'info');
        await callContractMethod(CONFIG.tokenContract, 'mint', [
            kp.publicKey(),
            100000000000 // $100k
        ], true);
        updateWalletInfo();
    }, 2000);
    return kp;
  } catch (error) {
    throw new Error('Failed to create funded account');
  }
}

async function updateWalletInfo() {
  const walletInfo = document.getElementById('walletInfo');
  const publicKey = keypair.publicKey();
  const tokenBalance = await callContractMethod(CONFIG.tokenContract, 'balance', [publicKey]);
  const formattedTokenBalance = new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2,
    maximumFractionDigits: 2
    }).format(tokenBalance / 1000000n);
  walletInfo.innerHTML = `
    <div style="color: var(--success);">• Wallet Connected :: Testnet •</div>
    <div style="font-size: 0.6em; margin: 4px 0px;">${publicKey}</div>
    <div style="color: var(--accent);">USDC Balance: ${formattedTokenBalance}</div>
  `;
}

async function loadMarketData() {
  try {
    const candidates = Object.keys(CONFIG.contracts);
    await Promise.all(
      candidates.map(candidate => loadCandidateData(candidate))
    );
    updateAllPrices();
  } catch (error) {
    console.error('Error loading market data:', error);
  }
}

async function loadCandidateData(candidate) {
  try {
    const contractAddress = CONFIG.contracts[candidate];
    const [marketInfo, probabilities, marketState] = await Promise.all([
      callContractMethod(contractAddress, 'get_market_info'),
      callContractMethod(contractAddress, 'get_current_probabilities'), 
      callContractMethod(contractAddress, 'get_market_state')
    ]);
    // marketInfo returns (trueShares, falseShares, trueTotal, falseTotal)
    if (!marketInfo) return;
    marketData[candidate] = {
      trueShares: marketInfo[0] || 0,
      falseShares: marketInfo[1] || 0, 
      trueTotal: marketInfo[2] || 0,
      falseTotal: marketInfo[3] || 0,
      state: marketState,
      probabilities: probabilities, // (trueProbability, falseProbability)
      loaded: true
    };
    //console.log(marketData, marketData[candidate])
  } catch (error) {
    console.error(`Error loading data for ${candidate}:`, error);
    // Keep loading state as false for error cases
    marketData[candidate] = {
      trueShares: 0,
      falseShares: 0,
      trueTotal: 0,
      falseTotal: 0,
      state: null,
      probabilities: null,
      loaded: false
    };
  }
}

function updateAllPrices() {
  const candidates = Object.keys(CONFIG.contracts);
  candidates.forEach(candidate => {
    updateCandidatePrices(candidate);
  });
}

function updateCandidatePrices(candidate) {
  const data = marketData[candidate];
  if (!data.loaded) {
    document.getElementById(`${candidate}-probability`).textContent = '...%';
    document.getElementById(`${candidate}-volume`).textContent = '$0';
    document.getElementById(`${candidate}-yes-price`).textContent = '$...';
    document.getElementById(`${candidate}-no-price`).textContent = '$...';
    return;
  }
  let yesPrice, noPrice;
  if (data.probabilities) {
    yesPrice = Number(data.probabilities[0]) / 1_000_000;
    noPrice = Number(data.probabilities[1]) / 1_000_000;
  } else {
    yesPrice = 0.5;
    noPrice = 0.5;
  }
  document.getElementById(`${candidate}-probability`).textContent = `${(yesPrice * 100).toFixed(2)}%`;
  const totalVolume = (data.trueTotal || 0n) + (data.falseTotal || 0n);
  document.getElementById(`${candidate}-volume`).textContent = `$${totalVolume / 1_000_000n}`;
  if (document.getElementById(`${candidate}-yes-price`) && document.getElementById(`${candidate}-no-price`)) {
    document.getElementById(`${candidate}-yes-price`).textContent = `$${yesPrice.toFixed(2)}`;
    document.getElementById(`${candidate}-no-price`).textContent = `$${noPrice.toFixed(2)}`;
  }
  updateSharesDisplay(candidate);
}

async function updateSharesDisplay(candidate) {
  const amountInput = document.getElementById(`${candidate}-amount`);
  const sharesDisplay = document.getElementById(`${candidate}-shares`);
  const amount = parseFloat(amountInput.value) || 0;
  if (amount > 0 && marketData[candidate].loaded) {
    try {
      const contractAddress = CONFIG.contracts[candidate];
      const shares = await callContractMethod(contractAddress, 'get_shares_for_cost', [
        amount * 1_000_000n,
        true
      ]);
      sharesDisplay.value = shares;
    } catch (error) {
      sharesDisplay.value = '...';
    }
  } else {
    sharesDisplay.value = amount > 0n ? '...' : '0';
  }
}

function setupEventListeners() {
  const candidates = Object.keys(CONFIG.contracts);
  candidates.forEach(candidate => {
    const amountInput = document.getElementById(`${candidate}-amount`);
    if (amountInput) {
      amountInput.addEventListener('input', () => updateSharesDisplay(candidate));
    }
  });
}

async function placeBet(candidate, betOnTrue) {
  if (isLoading) return;
  const amountInput = document.getElementById(`${candidate}-amount`);
  const amount = parseFloat(amountInput.value);
  if (!amount || amount < 1) {
    showMessage('Please enter a valid amount (minimum $1)', 'error');
    return;
  }
  try {
    isLoading = true;
    setLoadingState(candidate, true);
    showMessage(`Placing ${betOnTrue ? 'YES' : 'NO'} bet on ${getDisplayName(candidate)}...`, 'info');
    const contractAddress = CONFIG.contracts[candidate];
    const scaledAmount = Math.floor(amount * 1_000_000);
    const tokenContract = CONFIG.tokenContract;
    try {
      const allowance = await callContractMethod(tokenContract, 'allowance', [
        keypair.publicKey(),
        contractAddress
      ]);
      if (allowance < scaledAmount) {
        await callContractMethod(tokenContract, 'approve', [
          keypair.publicKey(),
          contractAddress,
          scaledAmount,
          'live_until_ledger', // live_until_ledger max u32
        ], true);
        showMessage('Token approval confirmed...');
        console.log('Token approval confirmed...');
        await new Promise(r => setTimeout(r, 10000));
      }
    } catch (error) {
      console.warn('Token approval check failed, proceeding...', error);
    }
    await callContractMethod(contractAddress, 'trade', [
      keypair.publicKey(),
      scaledAmount,
      betOnTrue
    ], true);
    amountInput.value = '';
    await loadMarketData();
    await updateAllUserBalances();
    showMessage(
      `Successfully placed ${betOnTrue ? 'YES' : 'NO'} bet on ${getDisplayName(candidate)} for $${amount}!`, 
      'success'
    );
  } catch (error) {
    console.error('Betting error:', error);
    showMessage(`Failed to place bet: ${error.message}`, 'error');
  } finally {
    isLoading = false;
    setLoadingState(candidate, false);
  }
}

function getDisplayName(candidate) {
  const names = {
    'vance': 'JD Vance',
    'newsom': 'Gavin Newsom', 
    'aoc': 'Alexandria Ocasio-Cortez',
    'buttigieg': 'Pete Buttigieg',
    'rubio': 'Marco Rubio',
    'beshear': 'Andy Beshear'
  };
  return names[candidate] || candidate;
}

function setLoadingState(candidate, loading) {
  //console.log(candidate)
  const card = document.querySelector(`[data-candidate="${candidate}"]`);
  const buttons = card.querySelectorAll('.btn');
  if (loading) {
    card.classList.add('loading');
    buttons.forEach(btn => {
      btn.disabled = true;
      const originalText = btn.innerHTML;
      btn.setAttribute('data-original-text', originalText);
      btn.innerHTML = '<span class="spinner"></span>';
    });
  } else {
    card.classList.remove('loading');
    buttons.forEach(btn => {
      btn.disabled = false;
      const originalText = btn.getAttribute('data-original-text');
      if (originalText) {
        btn.innerHTML = originalText;
      }
    });
  }
}

function showMessage(text, type = 'info') {
  const container = document.getElementById('messageContainer');
  const messageEl = document.createElement('div');
  messageEl.className = `message ${type}`;
  messageEl.textContent = text;
  container.appendChild(messageEl);
  setTimeout(() => {
    if (messageEl.parentNode) messageEl.parentNode.removeChild(messageEl);
  }, 5000);
}

async function callContractMethod(contractAddress, method, params = [], sendTx = false) {
    //console.log(method, params);
    try {
        const contract = new StellarSdk.Contract(contractAddress);
        const account = await rpc.getAccount(keypair.publicKey());

        // Convert params to SCVals
        const convertedParams = params.map(param => {
            if (typeof param === 'string') {
                if (param === 'live_until_ledger') return StellarSdk.nativeToScVal(3110400, { type: "u32" });
                
                if (param.length === 56 && (param.startsWith('G') || param.startsWith('C'))) {
                    // Stellar address or contract ID
                    return StellarSdk.Address.fromString(param).toScVal();
                }
                // Treat as integer string
                return StellarSdk.nativeToScVal(param, { type: "i128" });
            } else if (typeof param === 'number' || typeof param === 'bigint') {
                return StellarSdk.nativeToScVal(BigInt(param), { type: "i128" });
            } else if (typeof param === 'boolean') {
                return StellarSdk.nativeToScVal(param, { type: "bool" });
            }
            return StellarSdk.nativeToScVal(param);
        });
        let tx = new StellarSdk.TransactionBuilder(account, {
            fee: StellarSdk.BASE_FEE,
            networkPassphrase: CONFIG.networkPassphrase
        }).addOperation(contract.call(method, ...convertedParams)).setTimeout(30).build();
        tx = await rpc.prepareTransaction(tx);
        // Send tx to RPC nodes
        if (sendTx) {
            tx.sign(keypair);
            const result = await rpc.sendTransaction(tx);
            console.log('tx hash: ', result.hash)
            if (result.status === 'PENDING') {
                let finalResult;
                for (let i = 0; i < 10; i++) { // 10 retries (~20s total)
                    await new Promise(r => setTimeout(r, 2000)); // wait 2 seconds
                    finalResult = await rpc.getTransaction(result.hash);
                    if (finalResult.status !== 'NOT_FOUND' && finalResult.status !== 'PENDING') break;
                }
                if (!finalResult || finalResult.status === 'PENDING' || finalResult.status === 'NOT_FOUND') {
                    throw new Error('Transaction still pending after timeout');
                }
                if (finalResult.status === 'SUCCESS') {
                    return finalResult.returnValue
                        ? StellarSdk.scValToNative(finalResult.returnValue)
                        : null;
                } else {
                    throw new Error(`Transaction failed: ${finalResult.status}`);
                }
            } else if (result.status === 'SUCCESS') {
                return result.returnValue
                    ? StellarSdk.scValToNative(result.returnValue)
                    : null;
            } else {
                throw new Error(`Transaction failed: ${result.status}`);
            }
        } else {
            // Simulate transaction
            const simResult = await rpc.simulateTransaction(tx);
            if (simResult.error) throw new Error(`Simulation failed: ${simResult.error}`);
            if (simResult.result.retval) {
                return StellarSdk.scValToNative(simResult.result.retval);
            } else {
                return null; // No results — still valid
            }
        }
    } catch (error) {
        console.error(`Contract call error for ${method}:`, error);
        throw error;
    }
}

function formatCurrency(amount) {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    minimumFractionDigits: 2
  }).format(amount);
}

function formatNumber(num) {
  if (num >= 1000000) {
    return (num / 1000000).toFixed(1) + 'M';
  } else if (num >= 1000) {
    return (num / 1000).toFixed(1) + 'K';
  }
  return num.toLocaleString();
}

async function updateAllUserBalances() {
  const candidates = Object.keys(CONFIG.contracts);
  const promises = candidates.map(candidate => updateUserBalance(candidate));
  await Promise.all(promises);
}

async function updateUserBalance(candidate) {
  try {
    const contractAddress = CONFIG.contracts[candidate];
    const userBet = await callContractMethod(contractAddress, 'get_user_shares', [
      keypair.publicKey()
    ]);
    let position = userPositions[candidate];
    if (userBet) {
      if (userBet.bet_on_true) {
        position.yesShares = userBet[0];
      } else {
        position.noShares = userBet[1];
      }
      position.claimed = userBet.claimed;
    }
    document.getElementById(`${candidate}-yes-balance`).textContent = position.yesShares;
    document.getElementById(`${candidate}-no-balance`).textContent = position.noShares;
    const marketState = await callContractMethod(contractAddress, 'get_market_state');
    const isSettled = marketState !== 'Undecided';
    const claimBtn = document.getElementById(`${candidate}-claim-btn`);
    const hasShares = position.yesShares > 0 || position.noShares > 0;
    const hasWinningShares = isSettled && (
      (marketState === 'TrueOutcome' && position.yesShares > 0) ||
      (marketState === 'FalseOutcome' && position.noShares > 0)
    );
    if (hasWinningShares && !position.claimed) {
      claimBtn.disabled = false;
      claimBtn.textContent = 'Claim Winnings';
    } else if (position.claimed) {
      claimBtn.disabled = true;
      claimBtn.textContent = 'Claimed ✓';
    } else if (isSettled) {
      claimBtn.disabled = true;
      claimBtn.textContent = 'No Winnings';
    } else {
      claimBtn.disabled = true;
      claimBtn.textContent = hasShares ? 'Market Not Settled' : 'No Position';
    }
  } catch (error) {
    console.error(`Error updating user balance for ${candidate}:`, error);
    const position = userPositions[candidate];
    document.getElementById(`${candidate}-yes-balance`).textContent = position.yesShares.toLocaleString();
    document.getElementById(`${candidate}-no-balance`).textContent = position.noShares.toLocaleString();
    const claimBtn = document.getElementById(`${candidate}-claim-btn`);
    claimBtn.disabled = true;
    claimBtn.textContent = 'Contract Error';
  }
}

async function claimWinnings(candidate) {
  if (isLoading) return;
  try {
    isLoading = true;
    setLoadingState(candidate, true);
    showMessage(`Claiming winnings for ${getDisplayName(candidate)}...`, 'info');
    const contractAddress = CONFIG.contracts[candidate];
    await callContractMethod(contractAddress, 'claim', [
      keypair.publicKey()
    ], true);
    await loadMarketData();
    await updateAllUserBalances();
    showMessage(
      `Successfully claimed winnings from ${getDisplayName(candidate)}!`, 
      'success'
    );
  } catch (error) {
    console.error('Claiming error:', error);
    showMessage(`Failed to claim winnings: ${error.message}`, 'error');
  } finally {
    isLoading = false;
    setLoadingState(candidate, false);
  }
}

console.log('🗳️ SoroMarket initialized - 2028 Election Prediction Markets');