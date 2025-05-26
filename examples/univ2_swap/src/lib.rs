//! Minimal, `no_std` runner.

extern crate alloc;
extern crate core;

use alloc::string::String;
// This dependency is needed to define the necessary symbols used by the compiled bytecodes,
// but we don't use it directly, so silence the unused crate dependency warning.
use revmc_builtins as _;

use alloc::sync::Arc;
use core::str::FromStr;
use alloy::sol_types::SolValue;
use alloy_primitives::{address, keccak256, StorageKey};
use ethers_providers::Middleware;
use revm::{
    handler::register::EvmHandler,
    primitives::{hex, B256},
    Database,
};
use revm::db::{CacheDB, EthersDB};
use revm::primitives::{AccountInfo, Address, Bytecode, KECCAK_EMPTY, U256};
use tracing::info;
use revmc_context::EvmCompilerFn;

include!("./common.rs");

// The bytecode we statically linked.
revmc_context::extern_revmc! {
    fn univ2_pair;
    fn usdc;
    fn weth;
    fn other;
}

/// Build a [`revm::Evm`] with a custom handler that can call compiled functions.
pub fn build_evm<'a, DB: Database + 'static>(db: DB) -> revm::Evm<'a, ExternalContext, DB> {
    revm::Evm::builder()
        .with_db(db)
        .with_external_context(ExternalContext::new())
        .append_handler_register(register_handler)
        .build()
}


pub struct ExternalContext;

impl ExternalContext {
    fn new() -> Self {
        Self
    }

    fn get_function(&self, bytecode_hash: B256) -> Option<EvmCompilerFn> {
        // Can use any mapping between bytecode hash and function.
        if bytecode_hash == UNIV2_HASH {
            return Some(EvmCompilerFn::new(univ2_pair));
        }

        if bytecode_hash == USDC_HASH {
            return Some(EvmCompilerFn::new(usdc));
        }

        if bytecode_hash == WETH_HASH {
            return Some(EvmCompilerFn::new(weth));
        }

        if bytecode_hash == OTHER_HASH {
            return Some(EvmCompilerFn::new(other));
        }

        None
    }
}

// This `+ 'static` bound is only necessary here because of an internal cfg feature.
fn register_handler<DB: Database + 'static>(handler: &mut EvmHandler<'_, ExternalContext, DB>) {
    let prev = handler.execution.execute_frame.clone();
    handler.execution.execute_frame = Arc::new(move |frame, memory, tables, context| {
        let interpreter = frame.interpreter_mut();
        let bytecode_hash = interpreter.contract.hash.unwrap_or_default();
        // println!("bytecode hash: {:?}", bytecode_hash);
        if let Some(f) = context.external.get_function(bytecode_hash) {
            // println!("get function");
            Ok(unsafe { f.call_with_interpreter_and_memory(interpreter, memory, context) })
        } else {
            // println!("prev");
            prev(frame, memory, tables, context)
        }
    });
}


////////////////////////
///////////////////////




pub fn deploy_and_commit<M>(mut cache_db: &mut CacheDB<EthersDB<M>>, bytecode: &str)
where
    M: Middleware,
{
    let account = address!("18B06aaF27d44B756FCF16Ca20C1f183EB49111f");

    let weth = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let usdc_weth_pair = address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc");

    let weth_balance_slot = U256::from(3);

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
}
