use actix_web::{get, App, HttpResponse, HttpServer, Responder, post, web};
use comfy_table::{Table, Cell, Attribute, presets, ContentArrangement};
use miden_client::{accounts::{AccountStorageMode, AccountTemplate, AccountId}, config::{RpcConfig, Endpoint}, store::{sqlite_store::{SqliteStore, config::SqliteStoreConfig}, StoreAuthenticator, InputNoteRecord, NoteFilter}, crypto::{RpoRandomCoin, FeltRng}, Felt, transactions::{LocalTransactionProver, TransactionRequest, PaymentTransactionData}, Client, rpc::TonicRpcClient, notes::{NoteFile, NoteType, NoteConsumability}, utils::Deserializable, assets::FungibleAsset};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use actix_cors::Cors;
use std::{env, sync::Arc, path::PathBuf, fs::File, io::Read};

pub const CLIENT_BINARY_NAME: &str = "miden";
pub const FAUCET_ID: &str = "0x29b86f9443ad907a";


#[derive(Deserialize)]
struct Transfer {
    sender_wallet: String,
    target_wallet: String,
    amount: u64,
}

#[derive(Deserialize)]
struct BatchTransfer {
    transfers: Vec<Transfer>,
}

#[derive(Serialize)]
struct TransferResult {
    sender_wallet: String,
    target_wallet: String,
    amount: u64,
    tx_id: Option<String>,
    error: Option<String>,
}


fn get_rpc_config() -> RpcConfig {
    // Extract protocol, host, and port from the URL
    let protocol = "http".to_string(); // The protocol part (http or https)
    let host = "18.203.155.106".to_string(); // The host part
    let port = 57291; // The port number

    let endpoint = Endpoint::new(protocol, host, port); // Construct the Endpoint
    let timeout_ms = 30_000; // Timeout in milliseconds

    RpcConfig {
        endpoint,
        timeout_ms,
    }
}


async fn load_client() -> Client<RpoRandomCoin> {
    let rpc_config = get_rpc_config();
    let store_config = SqliteStoreConfig {
        database_filepath: "./store.sqlite3".to_string(),
    };

    let store = {
        let sqlite_store = SqliteStore::new(&store_config).await.unwrap();
        std::sync::Arc::new(sqlite_store)
    };

    let mut rng = rand::thread_rng();
    let coin_seed: [u64; 4] = rng.gen();

    let rng = RpoRandomCoin::new(coin_seed.map(Felt::new));
    let tx_prover = Arc::new(LocalTransactionProver::default());
    let authenticator = StoreAuthenticator::new_with_rng(store.clone(), rng);

    Client::new(
        Box::new(TonicRpcClient::new(&rpc_config)),
        rng,
        store,
        Arc::new(authenticator),
        tx_prover,
        false,
    )
}


async fn create_new_wallet(storage_type_mode: AccountStorageMode) -> String {
    println!("Creating new wallet...");
    let mut client = load_client().await;
    let client_template = AccountTemplate::BasicWallet { 
        mutable_code: false, 
        storage_mode: storage_type_mode
    };

    let (account, _seed) = client.new_account(client_template).await.unwrap();
    println!("Succesfully created new wallet. {:?}", account);
    account.id().to_hex() // return account id
}


pub async fn _import_note_sync(note_file_path: PathBuf) {
    println!("Current Path: {:?}", env::current_dir());
    println!("filepath: {:?}", note_file_path);
    // Create a new Tokio runtime
    // let rt = Runtime::new()?;

    // Block on the asynchronous operations
    // rt.block_on(async {
        let mut client = load_client().await;
        let note_file = read_note_file(note_file_path);
        let _note = client.import_note(note_file).await;

        // If the transaction is successful, print the result
        println!("Transaction result: {:?}", _note);
        // Ok(())
// })
}

fn read_note_file(filename: PathBuf) -> NoteFile {
    let mut contents = vec![];
    let mut file = File::open(&filename).expect("Failed to open file");
    file.read_to_end(&mut contents).expect("Failed to read file");
    NoteFile::read_from_bytes(&contents).expect("Failed to parse NoteFile")
}

/// Function to import multiple notes
pub async fn import_multiple_notes(note_file_paths: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    for note_file_path in note_file_paths {
        let note_path_buf = PathBuf::from(note_file_path);
        let _ = _import_note_sync(note_path_buf).await;
    }
    Ok(())
}


async fn _consume_notes_(account_id: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut client = load_client().await;
    let _ = client.sync_state().await;
    let input_notes = client.get_input_notes(NoteFilter::All).await.unwrap();

    let account_id = AccountId::from_hex(account_id).unwrap();
    print!("Account ID: {:?}", account_id);
    let tx_request: TransactionRequest = TransactionRequest::consume_notes(vec![input_notes[0].id()]);
    let transaction_execution_result = client.new_transaction(account_id, tx_request).await.unwrap();
    let tx_id = transaction_execution_result.executed_transaction().id();
    println!("note consumptions tx_id: {:?}", tx_id);
    Ok(tx_id.to_string())
}

