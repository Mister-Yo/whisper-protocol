use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, log, near_bindgen, AccountId, BorshStorageKey, NearToken, PanicOnDefault, Promise};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

#[derive(BorshStorageKey, BorshSerialize)]
#[borsh(crate = "near_sdk::borsh")]
enum StorageKey {
    Profiles,
    Groups,
}

/// A registered messaging profile
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct MessagingProfile {
    pub x25519_pubkey: String,
    pub key_version: u32,
    pub registered_at: u64,
    pub display_name: Option<String>,
}

/// Group chat metadata
#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[borsh(crate = "near_sdk::borsh")]
#[serde(crate = "near_sdk::serde")]
pub struct GroupChat {
    pub group_id: String,
    pub creator: AccountId,
    pub created_at: u64,
    pub name: Option<String>,
}

/// NEP-297 event
#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
struct WhisperEvent<'a> {
    standard: &'a str,
    version: &'a str,
    event: &'a str,
    data: serde_json::Value,
}

fn emit_event(event: &str, data: serde_json::Value) {
    let ev = WhisperEvent {
        standard: "whisper",
        version: "1.0.0",
        event,
        data,
    };
    let json = serde_json::to_string(&ev).unwrap();
    log!("EVENT_JSON:{}", json);
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
#[borsh(crate = "near_sdk::borsh")]
pub struct WhisperContract {
    profiles: LookupMap<AccountId, MessagingProfile>,
    groups: LookupMap<String, GroupChat>,
    profile_count: u64,
    message_count: u64,
    owner: AccountId,
}

#[near_bindgen]
impl WhisperContract {
    #[init]
    pub fn new() -> Self {
        assert!(!env::state_exists(), "Already initialized");
        Self {
            profiles: LookupMap::new(StorageKey::Profiles),
            groups: LookupMap::new(StorageKey::Groups),
            profile_count: 0,
            message_count: 0,
            owner: env::predecessor_account_id(),
        }
    }

    // ========================================================================
    // Key Registration
    // ========================================================================

    #[payable]
    pub fn register_key(&mut self, x25519_pubkey: String, display_name: Option<String>) {
        let account_id = env::predecessor_account_id();

        let decoded = BASE64
            .decode(&x25519_pubkey)
            .unwrap_or_else(|_| env::panic_str("Invalid base64 pubkey"));
        assert_eq!(decoded.len(), 32, "X25519 pubkey must be 32 bytes");

        let existing = self.profiles.get(&account_id);
        let key_version = existing.as_ref().map_or(1, |p| p.key_version + 1);

        if existing.is_none() {
            let deposit = env::attached_deposit();
            assert!(
                deposit >= NearToken::from_millinear(10),
                "Attach at least 0.01 NEAR for storage deposit"
            );
            self.profile_count += 1;
        }

        let profile = MessagingProfile {
            x25519_pubkey: x25519_pubkey.clone(),
            key_version,
            registered_at: env::block_timestamp(),
            display_name: display_name.clone(),
        };

        self.profiles.insert(&account_id, &profile);

        emit_event(
            "key_registered",
            serde_json::json!({
                "account_id": account_id.to_string(),
                "x25519_pubkey": x25519_pubkey,
                "key_version": key_version,
                "display_name": display_name,
            }),
        );
    }

    // ========================================================================
    // Messaging (event-based, no storage)
    // ========================================================================

    pub fn send_message(
        &mut self,
        to: AccountId,
        encrypted_body: String,
        nonce: String,
        recipient_key_version: u32,
        reply_to: Option<String>,
    ) {
        let from = env::predecessor_account_id();

        assert!(
            self.profiles.get(&to).is_some(),
            "Recipient has no registered messaging key"
        );

        self.message_count += 1;
        let message_id = self.message_count;

        emit_event(
            "message",
            serde_json::json!({
                "id": message_id,
                "from": from.to_string(),
                "to": to.to_string(),
                "encrypted_body": encrypted_body,
                "nonce": nonce,
                "recipient_key_version": recipient_key_version,
                "reply_to": reply_to,
                "timestamp": env::block_timestamp(),
            }),
        );
    }

    #[payable]
    pub fn send_message_with_payment(
        &mut self,
        to: AccountId,
        encrypted_body: String,
        nonce: String,
        recipient_key_version: u32,
        reply_to: Option<String>,
    ) -> Promise {
        let from = env::predecessor_account_id();
        let amount = env::attached_deposit();

        assert!(
            amount > NearToken::from_yoctonear(0),
            "Must attach NEAR tokens for payment message"
        );
        assert!(
            self.profiles.get(&to).is_some(),
            "Recipient has no registered messaging key"
        );

        self.message_count += 1;
        let message_id = self.message_count;

        emit_event(
            "message",
            serde_json::json!({
                "id": message_id,
                "from": from.to_string(),
                "to": to.to_string(),
                "encrypted_body": encrypted_body,
                "nonce": nonce,
                "recipient_key_version": recipient_key_version,
                "reply_to": reply_to,
                "timestamp": env::block_timestamp(),
                "payment": {
                    "token": "NEAR",
                    "amount": amount.as_yoctonear().to_string(),
                }
            }),
        );

        Promise::new(to).transfer(amount)
    }

    // ========================================================================
    // Group Chats
    // ========================================================================

    #[payable]
    pub fn create_group(
        &mut self,
        group_id: String,
        name: Option<String>,
        member_keys: String,
    ) {
        let creator = env::predecessor_account_id();
        let deposit = env::attached_deposit();

        assert!(
            deposit >= NearToken::from_millinear(10),
            "Attach at least 0.01 NEAR for storage"
        );
        assert!(
            self.groups.get(&group_id).is_none(),
            "Group ID already exists"
        );

        let group = GroupChat {
            group_id: group_id.clone(),
            creator: creator.clone(),
            created_at: env::block_timestamp(),
            name: name.clone(),
        };

        self.groups.insert(&group_id, &group);

        emit_event(
            "group_created",
            serde_json::json!({
                "group_id": group_id,
                "creator": creator.to_string(),
                "name": name,
                "member_keys": member_keys,
                "timestamp": env::block_timestamp(),
            }),
        );
    }

    pub fn send_group_message(
        &mut self,
        group_id: String,
        encrypted_body: String,
        nonce: String,
        group_key_version: u32,
    ) {
        let from = env::predecessor_account_id();

        assert!(
            self.groups.get(&group_id).is_some(),
            "Group does not exist"
        );

        self.message_count += 1;
        let message_id = self.message_count;

        emit_event(
            "group_message",
            serde_json::json!({
                "id": message_id,
                "group_id": group_id,
                "from": from.to_string(),
                "encrypted_body": encrypted_body,
                "nonce": nonce,
                "group_key_version": group_key_version,
                "timestamp": env::block_timestamp(),
            }),
        );
    }

    // ========================================================================
    // View Methods
    // ========================================================================

    pub fn get_profile(&self, account_id: AccountId) -> Option<MessagingProfile> {
        self.profiles.get(&account_id)
    }

    pub fn has_profile(&self, account_id: AccountId) -> bool {
        self.profiles.get(&account_id).is_some()
    }

    pub fn get_group(&self, group_id: String) -> Option<GroupChat> {
        self.groups.get(&group_id)
    }

    pub fn get_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "profile_count": self.profile_count,
            "message_count": self.message_count,
            "owner": self.owner.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    fn get_context(predecessor: &str) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(predecessor.parse().unwrap());
        builder.attached_deposit(NearToken::from_millinear(100));
        builder
    }

    #[test]
    fn test_register_key() {
        let context = get_context("alice.near");
        testing_env!(context.build());

        let mut contract = WhisperContract::new();
        let pubkey = BASE64.encode([1u8; 32]);
        contract.register_key(pubkey.clone(), Some("Alice".to_string()));

        let profile = contract.get_profile("alice.near".parse().unwrap()).unwrap();
        assert_eq!(profile.x25519_pubkey, pubkey);
        assert_eq!(profile.key_version, 1);
        assert_eq!(contract.profile_count, 1);
    }

    #[test]
    fn test_rotate_key() {
        let context = get_context("alice.near");
        testing_env!(context.build());

        let mut contract = WhisperContract::new();
        let pubkey1 = BASE64.encode([1u8; 32]);
        contract.register_key(pubkey1, None);

        let pubkey2 = BASE64.encode([2u8; 32]);
        contract.register_key(pubkey2.clone(), None);

        let profile = contract.get_profile("alice.near".parse().unwrap()).unwrap();
        assert_eq!(profile.x25519_pubkey, pubkey2);
        assert_eq!(profile.key_version, 2);
        assert_eq!(contract.profile_count, 1);
    }

    #[test]
    fn test_send_message() {
        let context = get_context("alice.near");
        testing_env!(context.build());

        let mut contract = WhisperContract::new();
        contract.register_key(BASE64.encode([1u8; 32]), None);

        let context_bob = get_context("bob.near");
        testing_env!(context_bob.build());
        contract.register_key(BASE64.encode([2u8; 32]), None);

        let context_alice = get_context("alice.near");
        testing_env!(context_alice.build());
        contract.send_message(
            "bob.near".parse().unwrap(),
            "encrypted".to_string(),
            "nonce".to_string(),
            1,
            None,
        );
        assert_eq!(contract.message_count, 1);
    }

    #[test]
    #[should_panic(expected = "Recipient has no registered messaging key")]
    fn test_send_to_unregistered() {
        let context = get_context("alice.near");
        testing_env!(context.build());

        let mut contract = WhisperContract::new();
        contract.register_key(BASE64.encode([1u8; 32]), None);

        contract.send_message(
            "nobody.near".parse().unwrap(),
            "data".to_string(),
            "nonce".to_string(),
            1,
            None,
        );
    }

    #[test]
    fn test_create_group() {
        let context = get_context("alice.near");
        testing_env!(context.build());

        let mut contract = WhisperContract::new();
        contract.create_group(
            "test-group".to_string(),
            Some("Test".to_string()),
            "{}".to_string(),
        );

        let group = contract.get_group("test-group".to_string()).unwrap();
        assert_eq!(group.creator.to_string(), "alice.near");
    }

    #[test]
    fn test_stats() {
        let context = get_context("alice.near");
        testing_env!(context.build());
        let contract = WhisperContract::new();
        assert_eq!(contract.get_stats()["profile_count"], 0);
    }
}
