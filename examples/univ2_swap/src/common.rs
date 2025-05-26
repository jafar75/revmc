
#[allow(dead_code)]
const FIBONACCI_CODE: &[u8] =
    &hex!("5f355f60015b8215601a578181019150909160019003916005565b9150505f5260205ff3");
#[allow(dead_code)]
const FIBONACCI_HASH: [u8; 32] =
    hex!("ab1ad1211002e1ddb8d9a4ef58a902224851f6a0273ee3e87276a8d21e649ce8");

#[allow(dead_code)]
const CONTRACT_HASH: [u8; 32] =
    hex!("8e5dc38fdee980c0675338cf6f572c046544c19bcf862a3f18034eab87f1e618");

#[allow(dead_code)]
const UNIV2_HASH: [u8; 32] =
    hex!("5b83bdbcc56b2e630f2807bbadd2b0c21619108066b92a58de081261089e9ce5");


#[allow(dead_code)]
const USDC_HASH: [u8; 32] =
    hex!("d80d4b7c890cb9d6a4893e6b52bc34b56b25335cb13716e0d1d31383e6b41505");

#[allow(dead_code)]
const WETH_HASH: [u8; 32] =
    hex!("cdfb7d322961af3acae7a8f7ee8b69c205b36f576cc5b077f170c7eb8ecbe3ea");

#[allow(dead_code)]
const OTHER_HASH: [u8; 32] =
    hex!("d0a06b12ac47863b5c7be4185c2deaad1c61557033f56c7d4ea74429cbb25e23");


// 0xd80d4b7c890cb9d6a4893e6b52bc34b56b25335cb13716e0d1d31383e6b41505

pub fn get_contract_bytecode_test() -> String {

    let mut hex_str = std::fs::read_to_string("./contract2.bin");
    if hex_str.is_err() {
        println!("err in open file msg: {:?}", hex_str);
        String::new()
    } else {
        let hex_str = hex_str.unwrap();
        println!("contract bytecode len: {:?}", hex_str.len());
        hex_str
    }

}

pub fn get_uniswap_v2_pair() -> String {

    let mut hex_str = std::fs::read_to_string("./univ2.bin");
    if hex_str.is_err() {
        println!("err in open file msg: {:?}", hex_str);
        String::new()
    } else {
        let hex_str = hex_str.unwrap();
        println!("contract bytecode len: {:?}", hex_str.len());
        hex_str
    }

}

pub fn get_pair(path: &str) -> String {

    let mut hex_str = std::fs::read_to_string(path);
    if hex_str.is_err() {
        println!("err in open file msg: {:?}", hex_str);
        String::new()
    } else {
        let hex_str = hex_str.unwrap();
        println!("contract bytecode len: {:?}", hex_str.len());
        hex_str
    }

}