#!/bin/bash

# SoroMarket Deployment Script
# Builds, deploys, and sets up prediction markets for 2028 US Election

set -e  # Exit on any error

# Configuration
SOURCE_ACCOUNT="james"
NETWORK="testnet"
SOROMARKET_WASM_PATH="../../target/wasm32-unknown-unknown/release/soromarket.wasm"
USDC_WASM_PATH="../../target/wasm32-unknown-unknown/release/usdc.wasm"
ORACLE_ADDRESS="GD6ERVU2XC35LUZQ57JKTRF6DMCNF2JI5TFL7COH5FSQ4TZ2IBA3H55C" 
INITIAL_RESERVE="1000000000"  # 1000 USDC (6 decimals) initial reserve per side

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 SoroMarket Deployment Script${NC}"
echo "================================="
echo ""

# Step 1: Build the contracts
echo -e "${YELLOW}📦 Building contracts...${NC}"

# Build USDC contract
echo "Building USDC contract..."
cd contracts/usdc
cargo build --target wasm32-unknown-unknown --release

if [ ! -f "$USDC_WASM_PATH" ]; then
    echo -e "${RED}❌ USDC contract build failed - WASM file not found at $USDC_WASM_PATH${NC}"
    exit 1
fi

# Build SoroMarket contract
echo "Building SoroMarket contract..."
cd ../soromarket
cargo build --target wasm32-unknown-unknown --release

if [ ! -f "$SOROMARKET_WASM_PATH" ]; then
    echo -e "${RED}❌ SoroMarket contract build failed - WASM file not found at $SOROMARKET_WASM_PATH${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Contracts built successfully${NC}"
echo ""

# Step 2: Deploy USDC token contract
echo -e "${YELLOW}💰 Deploying USDC token contract...${NC}"
USDC_CONTRACT_ID=$(stellar contract deploy \
    --wasm "$USDC_WASM_PATH" \
    --source "$SOURCE_ACCOUNT" \
    --network "$NETWORK" \
    2>/dev/null | grep -o 'C[A-Z0-9]\{55\}')

if [ -z "$USDC_CONTRACT_ID" ]; then
    echo -e "${RED}❌ USDC contract deployment failed${NC}"
    exit 1
fi

echo -e "${GREEN}✅ USDC contract deployed: $USDC_CONTRACT_ID${NC}"

# Step 3: Deploy SoroMarket contract (single deployment, will be reused for all markets)
echo -e "${YELLOW}🌐 Deploying SoroMarket contract...${NC}"
CONTRACT_ID=$(stellar contract deploy \
    --wasm "$SOROMARKET_WASM_PATH" \
    --source "$SOURCE_ACCOUNT" \
    --network "$NETWORK" \
    2>/dev/null | grep -o 'C[A-Z0-9]\{55\}')

if [ -z "$CONTRACT_ID" ]; then
    echo -e "${RED}❌ SoroMarket contract deployment failed${NC}"
    exit 1
fi

echo -e "${GREEN}✅ SoroMarket contract deployed: $CONTRACT_ID${NC}"
echo ""

# Step 4: Deploy individual market instances for each candidate
declare -A CANDIDATES=(
    ["vance"]="JD Vance - 2028 US Presidential Election"
    ["newsom"]="Gavin Newsom - 2028 US Presidential Election"  
    ["aoc"]="Alexandria Ocasio-Cortez - 2028 US Presidential Election"
    ["buttigieg"]="Pete Buttigieg - 2028 US Presidential Election"
    ["rubio"]="Marco Rubio - 2028 US Presidential Election"
    ["beshear"]="Andy Beshear - 2028 US Presidential Election"
)

declare -A CONTRACT_IDS

echo -e "${YELLOW}🗳️  Setting up prediction markets...${NC}"
echo ""

