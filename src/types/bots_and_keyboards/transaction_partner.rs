use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, StarAmount, User};

/// The other side of a Stars transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransactionPartner {
    /// A user, for example paying for an invoice or a subscription.
    #[non_exhaustive]
    User {
        /// The user.
        user: Box<User>,
        /// The affiliate that earned a commission on this sale. `None` unless the sale came through one.
        affiliate: Option<AffiliateInfo>,
        /// The payload the bot set on the invoice. `None` if the transaction carries none.
        invoice_payload: Option<String>,
        /// Length of the subscription period in seconds. `None` if it isn't a subscription.
        subscription_period: Option<i32>,
    },
    /// A chat or channel.
    Chat(Chat),
    /// Commission paid out by an affiliate program.
    #[non_exhaustive]
    AffiliateProgram {
        /// The bot that runs the program.
        sponsor_user: User,
        /// Commission as parts per thousand of the sale.
        commission_per_mille: i32,
    },
    /// A withdrawal to Fragment. `None` if it's neither pending nor failed but lacks a date and URL.
    Fragment(Option<RevenueWithdrawalState>),
    /// Telegram Ads.
    TelegramAds,
    /// Telegram API usage, such as paid broadcasts.
    #[non_exhaustive]
    TelegramApi {
        /// Number of requests over the regular limits that the transaction pays for. 0 if Telegram sends none.
        request_count: i32,
    },
    /// A partner this crate has no variant for, such as an app store.
    #[non_exhaustive]
    Unknown {
        /// TL constructor id of the transaction peer, for logging.
        constructor_id: u32,
    },
}

/// Affiliate commission that came with a sale.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AffiliateInfo {
    /// The chat that earned the commission.
    pub affiliate: Chat,
    /// Commission as parts per thousand of the sale.
    pub commission_per_mille: i32,
    /// Commission amount the affiliate received.
    pub amount: StarAmount,
}

/// Progress of a withdrawal to Fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RevenueWithdrawalState {
    /// The withdrawal is still in progress.
    Pending,
    /// The withdrawal finished.
    #[non_exhaustive]
    Succeeded {
        /// Unix time the withdrawal completed.
        date: i32,
        /// Link to the withdrawal's transaction.
        url: String,
    },
    /// The withdrawal failed.
    Failed,
}

