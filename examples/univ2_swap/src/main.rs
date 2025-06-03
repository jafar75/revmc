use std::env;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::ops::{Div, Mul};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use alloy::sol;
use alloy::sol_types::SolCall;
use alloy::sol_types::SolValue;
use alloy_primitives::{keccak256, Address, Bytes, TxKind, B256};
use anyhow::anyhow;
use ethers_core::types::{BlockId, BlockNumber};
use ethers_providers::{Http, Middleware, ProviderExt};
use revm::{db::{CacheDB, EmptyDB}, primitives::{address, hex, AccountInfo, Bytecode, TransactTo, U256}, Context};
use revm::db::{DbAccount, EthersDB};
use revm::primitives::{BlockEnv, CfgEnv, CfgEnvWithHandlerCfg, EnvWithHandlerCfg, ExecutionResult, HandlerCfg, Output, SpecId, TxEnv};
use tracing::info;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use revmc_context::private::revm_primitives::KECCAK_EMPTY;
use revmc_examples_univ2::{build_evm, deploy_and_commit};
use crate::r#static::{CALLDATA_FOR_TEST, CONTRACT_ADDRESS};

pub mod r#static;

include!("./common.rs");
#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        // One could add more layers here, for example logging to stdout:
        .with(LevelFilter::INFO)
        .with(tracing_subscriber::fmt::Layer::new())
        .init();

    let node_url = env::var("NODE_URL").unwrap_or("https://mainnet.infura.io/v3/c60b0bb42f8a4c6481ecd229eddaca27".to_string());
    let state = Arc::new(ethers_providers::Provider::<Http>::connect(node_url.as_str()).await);

    let block_id = BlockId::Number(BlockNumber::from(22515915_u64));
    let mut ethers_db = revm::db::EthersDB::new(Arc::clone(&state), Some(block_id)).expect("error in create ether db");
    let mut cache_db = CacheDB::new(ethers_db);

    let account = address!("18B06aaF27d44B756FCF16Ca20C1f183EB49111f");

    let weth = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let usdc_weth_pair = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");

    let weth_balance_slot = U256::from(3);
    sol! {
        function swap(uint amount0Out, uint amount1Out, address target, bytes callback) external;
    }


    // Give our test account some fake WETH and ETH
    let one_ether = U256::from(1_000_000_000_000_000_000u128);
    let hashed_acc_balance_slot = keccak256((account, weth_balance_slot).abi_encode());
    cache_db
        .insert_account_storage(weth, hashed_acc_balance_slot.into(), one_ether)
        .unwrap();

    let acc_info = AccountInfo {
        nonce: 0_u64,
        balance: one_ether,
        code_hash: KECCAK_EMPTY,
        code: None,
    };
    cache_db.insert_account_info(account, acc_info);

    let acc_weth_balance_before = balance_of(weth, account, &mut cache_db).unwrap();
    println!("WETH balance before swap: {}", acc_weth_balance_before);
    let acc_usdc_balance_before = balance_of(usdc, account, &mut cache_db).unwrap();
    println!("USDC balance before swap: {}", acc_usdc_balance_before);
    let (reserve0, reserve1) = get_reserves(usdc_weth_pair, &mut cache_db).unwrap();

    let amount_in = one_ether.div(U256::from(10));

    // Calculate USDC amount out
    let amount_out = get_amount_out(amount_in, reserve1, reserve0, &mut cache_db).await.unwrap();
    println!("amount in: {:?}    amount out: {:?}", amount_in, amount_out);

    // Transfer WETH to USDC-WETH pair
    transfer(account, usdc_weth_pair, amount_in, weth, &mut cache_db).unwrap();
    // Execute low-level swap without using UniswapV2 router


    let amount0_out = if true { amount_out } else { U256::from(0) };
    let amount1_out = if true { U256::from(0) } else { amount_out };

    let encoded = swapCall {
        amount0Out: amount0_out,
        amount1Out: amount1_out,
        target: account,
        callback: Bytes::new(),
    }.abi_encode();


    let mut evm = build_evm(cache_db);
    evm.context.evm.env.tx.transact_to = TransactTo::Call(usdc_weth_pair);
    evm.context.evm.env.tx.data = encoded.into();
    evm.context.evm.env.tx.caller = account;

    let res = evm.transact().unwrap();
    let acc_weth_balance_after = balance_of(weth, account, &mut evm.context.evm.db).unwrap();
    println!("WETH balance after swap: {}", acc_weth_balance_after);
    let acc_usdc_balance_after = balance_of(usdc, account, &mut evm.context.evm.db).unwrap();
    println!("USDC balance after swap: {}", acc_usdc_balance_after);

    // some sleep for warm up
    tokio::time::sleep(Duration::from_secs(2)).await;

    let t = Instant::now();
    for i in 0..100000 {
        let res = evm.transact().unwrap();
    }

    println!("time elapsed for swap: {:?}", t.elapsed());
}