for candidate in "${!CANDIDATES[@]}"; do
    description="${CANDIDATES[$candidate]}"
    
    echo -e "${BLUE}Setting up market for: $description${NC}"
    
    # Deploy a new instance for this candidate
    CANDIDATE_CONTRACT_ID=$(stellar contract deploy \
        --wasm "$SOROMARKET_WASM_PATH" \
        --source "$SOURCE_ACCOUNT" \
        --network "$NETWORK" \
        2>/dev/null | grep -o 'C[A-Z0-9]\{55\}')
    
    if [ -z "$CANDIDATE_CONTRACT_ID" ]; then
        echo -e "${RED}❌ Failed to deploy contract for $candidate${NC}"
        continue
    fi
    
    echo "  📄 Contract ID: $CANDIDATE_CONTRACT_ID"
    CONTRACT_IDS[$candidate]=$CANDIDATE_CONTRACT_ID
    
    # Get source account address
    SOURCE_ADDRESS=$(stellar keys address "$SOURCE_ACCOUNT" 2>/dev/null)

    # Mint tokens for trading and initial liquidity
    echo "  💰 Minting USDC to: ${SOURCE_ADDRESS}"
    MINT_AMOUNT="10000000000"  # 10,000 USDC for testing

    stellar contract invoke \
        --id "$USDC_CONTRACT_ID" \
        --source "$SOURCE_ACCOUNT" \
        --network "$NETWORK" \
        -- mint \
        --account "$SOURCE_ADDRESS" \
        --amount "$MINT_AMOUNT" \
        2>/dev/null

    # Approve spend for initial liquidity - change live_until_ledger for mainnet
    echo "  🎟️ Approving spend for initial liquidity..."
    stellar contract invoke \
        --id "$USDC_CONTRACT_ID" \
        --source "$SOURCE_ACCOUNT" \
        --network "$NETWORK" \
        -- approve \
        --owner "$SOURCE_ADDRESS" \
        --spender "$CANDIDATE_CONTRACT_ID" \
        --amount "$MINT_AMOUNT" \
        --live_until_ledger "3110400" \
        2>/dev/null
    
    # Setup the market (this will transfer initial liquidity)
    echo "  ⚙️  Initializing market with liquidity..."
    
    setup_result=$(stellar contract invoke \
        --id "$CANDIDATE_CONTRACT_ID" \
        --source "$SOURCE_ACCOUNT" \
        --network "$NETWORK" \
        -- setup \
        --deployer "$SOURCE_ADDRESS" \
        --oracle "$ORACLE_ADDRESS" \
        --token "$USDC_CONTRACT_ID" \
        --market "$description" \
        --initial_reserve "$INITIAL_RESERVE" \
        2>&1)
    
    if [ $? -eq 0 ]; then
        echo -e "  ${GREEN}✅ Market initialized with ${INITIAL_RESERVE} USDC reserves on each side${NC}"
    else
        echo -e "  ${RED}❌ Market initialization failed: $setup_result${NC}"
        continue
    fi
    echo ""
done

# Step 5: Generate configuration file for frontend
echo -e "${YELLOW}📝 Generating frontend configuration...${NC}"

cat > ../../contract-addresses.json << EOF
{
  "network": "$NETWORK",
  "tokenContract": "$USDC_CONTRACT_ID",
  "oracleAddress": "$ORACLE_ADDRESS",
  "contracts": {
EOF

first=true
for candidate in "${!CONTRACT_IDS[@]}"; do
    if [ "$first" = true ]; then
        first=false
    else
        echo "," >> ../../contract-addresses.json
    fi
    echo "    \"$candidate\": \"${CONTRACT_IDS[$candidate]}\"" >> ../../contract-addresses.json
done

cat >> ../../contract-addresses.json << EOF
  }
}
EOF

echo -e "${GREEN}✅ Configuration saved to contract-addresses.json${NC}"
echo ""

# Step 6: Display summary
echo -e "${BLUE}📋 Deployment Summary${NC}"
echo "====================="
echo "Network: $NETWORK"
echo "Source Account: $SOURCE_ACCOUNT"
echo "USDC Token Contract: $USDC_CONTRACT_ID"
echo "Oracle Address: $ORACLE_ADDRESS"
echo ""
echo "Market Contracts:"
for candidate in "${!CONTRACT_IDS[@]}"; do
    echo "  $candidate: ${CONTRACT_IDS[$candidate]}"
done
echo ""

# Step 7: Test a view function on one contract
if [ ${#CONTRACT_IDS[@]} -gt 0 ]; then
    test_candidate=$(echo "${!CONTRACT_IDS[@]}" | cut -d' ' -f1)
    test_contract_id="${CONTRACT_IDS[$test_candidate]}"
    
    echo -e "${YELLOW}🧪 Testing market info for $test_candidate...${NC}"
    
    market_info=$(stellar contract invoke \
        --id "$test_contract_id" \
        --source "$SOURCE_ACCOUNT" \
        --network "$NETWORK" \
        -- get_market_info \
        2>/dev/null)
    
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✅ Market info: $market_info${NC}"
    else
        echo -e "${RED}❌ Failed to get market info${NC}"
    fi
fi

echo ""
echo -e "${GREEN}🎉 Deployment complete! All markets are ready for trading.${NC}"
echo ""