impl TransactionPartner {
    pub(crate) fn parse(raw: &types::StarsTransaction, entities: &Entities) -> TransactionPartner {
        let peer = match &raw.peer {
            base::StarsTransactionPeer::StarsTransactionPeer(p) => &p.peer,
            base::StarsTransactionPeer::StarsTransactionPeerFragment => {
                let state = if raw.failed {
                    Some(RevenueWithdrawalState::Failed)
                } else if raw.pending {
                    Some(RevenueWithdrawalState::Pending)
                } else {
                    raw.transaction_date
                        .zip(raw.transaction_url.clone())
                        .map(|(date, url)| RevenueWithdrawalState::Succeeded { date, url })
                };
                return TransactionPartner::Fragment(state);
            }
            base::StarsTransactionPeer::StarsTransactionPeerAds => return TransactionPartner::TelegramAds,
            base::StarsTransactionPeer::StarsTransactionPeerAPI => {
                return TransactionPartner::TelegramApi { request_count: raw.floodskip_number.unwrap_or_default() };
            }
            other => return TransactionPartner::Unknown { constructor_id: other.constructor_id() },
        };
        let base::Peer::PeerUser(p) = peer else {
            return TransactionPartner::Chat(Chat::parse(peer, entities, false));
        };
        let user = User::parse(p.user_id, entities);
        // The bot's own sales name the affiliate in starref_peer; an affiliate's commission has the rate but
        // no starref_peer, and its peer is the sponsoring bot.
        let affiliate = match (&raw.starref_peer, raw.starref_commission_permille) {
            (Some(peer), Some(commission_per_mille)) => Some(AffiliateInfo {
                affiliate: Chat::parse(peer, entities, false),
                commission_per_mille,
                amount: raw.starref_amount.as_ref().map(StarAmount::parse).unwrap_or_default(),
            }),
            (None, Some(commission_per_mille)) => {
                return TransactionPartner::AffiliateProgram { sponsor_user: user, commission_per_mille };
            }
            _ => None,
        };
        TransactionPartner::User {
            user: Box::new(user),
            affiliate,
            invoice_payload: raw.bot_payload.as_deref().map(|p| String::from_utf8_lossy(p).into_owned()),
            subscription_period: raw.subscription_period,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AffiliateInfo, RevenueWithdrawalState, TransactionPartner};
    use crate::dispatcher::Entities;
    use crate::raw::{Serializable, base, types};
    use crate::types::{Chat, StarAmount, User};
    use crate::utils::get_peer;

    fn transaction(peer: base::StarsTransactionPeer) -> types::StarsTransaction {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        String::from("t").serialize(&mut buf);
        base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 50, nanos: 0 })).serialize(&mut buf);
        1i32.serialize(&mut buf);
        peer.serialize(&mut buf);
        types::StarsTransaction::deserialize_bare(&mut &buf[..]).unwrap()
    }

    fn with_peer(id: i64) -> types::StarsTransaction {
        transaction(base::StarsTransactionPeer::StarsTransactionPeer(Box::new(types::StarsTransactionPeer {
            peer: get_peer(id),
        })))
    }

    #[test]
    fn maps_fragment_withdrawal_states() {
        let entities = Entities::default();
        let state = |t: &types::StarsTransaction| match TransactionPartner::parse(t, &entities) {
            TransactionPartner::Fragment(state) => state,
            other => panic!("not a Fragment partner: {other:?}"),
        };
        let mut t = transaction(base::StarsTransactionPeer::StarsTransactionPeerFragment);
        assert_eq!(state(&t), None);
        (t.transaction_date, t.transaction_url) = (Some(7), Some("https://fragment.com/tx".into()));
        let succeeded = RevenueWithdrawalState::Succeeded { date: 7, url: "https://fragment.com/tx".into() };
        assert_eq!(state(&t), Some(succeeded));
        t.pending = true;
        assert_eq!(state(&t), Some(RevenueWithdrawalState::Pending));
        t.failed = true;
        assert_eq!(state(&t), Some(RevenueWithdrawalState::Failed));
    }

    #[test]
    fn tells_affiliate_sales_from_affiliate_commissions() {
        let entities = Entities::default();
        let user = User { id: 10, ..User::empty() };

        let mut sale = with_peer(10);
        sale.bot_payload = Some(b"order-1".to_vec());
        (sale.starref_commission_permille, sale.starref_peer) = (Some(100), Some(get_peer(-1000000000020)));
        sale.starref_amount =
            Some(base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 5, nanos: 0 })));
        let TransactionPartner::User { user: buyer, affiliate, invoice_payload, .. } =
            TransactionPartner::parse(&sale, &entities)
        else {
            panic!("not a User partner");
        };
        let affiliate_chat = Chat::parse(&get_peer(-1000000000020), &entities, false);
        assert_eq!((*buyer, invoice_payload.as_deref()), (user.clone(), Some("order-1")));
        let expected =
            AffiliateInfo { affiliate: affiliate_chat, commission_per_mille: 100, amount: StarAmount::new(5) };
        assert_eq!(affiliate, Some(expected));

        let mut commission = with_peer(10);
        commission.starref_commission_permille = Some(100);
        let program = TransactionPartner::AffiliateProgram { sponsor_user: user, commission_per_mille: 100 };
        assert_eq!(TransactionPartner::parse(&commission, &entities), program);
    }

    #[test]
    fn maps_chat_and_api_partners() {
        let entities = Entities::default();
        let chat = TransactionPartner::parse(&with_peer(-1000000000020), &entities);
        assert!(matches!(chat, TransactionPartner::Chat(c) if c.id == -1000000000020));

        let mut api = transaction(base::StarsTransactionPeer::StarsTransactionPeerAPI);
        api.floodskip_number = Some(3);
        assert_eq!(TransactionPartner::parse(&api, &entities), TransactionPartner::TelegramApi { request_count: 3 });
    }

    #[test]
    fn unmapped_partners_are_unknown_with_their_constructor_id() {
        let store = transaction(base::StarsTransactionPeer::StarsTransactionPeerAppStore);
        let partner = TransactionPartner::parse(&store, &Entities::default());
        assert_eq!(partner, TransactionPartner::Unknown { constructor_id: 0xb457_b375 });
    }
}