async fn consume_available_notes_for_user(account_id: &str) -> Result<String, String> {
    let mut client = load_client().await;
    let _ = client.sync_state().await;
    let mut list_of_notes = Vec::new();
    
    // Optionally specify an account_id; use None for all notes
    let account_id_hex = AccountId::from_hex(account_id).unwrap();
    let account_id: Option<AccountId> = Some(AccountId::from_hex(account_id).unwrap()); // Replace with actual AccountId

    let notes = client.get_consumable_notes(account_id).await?;
    print_consumable_notes_summary(&notes)?;

    list_of_notes.extend(notes.iter().map(|(note, _)| note.id()));
    let transaction_request = TransactionRequest::consume_notes(list_of_notes);
    let transaction_id = execute_transaction(&mut client, account_id_hex, transaction_request).await.unwrap();

    Ok(transaction_id)
}


async fn execute_transaction(
    client: &mut Client<impl FeltRng>,
    account_id: AccountId,
    transaction_request: TransactionRequest,
) -> Result<String, String> {
    println!("Executing transaction...");
    let transaction_execution_result =
        client.new_transaction(account_id, transaction_request).await?;


    println!("Proving transaction and then submitting it to node...");

    let transaction_id = transaction_execution_result.executed_transaction().id();
    let output_notes = transaction_execution_result
        .created_notes()
        .iter()
        .map(|note| note.id())
        .collect::<Vec<_>>();

    client.submit_transaction(transaction_execution_result).await?;

    println!("Succesfully created transaction.");
    println!("Transaction ID: {}", transaction_id);

    if output_notes.is_empty() {
        println!("The transaction did not generate any output notes.");
    } else {
        println!("Output notes:");
        output_notes.iter().for_each(|note_id| println!("\t- {}", note_id));
    }

    Ok(transaction_id.to_string())
}


/**
 * Here is an example for a pay-to-id transaction type:
 */
pub async fn transfer_asset (sender_account_id: AccountId, target_account_id: AccountId, amount: u64) -> Result<String, Box<dyn std::error::Error>> {
    let mut client = load_client().await;
    let faucet_id = AccountId::from_hex(FAUCET_ID)?;
    let fungible_asset: FungibleAsset = FungibleAsset::new(faucet_id, amount)?.into();

    let payment_transaction = PaymentTransactionData::new(
        vec![fungible_asset.into()],
        sender_account_id,
        target_account_id,
    );

    let transaction_request = TransactionRequest::pay_to_id(
        payment_transaction,
        None,
        NoteType::Private,
        client.rng(),
    )?;

    let tx_id = execute_transaction(&mut client, sender_account_id, transaction_request).await.unwrap();
    let _ = client.sync_state().await;

    Ok(tx_id)
}


fn print_consumable_notes_summary<'a, I>(notes: I) -> Result<(), String>
where
    I: IntoIterator<Item = &'a (InputNoteRecord, Vec<NoteConsumability>)>,
{
    let mut table = create_dynamic_table(&["Note ID", "Account ID", "Relevance"]);

    for (note, relevances) in notes {
        for relevance in relevances {
            table.add_row(vec![
                note.id().to_hex(),
                relevance.0.to_string(),
                relevance.1.to_string(),
            ]);
        }
    }

    println!("{table}");

    Ok(())
}


async fn list_notes(account_id: String) -> Result<(), String> {
    let client = load_client().await;
    let account_id: Option<String> = Some(String::from(account_id));

    let account_id = match account_id {
        Some(id) => Some(AccountId::from_hex(id.as_str()).map_err(|err| err.to_string())?),
        None => None,
    };
    let notes = client.get_consumable_notes(account_id).await?;
    print_consumable_notes_summary(&notes)?;
    Ok(())
}



pub async fn print_account_table() -> Result<u64, Box<dyn std::error::Error>> {
    let mut client = load_client().await; 
    let _ = client.sync_state().await;

    let account_id: &str = "0x8c4b6a13872cb095";
    let account_id = AccountId::from_hex(account_id).unwrap();
    
    let (account, _) = client.get_account(account_id).await.unwrap();
    let faucet_id = AccountId::from_hex(FAUCET_ID)?;
    let balance = account.vault().get_balance(faucet_id).unwrap();
    
    let mut table = create_dynamic_table(&["Account ID", "Storage Mode", "Nonce", "Amount"]);
    let accounts = client.get_account_headers().await.unwrap();

    for (acc, _acc_seed) in accounts.iter() {
        let (account_temp, _) = client.get_account(acc.id()).await.unwrap();
        table.add_row(vec![
            acc.id().to_string(),
            acc.id().storage_mode().to_string(),
            acc.nonce().as_int().to_string(),
            account_temp.vault().get_balance(faucet_id).unwrap().to_string(),
        ]);
    }

    println!("{table}");

    Ok(balance)
}


