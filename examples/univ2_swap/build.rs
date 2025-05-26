use revmc::{
    primitives::{hex, SpecId},
    EvmCompiler, EvmLlvmBackend, OptimizationLevel, Result,
};
use std::path::PathBuf;

include!("./src/common.rs");

fn main() -> Result<()> {
    // Emit the configuration to run compiled bytecodes.
    // This not used if we are only using statically linked bytecodes.
    revmc_build::emit();

    // Compile and statically link a bytecode.
    let name = "univ2_pair";
    let bytecode = hex::decode(get_uniswap_v2_pair()).unwrap();

    let name_usdc = "usdc";
    let bytecode_usdc = hex::decode(get_pair("usdc.bin")).unwrap();

    let name_weth = "weth";
    let bytecode_weth = hex::decode(get_pair("weth.bin")).unwrap();

    let name_other = "other";
    let bytecode_other = hex::decode(get_pair("other.bin")).unwrap();
    // let bytecode = UNIV2_CODE;

    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let context = revmc::llvm::inkwell::context::Context::create();
    let backend = EvmLlvmBackend::new(&context, true, OptimizationLevel::Aggressive)?;
    let mut compiler = EvmCompiler::new(backend);
    compiler.gas_metering(false);
    unsafe { compiler.stack_bound_checks(false); }
    // compiler.frame_pointers(true);
    compiler.set_dump_to(Some("./debug_dir".parse()?));
    compiler.debug_assertions(false);
    // compiler.local_stack(true);
    compiler.translate(name, &bytecode, SpecId::CANCUN)?;
    compiler.translate(name_usdc, &bytecode_usdc, SpecId::CANCUN)?;
    compiler.translate(name_weth, &bytecode_weth, SpecId::CANCUN)?;
    compiler.translate(name_other, &bytecode_other, SpecId::CANCUN)?;

    let object = out_dir.join(name).with_extension("o");
    compiler.write_object_to_file(&object)?;

    cc::Build::new().object(&object).static_flag(true).compile(name);
    // cc::Build::new().object(&object).shared_flag(true).compile(name);

    Ok(())
}