fn balance_of<M>(token: Address, address: Address, alloy_db: &mut CacheDB<EthersDB<M>>) -> anyhow::Result<U256>
where
    M: Middleware,
{
    sol! {
        function balanceOf(address account) public returns (uint256);
    }

    let encoded = balanceOfCall { account: address }.abi_encode();
    let cfg = CfgEnvWithHandlerCfg::new(CfgEnv::default(), HandlerCfg::new(SpecId::CANCUN));
    let env = EnvWithHandlerCfg::new_with_cfg_env(
        cfg,
        BlockEnv{
            ..Default::default()
        },
        TxEnv {
            caller: address!("0000000000000000000000000000000000000001"),
            transact_to: TxKind::Call(token),
            data: encoded.into(),
            value: U256::ZERO,
            ..Default::default()
        },
    );

    let mut evm = revm::Evm::builder()
        .with_db(alloy_db)
        .with_env_with_handler_cfg(env)
        .build();

    let res = evm.transact().unwrap();



    // let ref_tx = evm.replay().unwrap();
    // let result = ref_tx.result;

    let result = res.result;

    let value = match result {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => value,
        result => return Err(anyhow!("'balanceOf' execution failed: {result:?}")),
    };

    let balance = <U256>::abi_decode(&value, false)?;

    Ok(balance)
}

fn transfer<M>(
    from: Address,
    to: Address,
    amount: U256,
    token: Address,
    cache_db: &mut CacheDB<EthersDB<M>>,
) -> anyhow::Result<()>
where
    M: Middleware,
{
    sol! {
        function transfer(address to, uint amount) external returns (bool);
    }

    let encoded = transferCall { to, amount }.abi_encode();

    let cfg = CfgEnvWithHandlerCfg::new(CfgEnv::default(), HandlerCfg::new(SpecId::CANCUN));
    let env = EnvWithHandlerCfg::new_with_cfg_env(
        cfg,
        BlockEnv{
            ..Default::default()
        },
        TxEnv {
            caller: from,
            transact_to: TxKind::Call(token),
            data: encoded.into(),
            value: U256::ZERO,
            ..Default::default()
        },
    );

    let mut evm = revm::Evm::builder()
        .with_db(cache_db)
        .with_env_with_handler_cfg(env)
        .build();

    let res = evm.transact_commit().unwrap();

    // let ref_tx = evm.replay().unwrap();
    // let result = ref_tx.result;

    let ref_tx = res;

    // let ref_tx = evm.replay_commit().unwrap();
    let success: bool = match ref_tx {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => <bool>::abi_decode(&value, false)?,
        result => return Err(anyhow!("'transfer' execution failed: {result:?}")),
    };

    if !success {
        return Err(anyhow!("'transfer' failed"));
    }

    Ok(())
}

fn get_reserves<M>(pair_address: Address, cache_db: &mut CacheDB<EthersDB<M>>) -> anyhow::Result<(U256, U256)>
where
    M: Middleware,
{
    sol! {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }

    let encoded = getReservesCall {}.abi_encode();

    let cfg = CfgEnvWithHandlerCfg::new(CfgEnv::default(), HandlerCfg::new(SpecId::CANCUN));
    let env = EnvWithHandlerCfg::new_with_cfg_env(
        cfg,
        BlockEnv{
            ..Default::default()
        },
        TxEnv {
            caller: address!("0000000000000000000000000000000000000001"),
            transact_to: TxKind::Call(pair_address),
            data: encoded.into(),
            value: U256::ZERO,
            ..Default::default()
        },
    );

    let mut evm = revm::Evm::builder()
        .with_db(cache_db)
        .with_env_with_handler_cfg(env)
        .build();

    let res = evm.transact().unwrap();

    // let ref_tx = evm.replay().unwrap();
    // let result = ref_tx.result;

    let result = res.result;

    let value = match result {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => value,
        result => return Err(anyhow!("'getReserves' execution failed: {result:?}")),
    };

    let (reserve0, reserve1, _) = <(U256, U256, u32)>::abi_decode(&value, false)?;

    Ok((reserve0, reserve1))
}