pub async fn get_account_table() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut client = load_client().await;
    let _ = client.sync_state().await;

    let faucet_id = AccountId::from_hex(FAUCET_ID)?;
    let accounts = client.get_account_headers().await.unwrap();

    // Initialize a JSON array to store account details
    let mut account_details = Vec::new();

    for (index, (acc, _acc_seed)) in accounts.iter().enumerate() {
        let (account_temp, _) = client.get_account(acc.id()).await.unwrap();
        let balance = account_temp.vault().get_balance(faucet_id).unwrap();

        // Add account details to the JSON array
        account_details.push(json!({
            "index": index,
            "account_id": acc.id().to_string(),
            "balance": balance,
            "faucet": FAUCET_ID,
        }));
    }

    // Return the account details as a JSON array
    let result = json!({ "accounts": account_details });

    Ok(result)
}

pub fn create_dynamic_table(headers: &[&str]) -> Table {
    let header_cells = headers
        .iter()
        .map(|header| Cell::new(header).add_attribute(Attribute::Bold))
        .collect::<Vec<_>>();

    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_FULL)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth)
        .set_header(header_cells);

    table
}


#[post("/batch-transfer")]
async fn batch_transfer(batch: web::Json<BatchTransfer>) -> impl Responder {
    let mut results = Vec::new();

    for single_transfer in &batch.transfers {
        let sender_wallet = single_transfer.sender_wallet.clone();
        let target_wallet = single_transfer.target_wallet.clone();
        let amount = single_transfer.amount;

        let sender_account = match AccountId::from_hex(&sender_wallet) {
            Ok(account) => account,
            Err(e) => {
                results.push(TransferResult {
                    sender_wallet,
                    target_wallet,
                    amount,
                    tx_id: None,
                    error: Some(format!("Invalid sender wallet: {}", e)),
                });
                continue;
            }
        };

        let target_account = match AccountId::from_hex(&target_wallet) {
            Ok(account) => account,
            Err(e) => {
                results.push(TransferResult {
                    sender_wallet,
                    target_wallet,
                    amount,
                    tx_id: None,
                    error: Some(format!("Invalid target wallet: {}", e)),
                });
                continue;
            }
        };

        match transfer_asset(sender_account, target_account, amount).await {
            Ok(tx_id) => {
                results.push(TransferResult {
                    sender_wallet,
                    target_wallet,
                    amount,
                    tx_id: Some(tx_id),
                    error: None,
                });
            }
            Err(e) => {
                results.push(TransferResult {
                    sender_wallet,
                    target_wallet,
                    amount,
                    tx_id: None,
                    error: Some(format!(
                        "Failed to process transfer: {}",
                        e
                    )),
                });
            }
        }
    }

    HttpResponse::Ok().json(json!({ "results": results }))
}



#[post("/create-wallet")]
async fn create_wallet() -> impl Responder {
    let wallet_id = create_new_wallet(AccountStorageMode::Private).await;
    let response = serde_json::json!({
        "wallet_id": wallet_id,
        "account_type": AccountStorageMode::Private.to_string(),
    });
    HttpResponse::Ok().json(response)
}

#[get("/accounts")]
async fn print_table() -> impl Responder {
    let accounts = get_account_table().await.unwrap();
    HttpResponse::Ok().json(accounts)
}

#[post("/transfer")]
async fn transfer(transfer: web::Json<Transfer>) -> impl Responder {
    let sender_account = AccountId::from_hex(&transfer.sender_wallet).unwrap();
    let target_account = AccountId::from_hex(&transfer.target_wallet).unwrap();
    let amount = transfer.amount;

    let tx_id = transfer_asset(sender_account, target_account, amount).await.unwrap();
    let response = serde_json::json!({
        "tx_id": tx_id,
        "sender_wallet": transfer.sender_wallet,
        "target_wallet": transfer.target_wallet,
        "amount": amount,
    });

    HttpResponse::Ok().json(response)
}

#[get("/{account_id}/get-consumable-notes")]
async fn get_consumable_notes(path: web::Path<String>) -> impl Responder {
    let account_id = path.into_inner();
    let _ = list_notes(account_id).await;
    HttpResponse::Ok().body("Printing consumable notes")
}


#[get("/import-notes")]
async fn import_notes() -> impl Responder {
        
    let note_file_paths = vec![
        "./note_1.mno".to_string(),
    ];

    let _ = import_multiple_notes(note_file_paths).await;
    HttpResponse::Ok().body("Import notes")
}

#[post("/{account_id}/consume-available-notes")]
async fn consume_available_notes(path: web::Path<String>) -> impl Responder {
    let account_id = path.into_inner();
    let tx_ids = consume_available_notes_for_user(account_id.as_str()).await.unwrap();
    let response = serde_json::json!({
        "tx_ids": tx_ids
    });

    HttpResponse::Ok().json(response)
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin() // Allows requests from any origin
                    .allow_any_method() // Allows any HTTP method (GET, POST, etc.)
                    .allow_any_header() // Allows any headers in requests
                    .max_age(3600)      // Cache the preflight response for 1 hour
            )
            .service(batch_transfer)
            .service(import_notes)
            .service(print_table)
            .service(transfer)
            .service(consume_available_notes)
            .service(get_consumable_notes)
            .service(create_wallet)
            
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}