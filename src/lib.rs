use hex::{decode, encode};
use near_sdk::{
    env::{self, block_timestamp},
    near, require,
    store::{IterableMap, IterableSet},
    AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseError,
};

use dcap_qvl::{verify, QuoteCollateralV3};

mod collateral;
mod traits;
use traits::{ext_self, ext_voting, ProposalId, SelfCallbacks};

// Governance constants
const GAS_FOR_GOVERNANCE: Gas = Gas::from_tgas(50);
const GAS_FOR_CALLBACK: Gas = Gas::from_tgas(30);
const YOCTO_DEPOSIT: NearToken = NearToken::from_yoctonear(1);
const VOTING_CONTRACT: &str = "shade.ballotbox.testnet";

#[near(serializers = [json, borsh])]
#[derive(Clone)]
pub struct Worker {
    checksum: String,
    codehash: String,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    pub owner_id: AccountId,
    pub approved_codehashes: IterableSet<String>,
    pub worker_by_account_id: IterableMap<AccountId, Worker>,
}

#[near]
impl Contract {
    #[init]
    #[private]
    pub fn init(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            approved_codehashes: IterableSet::new(b"a"),
            worker_by_account_id: IterableMap::new(b"b"),
        }
    }

    // Owner management functions

    pub fn approve_codehash(&mut self, codehash: String) {
        self.require_owner();
        self.approved_codehashes.insert(codehash);
    }

    // Agent registration with full attestation verification
    // register args see: https://github.com/mattlockyer/based-agent-template/blob/main/pages/api/register.js

    pub fn register_agent(
        &mut self,
        quote_hex: String,
        collateral: String,
        checksum: String,
        tcb_info: String,
    ) -> bool {
        let collateral = collateral::get_collateral(collateral);
        let quote = decode(quote_hex).unwrap();
        let now = block_timestamp() / 1000000000;
        let result = verify::verify(&quote, &collateral, now).expect("report is not verified");
        let report = result.report.as_td10().unwrap();
        let report_data = format!("{}", String::from_utf8_lossy(&report.report_data));

        // verify the predecessor matches the report data
        require!(
            env::predecessor_account_id() == report_data,
            format!("predecessor_account_id != report_data: {}", report_data)
        );

        let rtmr3 = encode(report.rt_mr3.to_vec());
        let (shade_agent_api_image, shade_agent_app_image) =
            collateral::verify_codehash(tcb_info, rtmr3);

        // verify the code hashes are approved
        require!(self.approved_codehashes.contains(&shade_agent_api_image));
        require!(self.approved_codehashes.contains(&shade_agent_app_image));

        let predecessor = env::predecessor_account_id();
        self.worker_by_account_id.insert(
            predecessor,
            Worker {
                checksum,
                codehash: shade_agent_app_image,
            },
        );

        true
    }

    // Governance functions

    pub fn approve_proposal(&mut self, proposal_id: ProposalId, voting_start_time_sec: Option<u32>) -> Promise {
        self.require_approved_codehash();

        env::log_str(&format!("🤖 PROXY: Agent approving proposal {}", proposal_id));

        // Contract pays deposit from its own balance
        ext_voting::ext(VOTING_CONTRACT.parse().unwrap())
            .with_static_gas(GAS_FOR_GOVERNANCE)
            .with_attached_deposit(YOCTO_DEPOSIT)
            .approve_proposal(proposal_id, voting_start_time_sec)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_CALLBACK)
                    .governance_callback(proposal_id)
            )
    }

    // View functions

    pub fn get_agent(&self, account_id: AccountId) -> Worker {
        self.worker_by_account_id
            .get(&account_id)
            .expect("no worker found")
            .to_owned()
    }

    pub fn get_contract_balance(&self) -> NearToken {
        env::account_balance()
    }

    // Access control helpers

    fn require_owner(&mut self) {
        require!(env::predecessor_account_id() == self.owner_id);
    }

    fn require_approved_codehash(&mut self) {
        let worker = self.get_agent(env::predecessor_account_id());
        require!(self.approved_codehashes.contains(&worker.codehash));
    }
}

// Implement the callback trait
#[near]
impl SelfCallbacks for Contract {
    #[private]
    fn governance_callback(&mut self, proposal_id: ProposalId, #[callback_result] result: Result<serde_json::Value, PromiseError>) {
        match result {
            Ok(_proposal_info) => {
                env::log_str(&format!("✅ PROXY: Successfully approved proposal {}", proposal_id));
            }
            Err(e) => {
                env::log_str(&format!("❌ PROXY: Failed to approve proposal {}: {:?}", proposal_id, e));
            }
        }
    }
}