async fn get_amount_out<M>(
    amount_in: U256,
    reserve_in: U256,
    reserve_out: U256,
    cache_db: &mut CacheDB<EthersDB<M>>,
) -> anyhow::Result<U256>
where
    M: Middleware,
{
    let uniswap_v2_router = address!("7a250d5630b4cf539739df2c5dacb4c659f2488d");
    sol! {
        function getAmountOut(uint amountIn, uint reserveIn, uint reserveOut) external pure returns (uint amountOut);
    }

    let encoded = getAmountOutCall {
        amountIn: amount_in,
        reserveIn: reserve_in,
        reserveOut: reserve_out,
    }.abi_encode();

    let cfg = CfgEnvWithHandlerCfg::new(CfgEnv::default(), HandlerCfg::new(SpecId::CANCUN));
    let env = EnvWithHandlerCfg::new_with_cfg_env(
        cfg,
        BlockEnv{
            ..Default::default()
        },
        TxEnv {
            caller: address!("0000000000000000000000000000000000000000"),
            transact_to: TxKind::Call(uniswap_v2_router),
            data: encoded.into(),
            value: U256::ZERO,
            ..Default::default()
        },
    );

    let mut evm = revm::Evm::builder()
        .with_db(cache_db)
        .with_env_with_handler_cfg(env)
        .build();

    let res = evm.transact().unwrap();

    // let ref_tx = evm.replay().unwrap();
    // let result = ref_tx.result;

    let result = res.result;

    let value = match result {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => value,
        result => return Err(anyhow!("'getAmountOut' execution failed: {result:?}")),
    };

    let amount_out = <U256>::abi_decode(&value, false)?;

    Ok(amount_out)
}

// fn swap<M>(
//     from: Address,
//     pool_address: Address,
//     target: Address,
//     amount_out: U256,
//     is_token0: bool,
//     cache_db: &mut CacheDB<EthersDB<M>>,
// ) -> anyhow::Result<()>
// where
//     M: Middleware,
// {
//     sol! {
//         function swap(uint amount0Out, uint amount1Out, address target, bytes callback) external;
//     }
//
//     let amount0_out = if is_token0 { amount_out } else { U256::from(0) };
//     let amount1_out = if is_token0 { U256::from(0) } else { amount_out };
//
//     let encoded = swapCall {
//         amount0Out: amount0_out,
//         amount1Out: amount1_out,
//         target,
//         callback: Bytes::new(),
//     }
//         .abi_encode();
//
//     // let cfg = CfgEnvWithHandlerCfg::new(CfgEnv::default(), HandlerCfg::new(SpecId::CANCUN));
//     // let env = EnvWithHandlerCfg::new_with_cfg_env(
//     //     cfg,
//     //     BlockEnv{
//     //         ..Default::default()
//     //     },
//     //     TxEnv {
//     //         caller: from,
//     //         transact_to: TxKind::Call(pool_address),
//     //         data: encoded.into(),
//     //         value: U256::ZERO,
//     //         nonce: Some(1),
//     //         ..Default::default()
//     //     },
//     // );
//
//     let mut evm = build_evm(cache_db);
//     evm.context.evm.env.tx.transact_to = TransactTo::Call(pool_address);
//     evm.context.evm.env.tx.data = encoded.into();
//     evm.context.evm.env.tx.nonce = Some(1);
//     evm.context.evm.env.tx.caller = from;
//
//     // let mut evm = revm::Evm::builder()
//     //     .with_db(cache_db)
//     //     .with_env_with_handler_cfg(env)
//     //     .build();
//
//     let res = evm.transact_commit().unwrap();
//
//     // let ref_tx = evm.replay().unwrap();
//     // let result = ref_tx.result;
//
//     let ref_tx = res;
//
//     match ref_tx {
//         ExecutionResult::Success { .. } => {}
//         result => return Err(anyhow!("'swap' execution failed: {result:?}")),
//     };
//
//     Ok(())
// }