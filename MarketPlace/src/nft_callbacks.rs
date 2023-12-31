use crate::*;
/// approval callbacks from NFT Contracts
#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct MarketArgs {
    pub market_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<U128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ft_token_id: Option<AccountId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer_id: Option<AccountId>, // offer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_price: Option<U128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<U64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<U64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_auction: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_nft_contract_id: Option<AccountId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_token_id: Option<TokenId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_token_series_id: Option<TokenSeriesId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer_nft_contract_id: Option<AccountId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buyer_token_id: Option<TokenId>,
}

trait NonFungibleTokenApprovalsReceiver {
    fn nft_on_approve(
        &mut self,
        token_id: TokenId,
        owner_id: AccountId,
        approval_id: u64,
        msg: String,
    );
}

#[near_bindgen]
impl NonFungibleTokenApprovalsReceiver for Contract {
    fn nft_on_approve(
        &mut self,
        token_id: TokenId,
        owner_id: AccountId,
        approval_id: u64,
        msg: String,
    ) {
        // enforce cross contract call and owner_id is signer

        let nft_contract_id = env::predecessor_account_id();
        let signer_id = env::signer_account_id();
        assert_ne!(
            env::current_account_id(), nft_contract_id,
            "nft_on_approve should only be called via cross-contract call"
        );
        assert_eq!(owner_id, signer_id, "owner_id should be signer_id");

        assert!(
            self.approved_nft_contract_ids.contains(&nft_contract_id),
            "nft_contract_id is not approved"
        );

        let MarketArgs {
            market_type,
            price,
            ft_token_id,
            buyer_id,
            started_at,
            ended_at,
            end_price,
            is_auction,
            seller_nft_contract_id,
            seller_token_id,
            seller_token_series_id,
            buyer_nft_contract_id,
            buyer_token_id
        } = near_sdk::serde_json::from_str(&msg).expect("Not valid MarketArgs");

        // replace old approval id on trade
        let buyer_contract_account_id_token_id = make_triple(&nft_contract_id,
                                                             &owner_id,
                                                             &token_id);
        if let Some(mut old_trade) = self.trades.get(&buyer_contract_account_id_token_id){
            old_trade.approval_id = approval_id;
            self.trades.insert(&buyer_contract_account_id_token_id,&old_trade);
        }
        // replace old approval on market
        let contract_and_token_id = format!("{}{}{}", nft_contract_id, DELIMETER, token_id);
        if let Some(mut old_market) = self.market.get(&contract_and_token_id){
            old_market.approval_id = approval_id;
            self.market.insert(&contract_and_token_id,&old_market);
        }

        if market_type == "sale" {
            assert!(price.is_some(), "price not specified");

            let storage_amount = self.storage_minimum_balance().0;
            let owner_paid_storage = self.storage_deposits.get(&signer_id).unwrap_or(0);
            let signer_storage_required =
                (self.get_supply_by_owner_id(signer_id).0 + 1) as u128 * storage_amount;

            if owner_paid_storage < signer_storage_required {
                let notif=format!("Insufficient storage paid: {}, for {} sales at {} rate of per sale",
                owner_paid_storage,
                signer_storage_required / storage_amount,
                storage_amount
                );
                env::log_str(&notif);
                return;
            }
            
            self.internal_delete_market_data(&nft_contract_id, &token_id);

            let ft_token_id_res = ft_token_id.unwrap_or(near_account());

            if self.approved_ft_token_ids.contains(&ft_token_id_res) != true {
                env::panic_str(&"ft_token_id not approved");
            }

            self.internal_add_market_data(
                owner_id,
                approval_id,
                nft_contract_id,
                token_id,
                ft_token_id_res,
                price.unwrap(),
                started_at,
                ended_at,
                end_price,
                is_auction,
            );
        } else if market_type == "accept_offer" {
            assert!(buyer_id.is_some(), "Account id is not specified");
            assert!(price.is_some(), "Price is not specified (for check)");

            self.internal_accept_offer(
                nft_contract_id,
                buyer_id.unwrap(),
                token_id,
                owner_id,
                approval_id,
                price.unwrap().0,
            );
        } else if market_type == "accept_offer_paras_series" {
            assert!(buyer_id.is_some(), "Account id is not specified");
            assert!(
                self.paras_nft_contracts.contains(&nft_contract_id),
                "accepting offer series for Paras NFT only"
            );
            assert!(price.is_some(), "Price is not specified (for check)");

            self.internal_accept_offer_series(
                nft_contract_id,
                buyer_id.unwrap(),
                token_id,
                owner_id,
                approval_id,
                price.unwrap().0,
            );
        } else if market_type == "add_trade" {

            let storage_amount = self.storage_minimum_balance().0;
            let owner_paid_storage = self.storage_deposits.get(&signer_id).unwrap_or(0);
            let signer_storage_required =
                (self.get_supply_by_owner_id(signer_id).0 + 1) as u128 * storage_amount;

            if owner_paid_storage < signer_storage_required {
                let notif=format!("Insufficient storage paid: {}, for {} sales at {} rate of per sale",
                                  owner_paid_storage,
                                  signer_storage_required / storage_amount,
                                  storage_amount
                );
                env::log_str(&notif);
                return;
            }

            self.add_trade(
                seller_nft_contract_id.unwrap(),
                seller_token_id,
                seller_token_series_id,
                nft_contract_id,
                owner_id,
                Some(token_id),
                approval_id,
            );
        } else if market_type == "accept_trade" {

            assert!(buyer_id.is_some(), "Account id is not specified");
            assert!(buyer_nft_contract_id.is_some(), "Buyer NFT contract id is not specified");
            assert!(buyer_token_id.is_some(), "Buyer token id is not specified");

            self.internal_accept_trade(
                nft_contract_id,
                buyer_id.unwrap(),
                token_id,
                owner_id,
                approval_id,
                buyer_nft_contract_id.unwrap(),
                buyer_token_id.unwrap()
            );

        } else if market_type == "accept_trade_paras_series" {

            assert!(buyer_id.is_some(), "Account id is not specified");
            assert!(
                self.paras_nft_contracts.contains(&nft_contract_id),
                "accepting offer series for Paras NFT only"
            );
            assert!(buyer_nft_contract_id.is_some(), "Buyer NFT contract id is not specified");
            assert!(buyer_token_id.is_some(), "Buyer token id is not specified");

            self.internal_accept_trade_series(
                nft_contract_id,
                buyer_id.unwrap(),
                token_id,
                owner_id,
                approval_id,
                buyer_nft_contract_id.unwrap(),
                buyer_token_id.unwrap()
            );

        }
    }
}
