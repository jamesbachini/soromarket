#!/bin/bash

# SoroMarket Deployment Script
# Builds, deploys, and sets up prediction markets for 2028 US Election

set -e  # Exit on any error

# Configuration
SOURCE_ACCOUNT="james"
NETWORK="testnet"
WASM_PATH="../../target/wasm32-unknown-unknown/release/soromarket.wasm"
TOKEN_CONTRACT="CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA"  # USDC testnet
ORACLE_ADDRESS="GD6ERVU2XC35LUZQ57JKTRF6DMCNF2JI5TFL7COH5FSQ4TZ2IBA3H55C" 
LIQUIDITY_PARAM="500000"  # 50% liquidity parameter

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 SoroMarket Deployment Script${NC}"
echo "================================="
echo ""

# Step 1: Build the contract
echo -e "${YELLOW}📦 Building contract...${NC}"
cd contracts/soromarket
cargo build --target wasm32-unknown-unknown --release

if [ ! -f "$WASM_PATH" ]; then
    echo -e "${RED}❌ Build failed - WASM file not found at $WASM_PATH${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Contract built successfully${NC}"
echo ""

# Step 2: Deploy contract (single deployment, will be reused for all markets)
echo -e "${YELLOW}🌐 Deploying contract...${NC}"
CONTRACT_ID=$(stellar contract deploy \
    --wasm "$WASM_PATH" \
    --source "$SOURCE_ACCOUNT" \
    --network "$NETWORK" \
    2>/dev/null | grep -o 'C[A-Z0-9]\{55\}')

if [ -z "$CONTRACT_ID" ]; then
    echo -e "${RED}❌ Contract deployment failed${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Contract deployed: $CONTRACT_ID${NC}"
echo ""

# Step 3: Deploy individual market instances for each candidate
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
        --wasm "$WASM_PATH" \
        --source "$SOURCE_ACCOUNT" \
        --network "$NETWORK" \
        2>/dev/null | grep -o 'C[A-Z0-9]\{55\}')
    
    if [ -z "$CANDIDATE_CONTRACT_ID" ]; then
        echo -e "${RED}❌ Failed to deploy contract for $candidate${NC}"
        continue
    fi
    
    echo "  📄 Contract ID: $CANDIDATE_CONTRACT_ID"
    CONTRACT_IDS[$candidate]=$CANDIDATE_CONTRACT_ID
    
    # Setup the market
    echo "  ⚙️  Initializing market..."
    
    setup_result=$(stellar contract invoke \
        --id "$CANDIDATE_CONTRACT_ID" \
        --source "$SOURCE_ACCOUNT" \
        --network "$NETWORK" \
        -- setup \
        --oracle "$ORACLE_ADDRESS" \
        --token "$TOKEN_CONTRACT" \
        --market "$description" \
        --liquidity_param "$LIQUIDITY_PARAM" \
        2>&1)
    
    if [ $? -eq 0 ]; then
        echo -e "  ${GREEN}✅ Market initialized successfully${NC}"
    else
        echo -e "  ${RED}❌ Market initialization failed: $setup_result${NC}"
        continue
    fi
    
    echo ""
done

# Step 4: Generate configuration file for frontend
echo -e "${YELLOW}📝 Generating frontend configuration...${NC}"

cat > ../../contract-addresses.json << EOF
{
  "network": "$NETWORK",
  "tokenContract": "$TOKEN_CONTRACT",
  "oracleAddress": "$ORACLE_ADDRESS", 
  "liquidityParam": $LIQUIDITY_PARAM,
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

# Step 5: Display summary
echo -e "${BLUE}📋 Deployment Summary${NC}"
echo "====================="
echo "Network: $NETWORK"
echo "Source Account: $SOURCE_ACCOUNT"
echo "Token Contract: $TOKEN_CONTRACT"
echo "Oracle Address: $ORACLE_ADDRESS"
echo "Liquidity Parameter: $LIQUIDITY_PARAM"
echo ""
echo "Market Contracts:"
for candidate in "${!CONTRACT_IDS[@]}"; do
    echo "  $candidate: ${CONTRACT_IDS[$candidate]}"
done
echo ""

# Step 6: Generate update command for frontend
echo -e "${YELLOW}🔧 Frontend Integration${NC}"
echo "To update your frontend with these contract addresses:"
echo ""
echo "1. Copy the addresses from contract-addresses.json"
echo "2. Update the CONFIG.contracts object in index.html"
echo "3. Update CONFIG.tokenContract with: $TOKEN_CONTRACT"
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
echo "Next steps:"
echo "1. Update frontend configuration with the contract addresses above"
echo "2. Ensure users have USDC tokens for betting"
echo "3. Test the betting functionality"
echo "4. Set up oracle for market settlement"