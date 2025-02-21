use ethers::{ 
    abi::Abi,
    types::{
        Transaction,
        transaction::eip2718::TypedTransaction,
         H160, 
         U256
        }, contract::{abigen, Contract}, providers::{Middleware, Provider, Ws}
};
use anyhow::{Result, anyhow};
use std::sync::Arc;
use lazy_static::lazy_static;

abigen!(IERC20, "./IERC20.json");


lazy_static! {
    pub static ref UNISWAP_V2_ROUTER_ABI: Abi = serde_json::from_str(include_str!("../UniswapV2RouterABI.json")).unwrap();
    pub static ref POOL_ABI: Abi = serde_json::from_str(include_str!("../UniswapV2PairABI.json")).unwrap();
    pub static ref FACTORY_ABI: Abi = serde_json::from_str(include_str!("../UniswapV2FactoryABI.json")).unwrap();
    pub static ref CONTRACT_ABI: Abi = serde_json::from_str(include_str!("SniperBotABI.json")).unwrap();
}


pub async fn get_pool_address(
    provider: Arc<Provider<Ws>>,
    factory_address: H160,
    token_in: H160,
    token_out: H160,
) -> Result<H160> {
    let factory = Contract::new(factory_address, FACTORY_ABI.clone(), provider.clone());
    let pair_address = factory
        .method("getPair", (token_in, token_out))?
        .call()
        .await
        .map_err(|e| anyhow!("Failed to query pair address: {}", e))?;

    if pair_address == H160::zero() {
        return Err(anyhow!("No pair found for tokens {} and {}", token_in, token_out));
    }

    Ok(pair_address)
}

pub async fn gas_estimate(tx: &TypedTransaction, provider: Arc<Provider<Ws>>) -> Result<U256> {
    provider
        .estimate_gas(tx, None)
        .await
        .map_err(|e| anyhow!("Failed to get gas estimate: {:?}", e))
}

pub async fn check_contract_balance(
    provider: Arc<Provider<Ws>>,
    contract_address: H160,
    token_address: H160,
) -> Result<U256> {
    let token = IERC20::new(token_address, provider.clone());
    let balance = token.balance_of(contract_address).call().await?;
    Ok(balance)
}

pub async fn is_target_pair(tx: &Transaction, target_token_in: H160, target_token_out: H160) -> bool {
    let decoded_tx = decode_transaction(tx).await;
    match decoded_tx {
        Ok((token_in, token_out, _)) => {
            (token_in == target_token_in && token_out == target_token_out)
                || (token_in == target_token_out && token_out == target_token_in)
        }
        Err(_) => false,
    }
}
pub async fn decode_transaction(tx: &Transaction) -> Result<(H160, H160, U256)> {
    let func = UNISWAP_V2_ROUTER_ABI
        .function("swapExactTokensForTokens")
        .map_err(|e| anyhow!("Failed to load UniswapV2Router function: {:?}", e))?;

    let decoded = func
        .decode_input(&tx.input)
        .map_err(|e| anyhow!("Failed to decode transaction: {:?}", e))?;

    let token_in = decoded[0]
        .clone()
        .into_address()
        .ok_or(anyhow!("Error decoding token_in"))?;
    let token_out = decoded[1]
        .clone()
        .into_address()
        .ok_or(anyhow!("Error decoding token_out"))?;
    let amount_in = decoded[2]
        .clone()
        .into_uint()
        .ok_or(anyhow!("Error decoding amount_in"))?;

    Ok((token_in, token_out, amount_in))
}

pub fn calculate_minout(expected: U256, slippage_bps: U256) -> U256 {
    expected * (U256::from(10_000) - slippage_bps) / U256::from(10_000)
}
