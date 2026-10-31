use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::GiftAuctionState;

impl Client {
    pub async fn get_gift_auction_state(&self, gift_id: i64) -> Result<GiftAuctionState> {
        let auction = base::InputStarGiftAuction::InputStarGiftAuction(types::InputStarGiftAuction { gift_id });
        let request = functions::payments::GetStarGiftAuctionState { auction, version: 0 };
        let base::payments::StarGiftAuctionState::StarGiftAuctionState(reply) = self.invoke(&request).await?;
        let mut reply = *reply;
        let entities = Entities::new(self, std::mem::take(&mut reply.users), std::mem::take(&mut reply.chats));
        GiftAuctionState::parse(&reply, &entities)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::messages_and_media::media::tests::document_media;
    use crate::types::{
        ActiveGiftAuction, FinishedGiftAuction, GiftAuctionBid, GiftAuctionBidLevel, GiftAuctionRound,
        GiftAuctionStatus,
    };

    fn auction_gift() -> base::StarGift {
        let sticker = base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: false,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        }));
        let base::MessageMedia::MessageMediaDocument(media) = document_media("image/webp", vec![sticker]) else {
            panic!("expected a document")
        };
        base::StarGift::StarGift(Box::new(types::StarGift {
            limited: true,
            sold_out: false,
            birthday: false,
            require_premium: false,
            limited_per_user: false,
            peer_color_available: false,
            auction: true,
            id: 1,
            sticker: media.document.expect("a sticker"),
            stars: 50,
            availability_remains: Some(10),
            availability_total: Some(100),
            availability_resale: None,
            convert_stars: 40,
            first_sale_date: None,
            last_sale_date: None,
            upgrade_stars: None,
            resell_min_stars: None,
            title: None,
            released_by: None,
            per_user_total: None,
            per_user_remains: None,
            locked_until_date: None,
            auction_slug: Some("auction".into()),
            gifts_per_round: Some(10),
            auction_start_date: Some(100),
            upgrade_variants: None,
            background: None,
        }))
    }

    fn active() -> base::StarGiftAuctionState {
        let level = types::AuctionBidLevel { pos: 1, amount: 700, date: 150 };
        base::StarGiftAuctionState::StarGiftAuctionState(Box::new(types::StarGiftAuctionState {
            version: 3,
            start_date: 100,
            end_date: 900,
            min_bid_amount: 500,
            bid_levels: vec![base::AuctionBidLevel::AuctionBidLevel(Box::new(level))],
            top_bidders: vec![8],
            next_round_at: 300,
            last_gift_num: 90,
            gifts_left: 10,
            current_round: 2,
            total_rounds: 3,
            rounds: vec![
                base::StarGiftAuctionRound::StarGiftAuctionRound(types::StarGiftAuctionRound { num: 1, duration: 60 }),
                base::StarGiftAuctionRound::StarGiftAuctionRoundExtendable(Box::new(
                    types::StarGiftAuctionRoundExtendable { num: 2, duration: 60, extend_top: 3, extend_window: 30 },
                )),
            ],
        }))
    }

    fn user_state(bid: bool) -> base::StarGiftAuctionUserState {
        base::StarGiftAuctionUserState::StarGiftAuctionUserState(Box::new(types::StarGiftAuctionUserState {
            returned: false,
            bid_amount: bid.then_some(700),
            bid_date: bid.then_some(150),
            min_bid_amount: bid.then_some(701),
            bid_peer: bid.then_some(base::Peer::PeerUser(types::PeerUser { user_id: 5 })),
            acquired_count: 1,
        }))
    }

    fn reply(gift: base::StarGift, state: base::StarGiftAuctionState, bid: bool) -> Vec<u8> {
        let reply = types::payments::StarGiftAuctionState {
            gift,
            state,
            user_state: user_state(bid),
            timeout: 30,
            users: Vec::new(),
            chats: Vec::new(),
        };
        base::payments::StarGiftAuctionState::StarGiftAuctionState(Box::new(reply)).to_bytes()
    }

    #[tokio::test]
    async fn maps_an_active_auction_and_the_users_bid() {
        let (client, mut sent) = Client::scripted(vec![Ok(reply(auction_gift(), active(), true))]);
        let state = client.get_gift_auction_state(1).await.unwrap();
        let auction = base::InputStarGiftAuction::InputStarGiftAuction(types::InputStarGiftAuction { gift_id: 1 });
        let request = functions::payments::GetStarGiftAuctionState { auction, version: 0 };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());

        assert!(state.gift.is_auction);
        assert_eq!((state.acquired_gift_count, state.is_bid_returned, state.timeout), (1, false, 30));
        let GiftAuctionStatus::Active(active) = state.status else { panic!("expected an active auction") };
        let ActiveGiftAuction { top_bidders, .. } = &*active;
        assert_eq!(top_bidders.iter().map(|u| u.id).collect::<Vec<_>>(), [8]);
        let expected = ActiveGiftAuction {
            start_date: 100,
            end_date: 900,
            min_bid_star_count: 500,
            bid_levels: vec![GiftAuctionBidLevel { position: 1, star_count: 700, date: 150 }],
            top_bidders: top_bidders.clone(),
            next_round_date: 300,
            last_gift_number: 90,
            remaining_gift_count: 10,
            current_round: 2,
            total_round_count: 3,
            rounds: vec![
                GiftAuctionRound { number: 1, duration: 60, extend_top: None, extend_window: None },
                GiftAuctionRound { number: 2, duration: 60, extend_top: Some(3), extend_window: Some(30) },
            ],
        };
        assert_eq!(*active, expected);
        let Some(GiftAuctionBid { star_count, date, min_star_count, recipient }) = state.my_bid else {
            panic!("expected a bid")
        };
        assert_eq!((star_count, date, min_star_count, recipient.id), (700, 150, 701, 5));
    }

    #[tokio::test]
    async fn maps_a_finished_auction_without_a_bid() {
        let finished = types::StarGiftAuctionStateFinished {
            start_date: 100,
            end_date: 900,
            average_price: 650,
            listed_count: Some(4),
            fragment_listed_count: Some(2),
            fragment_listed_url: Some("https://fragment.com".into()),
        };
        let finished = base::StarGiftAuctionState::StarGiftAuctionStateFinished(Box::new(finished));
        let (client, _sent) = Client::scripted(vec![Ok(reply(auction_gift(), finished, false))]);
        let state = client.get_gift_auction_state(1).await.unwrap();
        let expected = FinishedGiftAuction {
            start_date: 100,
            end_date: 900,
            average_price_star_count: 650,
            listed_count: Some(4),
            fragment_listed_count: Some(2),
            fragment_listed_url: Some("https://fragment.com".into()),
        };
        assert_eq!(state.status, GiftAuctionStatus::Finished(expected));
        assert_eq!(state.my_bid, None);
    }

    #[tokio::test]
    async fn no_state_or_an_upgraded_gift_is_unexpected() {
        let not_modified = base::StarGiftAuctionState::StarGiftAuctionStateNotModified;
        let unique = super::super::craft_gift::tests::unique();
        let replies = vec![Ok(reply(auction_gift(), not_modified, false)), Ok(reply(unique, active(), false))];
        let (client, _sent) = Client::scripted(replies);
        for _ in 0..2 {
            assert!(matches!(client.get_gift_auction_state(1).await, Err(Error::Unexpected(_))));
        }
    }
}
