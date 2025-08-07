use near_sdk::{
    env, near, require,
    store::{IterableMap, IterableSet},
    AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseError,
};

// Import traits from separate file
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

    pub fn approve_codehash(&mut self, codehash: String) {
        self.require_owner();
        self.approved_codehashes.insert(codehash);
    }

    pub fn register_agent(&mut self, codehash: String) -> bool {
        // LOCAL DEV CONTRACT, SKIPPING ATTESTATION CHECKS
        let predecessor = env::predecessor_account_id();
        self.worker_by_account_id.insert(predecessor, Worker { codehash });
        true
    }

    // GOVERNANCE FUNCTION
    pub fn approve_proposal(&mut self, proposal_id: ProposalId, voting_start_time_sec: Option<u32>) -> Promise {
        self.require_approved_codehash();

        env::log_str(&format!("🤖 PROXY: Agent approving proposal {}", proposal_id));

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

    pub fn get_agent(&self, account_id: AccountId) -> Worker {
        self.worker_by_account_id
            .get(&account_id)
            .expect("no worker found")
            .to_owned()
    }

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