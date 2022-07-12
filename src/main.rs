// Copyright Rivtower Technologies LLC.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[cfg(all(feature = "evm", feature = "chaincode"))]
compile_error!("features `evm` and `chaincode` are mutually exclusive");

#[cfg(all(feature = "crypto_sm", feature = "crypto_eth"))]
compile_error!("features `crypto_sm` and `crypto_eth` are mutually exclusive");

mod client;
mod crypto;
mod display;
mod proto;
mod utils;
mod wallet;
mod dSpace;
mod dSpace_grpc;

extern crate grpc;
use futures::executor;

use std::thread;
use anyhow::anyhow;
use anyhow::Result;

use client::Client;
use display::Display as _;
use utils::hex;
use wallet::Wallet;

/// Store action target address
pub const STORE_ADDRESS: &str = "0xffffffffffffffffffffffffffffffffff010000";
/// StoreAbi action target address
pub const ABI_ADDRESS: &str = "0xffffffffffffffffffffffffffffffffff010001";
/// Amend action target address
pub const AMEND_ADDRESS: &str = "0xffffffffffffffffffffffffffffffffff010002";

/// amend the abi data
pub const AMEND_ABI: &str = "0x01";
/// amend the account code
pub const AMEND_CODE: &str = "0x02";
/// amend the kv of db
pub const AMEND_KV_H256: &str = "0x03";
/// amend account balance
pub const AMEND_BALANCE: &str = "0x05";

struct MyDSpace {
    client: Client,
    wallet: Wallet,
}

#[tonic::async_trait]
impl dSpace_grpc::DSpaceService for MyDSpace {
    fn call(&self, _o: grpc::ServerHandlerContext, req: grpc::ServerRequestSingle<dSpace::CallRequest>, resp: grpc::ServerResponseUnarySink<dSpace::CallResponse>) -> grpc::Result<()> {
        let to = utils::parse_addr(req.message.get_To()).unwrap();
        let data = utils::parse_data(req.message.get_Data()).unwrap();

        let mut my_res = dSpace::CallResponse::new();
        let res = executor::block_on(call(&self.client, Vec::new(), to, data));
        my_res.set_Response(res);

        resp.finish(my_res)
    }
    fn send(&self, _o: grpc::ServerHandlerContext, req: grpc::ServerRequestSingle<dSpace::SendRequest>, resp: grpc::ServerResponseUnarySink<dSpace::SendResponse>) -> grpc::Result<()> {
        let to = utils::parse_addr(req.message.get_To()).unwrap();
        let data = utils::parse_data(req.message.get_Data()).unwrap();
        let value = utils::parse_value("").unwrap();

        let mut my_res = dSpace::SendResponse::new();
        let res = executor::block_on(send(&self.client, to, data, value));
        my_res.set_Response(res);

        resp.finish(my_res)
    }
    fn receipt(&self, _o: grpc::ServerHandlerContext, req: grpc::ServerRequestSingle<dSpace::ReceiptRequest>, resp: grpc::ServerResponseUnarySink<dSpace::ReceiptResponse>) -> grpc::Result<()> {
        let tx_hash = utils::parse_value(req.message.get_TxHash()).unwrap();

        let mut my_res = dSpace::ReceiptResponse::new();
        let res = executor::block_on(receipt(&self.client, tx_hash));
        my_res.set_Response(res);

        resp.finish(my_res)
    }
    fn create_account(&self, _o: grpc::ServerHandlerContext, req: grpc::ServerRequestSingle<dSpace::CreateAccountRequest>, resp: grpc::ServerResponseUnarySink<dSpace::CreateAccountResponse>) -> grpc::Result<()> {
        let mut my_res = dSpace::CreateAccountResponse::new();
        let res = create_account(&self.wallet, req.message.get_Id());
        my_res.set_Response(res);

        resp.finish(my_res)
    }
}

fn main() -> Result<()> {
    let user = std::env::var("CITA_CLOUD_DEFAULT_USER").ok();

    let rpc_addr = {
        if let Ok(rpc_addr) = std::env::var("CITA_CLOUD_RPC_ADDR") {
            rpc_addr
        } else {
            "localhost:30004".to_string()
        }
    };
    let executor_addr = {
        if let Ok(executor_addr) = std::env::var("CITA_CLOUD_EXECUTOR_ADDR") {
            executor_addr
        } else {
            "localhost:30005".to_string()
        }
    };

    let wallet = {
        let data_dir = {
            let home = home::home_dir().expect("cannot find home dir");
            home.join(".cloud-cli")
        };
        Wallet::open(data_dir)
    };

    let account = match user {
        Some(user) => wallet
            .load_account(&user)
            .ok_or_else(|| anyhow!("account not found"))?,
        None => wallet.default_account()?,
    };

    let client = Client::new(account.clone(), &rpc_addr, &executor_addr);

    // grpc
    let mut server = grpc::ServerBuilder::new_plain();
    server.http.set_addr("127.0.0.1:8000").unwrap();
    server.add_service(dSpace_grpc::DSpaceServiceServer::new_service_def(MyDSpace{client, wallet}));
    server.build().unwrap();

    println!("Server is running...");

    loop{
        thread::park();
    }
}

async fn call(client: &Client, from: Vec<u8>, to: Vec<u8>, data: Vec<u8>) -> String {
    let result = client.call(from, to, data).await;
    hex(&result)
}

async fn send(client: &Client, to: Vec<u8>, data: Vec<u8>, value: Vec<u8>) -> String {
    let tx_hash = client.send(to, data, value).await;
    hex(&tx_hash)
}

fn create_account(wallet: &Wallet, user: &str) -> String {
    let addr = wallet.create_account(user);
    hex(&addr)
}

async fn receipt(client: &Client, tx_hash: Vec<u8>) -> String {
    #[cfg(feature = "evm")]
    let receipt = client.get_receipt(tx_hash).await;
    receipt.display()
}
