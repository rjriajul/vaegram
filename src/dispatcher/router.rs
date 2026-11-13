use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use super::Entities;
use super::listeners::{Listeners, Offer};
pub(crate) use super::tasks::run_hook;
use super::tasks::{Tasks, guard};
use crate::Client;
use crate::client::Me;
use crate::filters::{Facts, Filter, Inspect, PeerKinds, Subject};
use crate::handlers::{Callback, ErrorCallback, HandlerId, HandlerInfo, RawCallback, Registered, Routed, callback};
use crate::raw::base;
use crate::types::{
    BoostUpdate, BotSubscriptionUpdated, BusinessConnection, CallbackQuery, ChatActionUpdated, ChatBoostRemoved,
    ChatBoostUpdated, ChatJoinRequest, ChatMemberUpdated, ChosenInlineResult, DeletedBusinessMessages,
    DeletedEphemeralMessages, DeletedMessages, EphemeralMessage, GuestMessage, InlineQuery, ManagedBotUpdated, Message,
    MessageGenerationStopped, MessageReactionCountUpdated, MessageReactionUpdated, PendingJoinRequestsUpdated,
    PinnedMessagesUpdated, PollAnswer, PollUpdated, PreCheckoutQuery, PurchasedPaidMedia, RawUpdate, ReactionsUpdated,
    ReadHistoryUpdated, SharedWidgetQuery, ShippingQuery, StarsBalanceUpdated, Story, TonWalletState,
    UserStatusUpdated, WidgetDataUpdated, WidgetQuery,
};

type Scan = (Vec<usize>, Option<usize>);
const PARSED: Scan = (Vec::new(), Some(0));

pub(crate) struct Route<T> {
    group: i32,
    stop: bool,
    id: u32,
    filter: Filter<T>,
    callback: Callback<T>,
}

impl<T> Clone for Route<T> {
    fn clone(&self) -> Self {
        let (filter, callback) = (self.filter.clone(), self.callback.clone());
        Route { group: self.group, stop: self.stop, id: self.id, filter, callback }
    }
}

pub(crate) struct Handlers<T>(Vec<Route<T>>);

impl<T> Default for Handlers<T> {
    fn default() -> Self {
        Handlers(Vec::new())
    }
}

impl<T> Clone for Handlers<T> {
    fn clone(&self) -> Self {
        Handlers(self.0.clone())
    }
}

impl<T> Handlers<T> {
    fn remove(&mut self, id: u32) -> bool {
        let before = self.0.len();
        self.0.retain(|r| r.id != id);
        self.0.len() != before
    }

    fn describe(&self, kind: UpdateKind, list: &mut Vec<HandlerInfo>) {
        list.extend(self.0.iter().map(|r| HandlerInfo::new(kind, r.id, Some(r.group), r.stop)));
    }
}

impl<T: Routed + Subject + Inspect> Handlers<T> {
    pub(crate) fn push(&mut self, id: u32, filter: Filter<T>, callback: Callback<T>) {
        self.insert(Route { group: 0, stop: false, id, filter, callback });
    }

    fn insert(&mut self, route: Route<T>) {
        let at = self.0.partition_point(|r| (r.group, r.id) < (route.group, route.id));
        self.0.insert(at, route);
    }

    fn configure(&mut self, id: u32, group: Option<i32>, stop: bool) {
        if let Some(at) = self.0.iter().position(|r| r.id == id) {
            let mut route = self.0.remove(at);
            route.group = group.unwrap_or(route.group);
            route.stop |= stop;
            self.insert(route);
        }
    }

    fn walk(&self, start: usize, matched: &mut Vec<usize>, test: impl Fn(&Filter<T>) -> Option<bool>) -> Option<usize> {
        let mut group = matched.last().map(|&i| self.0[i].group);
        for (i, route) in self.0.iter().enumerate().skip(start) {
            if group == Some(route.group) {
                continue;
            }
            match test(&route.filter) {
                None => return Some(i),
                Some(false) => continue,
                Some(true) => matched.push(i),
            }
            if route.stop {
                break;
            }
            group = Some(route.group);
        }
        None
    }

    /// Walks the raw update's facts. `None` when nothing can match, so the update is never parsed; a miss
    /// allocates nothing.
    fn scan(&self, facts: &Facts<'_>) -> Option<Scan> {
        let mut matched = Vec::new();
        let resume = self.walk(0, &mut matched, |f| f.decide(facts));
        (!matched.is_empty() || resume.is_some()).then_some((matched, resume))
    }

    fn finish(&self, kind: UpdateKind, tasks: &mut Tasks, me: &Me, (mut matched, resume): Scan, item: T) {
        if let Some(start) = resume {
            let facts = item.facts(me);
            let events = &tasks.events;
            let test = |f: &Filter<T>| guard(events, kind, item.chat_id(), || f.eval(&facts, &item)).unwrap_or(false);
            self.walk(start, &mut matched, |f| Some(test(f)));
        }
        if !matched.is_empty() {
            tasks.run(kind, item, matched.iter().map(|&i| self.0[i].callback.clone()).collect());
        }
    }
}

impl Handlers<Message> {
    fn run(&self, kind: UpdateKind, tasks: &mut Tasks, client: &Client, raw: base::Message, entities: &Entities) {
        self.run_business(kind, tasks, client, raw, entities, None);
    }

    fn run_business(
        &self,
        kind: UpdateKind,
        tasks: &mut Tasks,
        client: &Client,
        mut raw: base::Message,
        entities: &Entities,
        business: Option<(String, Option<base::Message>)>,
    ) {
        // Only new messages reach listeners: never edits, and never business messages.
        let listening =
            kind == UpdateKind::Message && business.is_none() && client.0.handlers.listeners.messages.waiting();
        if self.0.is_empty() && !listening {
            return;
        }
        let me = client.0.me.lock().unwrap().clone();
        if let base::Message::Message(m) = &mut raw {
            Message::fill_rich_text(m);
        }
        let facts = Facts::message(&raw, PeerKinds::message(&raw, entities), &me).via_business(business.is_some());
        let scan = self.scan(&facts);
        if scan.is_none() && !listening {
            return;
        }
        let Some(mut message) = Message::from_raw(client, raw, entities) else { return };
        if let Some((connection_id, reply_to)) = business {
            message.set_business(client, connection_id, reply_to, entities);
        }
        if listening {
            match listened(kind, tasks, client.0.handlers.listeners.messages.offer(message, &me)) {
                Some(m) => message = m,
                None => return,
            }
        }
        if let Some(scan) = scan {
            self.finish(kind, tasks, &me, scan, message);
        }
    }

    /// A new message a listener took but dropped unread, or held and gave back: the other listeners, then the
    /// handlers, get it. Holds don't take it again.
    fn redeliver(&self, tasks: &mut Tasks, client: &Client, message: Message) {
        let me = client.0.me.lock().unwrap().clone();
        let offer = client.0.handlers.listeners.messages.offer_returned(message, &me);
        let Some(message) = listened(UpdateKind::Message, tasks, offer) else { return };
        if !self.0.is_empty() {
            self.finish(UpdateKind::Message, tasks, &me, PARSED, message);
        }
    }
}

/// What a new message offered to the listeners leaves for the handlers; `None` once one of the listeners, or a
/// next-step handler, took it.
fn listened(kind: UpdateKind, tasks: &mut Tasks, offer: Offer<Message>) -> Option<Message> {
    match offer {
        Offer::Claimed => None,
        Offer::Call(callback, m) => {
            tasks.run(kind, m, vec![callback]);
            None
        }
        Offer::Unclaimed(m) | Offer::Stranger(m, _) => Some(m),
    }
}

type Hook = Option<(u32, Callback<Client>)>;

/// The lifecycle hooks, one of each kind: registering again replaces.
#[derive(Default, Clone)]
pub(crate) struct Hooks {
    start: Hook,
    stop: Hook,
    connect: Hook,
    disconnect: Hook,
}

impl Hooks {
    fn slot(&mut self, kind: UpdateKind) -> Option<&mut Hook> {
        match kind {
            UpdateKind::Start => Some(&mut self.start),
            UpdateKind::Stop => Some(&mut self.stop),
            UpdateKind::Connect => Some(&mut self.connect),
            UpdateKind::Disconnect => Some(&mut self.disconnect),
            _ => None,
        }
    }

    pub(crate) fn set(&mut self, kind: UpdateKind, id: u32, hook: Callback<Client>) {
        if let Some(slot) = self.slot(kind) {
            *slot = Some((id, hook));
        }
    }

    fn remove(&mut self, kind: UpdateKind, id: u32) -> bool {
        self.slot(kind).and_then(|slot| slot.take_if(|(i, _)| *i == id)).is_some()
    }

    pub(crate) fn get(&self, kind: UpdateKind) -> Option<Callback<Client>> {
        let hook = match kind {
            UpdateKind::Start => &self.start,
            UpdateKind::Stop => &self.stop,
            UpdateKind::Connect => &self.connect,
            UpdateKind::Disconnect => &self.disconnect,
            _ => return None,
        };
        hook.as_ref().map(|(_, hook)| hook.clone())
    }

    fn describe(&self, list: &mut Vec<HandlerInfo>) {
        let hooks = [
            (UpdateKind::Start, &self.start),
            (UpdateKind::Stop, &self.stop),
            (UpdateKind::Connect, &self.connect),
            (UpdateKind::Disconnect, &self.disconnect),
        ];
        for (kind, hook) in hooks {
            list.extend(hook.as_ref().map(|(id, _)| HandlerInfo::new(kind, *id, None, false)));
        }
    }
}

/// Each kind line gives its `UpdateKind`, `Router` list and `configure` arm; its routes get the standard arm.
/// A parse may return `T` or `Option<T>` (`.into()` turns both into `Option<T>`); `None` skips the update.
/// Kinds without routes are dispatched by hand in `dispatch`.
macro_rules! update_kinds {
    (
        |$client:ident, $entities:ident, $me:ident|
        $($(#[$doc:meta])* $field:ident: $kind:ident => $ty:ty { $($variant:ident($u:ident) => $parse:expr),* $(,)? })*
    ) => {
        /// Which handler route an update or a handler belongs to; [`HandlerError::kind`] says which one failed.
        /// Each variant names the `Client::on_*` method that registers it.
        ///
        /// [`HandlerError::kind`]: crate::handlers::HandlerError::kind
        #[non_exhaustive]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum UpdateKind {
            $($(#[$doc])* $kind,)*
            /// Every update unparsed, before filtering: [`on_raw_update`](crate::Client::on_raw_update).
            Raw,
            /// The hook `run` awaits before the first update: [`on_start`](crate::Client::on_start).
            Start,
            /// The hook that runs as `run` returns: [`on_stop`](crate::Client::on_stop).
            Stop,
            /// The hook for the main connection coming back: [`on_connect`](crate::Client::on_connect).
            Connect,
            /// The hook for the main connection being lost or closed: [`on_disconnect`](crate::Client::on_disconnect).
            Disconnect,
        }

        #[derive(Default, Clone)]
        pub(crate) struct Router {
            $(pub(crate) $field: Handlers<$ty>,)*
            pub(crate) raw: Vec<(u32, RawCallback)>,
            pub(crate) on_error: Option<ErrorCallback>,
            pub(crate) hooks: Hooks,
            next_id: u32,
        }

        impl Router {
            pub(crate) fn configure(&mut self, kind: UpdateKind, id: u32, group: Option<i32>, stop: bool) {
                match kind {
                    $(UpdateKind::$kind => self.$field.configure(id, group, stop),)*
                    UpdateKind::Raw
                    | UpdateKind::Start
                    | UpdateKind::Stop
                    | UpdateKind::Connect
                    | UpdateKind::Disconnect => {}
                }
            }

            pub(crate) fn remove(&mut self, kind: UpdateKind, id: u32) -> bool {
                match kind {
                    $(UpdateKind::$kind => self.$field.remove(id),)*
                    UpdateKind::Raw => {
                        let before = self.raw.len();
                        self.raw.retain(|(i, _)| *i != id);
                        self.raw.len() != before
                    }
                    UpdateKind::Start | UpdateKind::Stop | UpdateKind::Connect | UpdateKind::Disconnect => {
                        self.hooks.remove(kind, id)
                    }
                }
            }

            /// Every route by kind, each in the order it runs; then the raw handlers and the hooks.
            pub(crate) fn list(&self) -> Vec<HandlerInfo> {
                let mut list = Vec::new();
                $(self.$field.describe(UpdateKind::$kind, &mut list);)*
                list.extend(self.raw.iter().map(|(id, _)| HandlerInfo::new(UpdateKind::Raw, *id, None, false)));
                self.hooks.describe(&mut list);
                list
            }

            fn route(
                &self,
                $client: &Client,
                update: base::Update,
                $entities: &Entities,
                tasks: &mut Tasks,
            ) -> Option<base::Update> {
                let $me = || $client.0.me.lock().unwrap().clone();
                match update {
                    $($(base::Update::$variant($u) if !self.$field.0.is_empty() => {
                        let item: Option<$ty> = $parse.into();
                        if let Some(item) = item {
                            self.$field.finish(UpdateKind::$kind, tasks, &$me(), PARSED, item);
                        }
                    })*)*
                    update => return Some(update),
                }
                None
            }
        }
    };
}

update_kinds! {
    |client, entities, me|
    // Messages and plain callback queries are pre-filtered on `Facts` in `dispatch`, before they are parsed.
    /// New messages: [`on_message`](crate::Client::on_message).
    message: Message => Message {}
    /// Edited messages: [`on_edited_message`](crate::Client::on_edited_message).
    edited_message: EditedMessage => Message {}
    /// Presses of inline keyboard buttons: [`on_callback_query`](crate::Client::on_callback_query).
    callback_query: CallbackQuery => CallbackQuery {
        UpdateEphemeralBotCallbackQuery(u) => CallbackQuery::parse_ephemeral(client, *u, entities),
        UpdateInlineBotCallbackQuery(u) => CallbackQuery::parse_inline(client, *u, entities),
        UpdateBusinessBotCallbackQuery(u) => CallbackQuery::parse_business(client, *u, entities),
    }
    /// Queries from widgets: [`on_widget_query`](crate::Client::on_widget_query).
    widget_query: WidgetQuery => WidgetQuery {
        UpdateBotWidgetQuery(u) => WidgetQuery::parse(client, &u, entities),
        UpdateInlineBotWidgetQuery(u) => WidgetQuery::parse_inline(client, *u, entities),
    }
    /// Queries from shared widget copies: [`on_shared_widget_query`](crate::Client::on_shared_widget_query).
    shared_widget_query: SharedWidgetQuery => SharedWidgetQuery {
        UpdateBotWidgetCopyQuery(u) => SharedWidgetQuery::parse(client, &u, entities),
    }
    /// Changes to widget data: [`on_widget_data_updated`](crate::Client::on_widget_data_updated).
    widget_data_updated: WidgetDataUpdated => WidgetDataUpdated {
        UpdateMessageWidgetData(u) => WidgetDataUpdated::parse(client, &u, entities),
    }
    /// Queries typed after the bot's username: [`on_inline_query`](crate::Client::on_inline_query).
    inline_query: InlineQuery => InlineQuery {
        UpdateBotInlineQuery(u) => InlineQuery::parse(client, &u, entities),
    }
    /// Inline results a user picked: [`on_chosen_inline_result`](crate::Client::on_chosen_inline_result).
    chosen_inline_result: ChosenInlineResult => ChosenInlineResult {
        UpdateBotInlineSend(u) => ChosenInlineResult::parse(client, &u, entities),
    }
    /// Requests to join a chat: [`on_chat_join_request`](crate::Client::on_chat_join_request).
    chat_join_request: ChatJoinRequest => ChatJoinRequest {
        UpdateBotChatInviteRequester(u) => ChatJoinRequest::parse(client, &u, entities),
    }
    /// Messages sent through the bot in chats it isn't in: [`on_guest_message`](crate::Client::on_guest_message).
    guest_message: GuestMessage => GuestMessage {
        UpdateBotGuestChatQuery(u) => GuestMessage::parse(client, *u, entities),
    }
    /// Changes to Star subscriptions to the bot:
    /// [`on_bot_subscription_updated`](crate::Client::on_bot_subscription_updated).
    bot_subscription_updated: BotSubscriptionUpdated => BotSubscriptionUpdated {
        UpdateBotStarsSubscription(u) => BotSubscriptionUpdated::parse(client, &u, entities),
    }
    // A private chat's typing update may also be a stopped draft, so `dispatch` routes it to both kinds.
    /// Streamed message drafts that were stopped:
    /// [`on_message_generation_stopped`](crate::Client::on_message_generation_stopped).
    message_generation_stopped: MessageGenerationStopped => MessageGenerationStopped {}
    /// Typing and other activities shown in a chat: [`on_chat_action`](crate::Client::on_chat_action).
    chat_action: ChatAction => ChatActionUpdated {
        UpdateChatUserTyping(u) => ChatActionUpdated::parse_chat(client, &u, entities),
        UpdateChannelUserTyping(u) => ChatActionUpdated::parse_channel(client, &u, entities),
    }
    /// Updates to bots this account manages: [`on_managed_bot`](crate::Client::on_managed_bot).
    managed_bot: ManagedBot => ManagedBotUpdated {
        UpdateManagedBot(u) => ManagedBotUpdated::parse(client, &u, entities),
    }
    /// New ephemeral messages: [`on_ephemeral_message`](crate::Client::on_ephemeral_message).
    ephemeral_message: EphemeralMessage => EphemeralMessage {
        UpdateNewEphemeralMessage(u) => EphemeralMessage::parse(client, u.message, entities),
    }
    /// Edited ephemeral messages: [`on_edited_ephemeral_message`](crate::Client::on_edited_ephemeral_message).
    edited_ephemeral_message: EditedEphemeralMessage => EphemeralMessage {
        UpdateEditEphemeralMessage(u) => EphemeralMessage::parse(client, u.message, entities),
    }
    /// Deleted ephemeral messages: [`on_deleted_ephemeral_messages`](crate::Client::on_deleted_ephemeral_messages).
    deleted_ephemeral_messages: DeletedEphemeralMessages => DeletedEphemeralMessages {
        UpdateDeleteEphemeralMessages(u) => DeletedEphemeralMessages::parse(client, *u, entities),
    }
    /// A user's or chat's reactions to a message changed: [`on_message_reaction`](crate::Client::on_message_reaction).
    message_reaction: MessageReaction => MessageReactionUpdated {
        UpdateBotMessageReaction(u) => MessageReactionUpdated::parse(client, &u, entities),
    }
    /// Anonymous reaction counts on a message changed:
    /// [`on_message_reaction_count`](crate::Client::on_message_reaction_count).
    message_reaction_count: MessageReactionCount => MessageReactionCountUpdated {
        UpdateBotMessageReactions(u) => MessageReactionCountUpdated::parse(client, &u, entities),
    }
    /// The reactions on a message changed, as user accounts see them:
    /// [`on_reactions`](crate::Client::on_reactions).
    reactions: Reactions => ReactionsUpdated {
        UpdateMessageReactions(u) => ReactionsUpdated::parse(client, &u, entities),
    }
    /// A business connection of the bot changed: [`on_business_connection`](crate::Client::on_business_connection).
    business_connection: BusinessConnection => BusinessConnection {
        UpdateBotBusinessConnect(u) => BusinessConnection::parse(client, u.connection, entities),
    }
    /// New messages through a business connection: [`on_business_message`](crate::Client::on_business_message).
    business_message: BusinessMessage => Message {}
    /// Edited messages through a business connection:
    /// [`on_edited_business_message`](crate::Client::on_edited_business_message).
    edited_business_message: EditedBusinessMessage => Message {}
    /// Messages deleted through a business connection:
    /// [`on_deleted_business_messages`](crate::Client::on_deleted_business_messages).
    deleted_business_messages: DeletedBusinessMessages => DeletedBusinessMessages {
        UpdateBotDeleteBusinessMessage(u) => DeletedBusinessMessages::parse(client, *u, entities),
    }
    /// Shipping queries for invoices with a flexible price: [`on_shipping_query`](crate::Client::on_shipping_query).
    shipping_query: ShippingQuery => ShippingQuery {
        UpdateBotShippingQuery(u) => ShippingQuery::parse(client, &u, entities),
    }
    /// Queries sent before a payment completes: [`on_pre_checkout_query`](crate::Client::on_pre_checkout_query).
    pre_checkout_query: PreCheckoutQuery => PreCheckoutQuery {
        UpdateBotPrecheckoutQuery(u) => PreCheckoutQuery::parse(client, &u, entities),
    }
    /// Purchases of paid media: [`on_purchased_paid_media`](crate::Client::on_purchased_paid_media).
    purchased_paid_media: PurchasedPaidMedia => PurchasedPaidMedia {
        UpdateBotPurchasedPaidMedia(u) => PurchasedPaidMedia::parse(client, &u, entities),
    }
    // One boost update parses into either kind, so `dispatch` routes both.
    /// Boosts added to or changed in a chat: [`on_chat_boost`](crate::Client::on_chat_boost).
    chat_boost: ChatBoost => ChatBoostUpdated {}
    /// Boosts removed from a chat: [`on_removed_chat_boost`](crate::Client::on_removed_chat_boost).
    removed_chat_boost: RemovedChatBoost => ChatBoostRemoved {}
    // A member update goes to exactly one of these, depending on whose membership changed; `dispatch` routes both.
    /// Another member's status in a chat changed: [`on_chat_member_updated`](crate::Client::on_chat_member_updated).
    chat_member_updated: ChatMemberUpdated => ChatMemberUpdated {}
    /// This account's own membership changed: [`on_my_chat_member`](crate::Client::on_my_chat_member).
    my_chat_member: MyChatMember => ChatMemberUpdated {}
    /// A poll's results changed or it closed: [`on_poll`](crate::Client::on_poll).
    poll: Poll => PollUpdated {
        UpdateMessagePoll(u) => PollUpdated::parse(client, &u, entities),
    }
    /// Votes in a poll: [`on_poll_answer`](crate::Client::on_poll_answer).
    poll_answer: PollAnswer => PollAnswer {
        UpdateMessagePollVote(u) => PollAnswer::parse(client, *u, entities),
    }
    /// Deleted messages: [`on_deleted_messages`](crate::Client::on_deleted_messages).
    deleted_messages: DeletedMessages => DeletedMessages {
        UpdateDeleteMessages(u) => DeletedMessages::parse(client, *u),
        UpdateDeleteChannelMessages(u) => DeletedMessages::parse_channel(client, *u, entities),
    }
    /// Messages pinned or unpinned in a chat: [`on_pinned_messages`](crate::Client::on_pinned_messages).
    pinned_messages: PinnedMessages => PinnedMessagesUpdated {
        UpdatePinnedMessages(u) => PinnedMessagesUpdated::parse(client, *u, entities),
        UpdatePinnedChannelMessages(u) => PinnedMessagesUpdated::parse_channel(client, *u, entities),
    }
    /// Read receipts: [`on_read_history`](crate::Client::on_read_history).
    read_history: ReadHistory => ReadHistoryUpdated {
        UpdateReadHistoryInbox(u) => ReadHistoryUpdated::parse_inbox(client, &u, entities),
        UpdateReadHistoryOutbox(u) => ReadHistoryUpdated::parse_outbox(client, &u, entities),
        UpdateReadChannelInbox(u) => ReadHistoryUpdated::parse_channel_inbox(client, &u, entities),
        UpdateReadChannelOutbox(u) => ReadHistoryUpdated::parse_channel_outbox(client, &u, entities),
        UpdateReadChannelDiscussionInbox(u) => ReadHistoryUpdated::parse_thread_inbox(client, &u, entities),
        UpdateReadChannelDiscussionOutbox(u) => ReadHistoryUpdated::parse_thread_outbox(client, &u, entities),
    }
    /// The number of pending join requests in a chat changed:
    /// [`on_pending_join_requests`](crate::Client::on_pending_join_requests).
    pending_join_requests: PendingJoinRequests => PendingJoinRequestsUpdated {
        UpdatePendingJoinRequests(u) => PendingJoinRequestsUpdated::parse(client, &u, entities),
    }
    /// A user's online status changed: [`on_user_status`](crate::Client::on_user_status).
    user_status: UserStatus => UserStatusUpdated {
        UpdateUserStatus(u) => UserStatusUpdated::parse(client, &u, entities),
    }
    /// This account's TON wallet state changed: [`on_ton_wallet_state`](crate::Client::on_ton_wallet_state).
    ton_wallet_state: TonWalletState => TonWalletState {
        UpdateWalletState(u) => TonWalletState::parse(client, &u.state),
    }
    /// This account's Stars balance changed: [`on_stars_balance`](crate::Client::on_stars_balance).
    stars_balance: StarsBalance => StarsBalanceUpdated {
        UpdateStarsBalance(u) => StarsBalanceUpdated::parse(client, &u),
    }
    // A deleted story parses to `None`, so it never reaches a handler.
    /// Stories, apart from deleted ones: [`on_story`](crate::Client::on_story).
    story: Story => Story {
        UpdateStory(u) => {
            let u = *u;
            Story::parse(client, &u.peer, u.story, entities)
        }
    }
}

impl Router {
    pub(crate) fn next_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub(crate) fn dispatch(&self, client: &Client, update: base::Update, entities: &Arc<Entities>, tasks: &mut Tasks) {
        use base::Update as U;
        client.0.caches.observe(&update, entities);
        // Called here, in arrival order, before each future is spawned: `on_raw_update` promises it.
        for (_, callback) in &self.raw {
            let raw = RawUpdate { update: update.clone(), entities: entities.clone() };
            if let Some(future) = guard(&tasks.events, UpdateKind::Raw, None, || callback(client.clone(), raw)) {
                tasks.raw(future);
            }
        }
        let update = if client.0.handlers.listeners.callbacks.waiting() {
            let Some(update) = self.listen_callback(client, update, entities, tasks) else { return };
            update
        } else {
            update
        };
        let Some(update) = self.route(client, update, entities, tasks) else { return };
        let me = || client.0.me.lock().unwrap().clone();
        match update {
            U::UpdateNewMessage(u) => self.message.run(UpdateKind::Message, tasks, client, u.message, entities),
            U::UpdateNewChannelMessage(u) => self.message.run(UpdateKind::Message, tasks, client, u.message, entities),
            U::UpdateEditMessage(u) => {
                self.edited_message.run(UpdateKind::EditedMessage, tasks, client, u.message, entities)
            }
            U::UpdateEditChannelMessage(u) => {
                self.edited_message.run(UpdateKind::EditedMessage, tasks, client, u.message, entities)
            }
            U::UpdateBotCallbackQuery(u) if !self.callback_query.0.is_empty() => {
                let (me, kinds) = (me(), PeerKinds::callback(&u, entities));
                if let Some(scan) = self.callback_query.scan(&Facts::callback(&u, kinds, &me)) {
                    let query = CallbackQuery::parse(client, &u, entities);
                    self.callback_query.finish(UpdateKind::CallbackQuery, tasks, &me, scan, query);
                }
            }
            U::UpdateBotNewBusinessMessage(u) => {
                let (kind, u) = (UpdateKind::BusinessMessage, *u);
                let business = Some((u.connection_id, u.reply_to_message));
                self.business_message.run_business(kind, tasks, client, u.message, entities, business);
            }
            U::UpdateBotEditBusinessMessage(u) => {
                let (kind, u) = (UpdateKind::EditedBusinessMessage, *u);
                let business = Some((u.connection_id, u.reply_to_message));
                self.edited_business_message.run_business(kind, tasks, client, u.message, entities, business);
            }
            U::UpdateBotChatBoost(u) if !self.chat_boost.0.is_empty() || !self.removed_chat_boost.0.is_empty() => {
                match BoostUpdate::parse(client, *u, entities) {
                    Some(BoostUpdate::Updated(item)) => {
                        self.chat_boost.finish(UpdateKind::ChatBoost, tasks, &me(), PARSED, item)
                    }
                    Some(BoostUpdate::Removed(item)) => {
                        self.removed_chat_boost.finish(UpdateKind::RemovedChatBoost, tasks, &me(), PARSED, item)
                    }
                    None => {}
                }
            }
            U::UpdateChannelParticipant(u) => {
                self.member_update(tasks, &me(), u.user_id, || ChatMemberUpdated::parse_channel(client, *u, entities))
            }
            U::UpdateChatParticipant(u) => {
                self.member_update(tasks, &me(), u.user_id, || ChatMemberUpdated::parse_chat(client, *u, entities))
            }
            U::UpdateBotStopped(u) => {
                let me = me();
                self.member_update(tasks, &me, me.id, || {
                    ChatMemberUpdated::parse_bot_stopped(client, &me, &u, entities)
                })
            }
            U::UpdateUserTyping(u) => {
                if !self.message_generation_stopped.0.is_empty() {
                    if let Some(item) = MessageGenerationStopped::parse(client, &u, entities) {
                        let kind = UpdateKind::MessageGenerationStopped;
                        self.message_generation_stopped.finish(kind, tasks, &me(), PARSED, item);
                    }
                }
                if !self.chat_action.0.is_empty() {
                    if let Some(item) = ChatActionUpdated::parse_user(client, &u, entities) {
                        self.chat_action.finish(UpdateKind::ChatAction, tasks, &me(), PARSED, item);
                    }
                }
            }
            _ => {}
        }
    }

    /// Offers any callback query to the listeners; `None` once it was handled here.
    fn listen_callback(
        &self,
        client: &Client,
        update: base::Update,
        entities: &Entities,
        tasks: &mut Tasks,
    ) -> Option<base::Update> {
        use base::Update as U;
        let query = match update {
            U::UpdateBotCallbackQuery(u) => CallbackQuery::parse(client, &u, entities),
            U::UpdateInlineBotCallbackQuery(u) => CallbackQuery::parse_inline(client, *u, entities),
            U::UpdateBusinessBotCallbackQuery(u) => CallbackQuery::parse_business(client, *u, entities),
            U::UpdateEphemeralBotCallbackQuery(u) => CallbackQuery::parse_ephemeral(client, *u, entities),
            update => return Some(update),
        };
        self.offer_callback(client, query, tasks);
        None
    }

    /// The listeners, then the handlers, get a callback query.
    fn offer_callback(&self, client: &Client, query: CallbackQuery, tasks: &mut Tasks) {
        let me = client.0.me.lock().unwrap().clone();
        let kind = UpdateKind::CallbackQuery;
        match client.0.handlers.listeners.callbacks.offer(query, &me) {
            Offer::Claimed => {}
            Offer::Call(callback, query) => tasks.run(kind, query, vec![callback]),
            Offer::Unclaimed(query) if !self.callback_query.0.is_empty() => {
                self.callback_query.finish(kind, tasks, &me, PARSED, query)
            }
            Offer::Unclaimed(_) => {}
            Offer::Stranger(query, alert) => {
                let answer = callback(move |query: CallbackQuery| {
                    let alert = alert.clone();
                    async move { query.answer().text(&*alert).alert().await }
                });
                tasks.run(kind, query, vec![answer]);
            }
        }
    }

    /// Routes again what listeners dropped before reading gave back, as if it had just arrived.
    pub(crate) fn redeliver(&self, client: &Client, tasks: &mut Tasks) {
        let listeners = &client.0.handlers.listeners;
        for message in listeners.messages.take_returned() {
            self.message.redeliver(tasks, client, message);
        }
        for query in listeners.callbacks.take_returned() {
            self.offer_callback(client, query, tasks);
        }
    }

    fn member_update(&self, tasks: &mut Tasks, me: &Me, user_id: i64, parse: impl FnOnce() -> ChatMemberUpdated) {
        let (handlers, kind) = if user_id == me.id {
            (&self.my_chat_member, UpdateKind::MyChatMember)
        } else {
            (&self.chat_member_updated, UpdateKind::ChatMemberUpdated)
        };
        if !handlers.0.is_empty() {
            handlers.finish(kind, tasks, me, PARSED, parse());
        }
    }
}

#[derive(Default)]
pub(crate) struct Routes {
    router: RwLock<Arc<Router>>,
    generation: AtomicU64,
    pub(crate) listeners: Listeners,
}

impl Routes {
    pub(crate) fn add<R>(&self, f: impl FnOnce(&mut Router) -> R) -> R {
        let mut router = self.router.write().unwrap();
        let result = f(Arc::make_mut(&mut router));
        self.generation.fetch_add(1, Ordering::Release);
        result
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(crate) fn current(&self) -> Arc<Router> {
        self.router.read().unwrap().clone()
    }

    /// A route or a next-step handler. Only a route change makes running dispatchers take a new router.
    pub(crate) fn remove(&self, handler: HandlerId) -> bool {
        match handler.0 {
            Registered::Route(kind, id) => self.add(|r| r.remove(kind, id)),
            Registered::Listener(id) => self.listeners.remove(id),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    use tokio::sync::Semaphore;

    use super::*;
    use crate::dispatcher::listeners::tests::{dispatch as dispatch_on, raw_inline_query, raw_query, until_waiting};
    use crate::enums::ClientEvent;
    use crate::errors::Error;
    use crate::filters;
    use crate::raw::{Serializable, functions, types};
    use crate::types::messages_and_media::ephemeral_message::tests::{channel, ephemeral};
    use crate::types::messages_and_media::message::tests::service;
    use crate::utils::{get_channel_id, get_peer};

    type Log = Arc<Mutex<Vec<&'static str>>>;

    fn record<T: Send + 'static>(log: &Log, name: &'static str) -> Callback<T> {
        let log = log.clone();
        callback(move |_: T| {
            log.lock().unwrap().push(name);
            async { Ok(()) }
        })
    }

    fn fail<T: Send + 'static>(log: &Log, name: &'static str) -> Callback<T> {
        let log = log.clone();
        callback(move |_: T| {
            log.lock().unwrap().push(name);
            async { Err::<(), _>(Error::Unexpected("guard")) }
        })
    }

    fn new_message(id: i32, out: bool) -> base::Update {
        let mut message = Message::bare(id, get_peer(1), 0);
        (message.out, message.message) = (out, "/start".into());
        let message = base::Message::Message(Box::new(message));
        base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 0, pts_count: 0 }))
    }

    fn inline_query() -> base::Update {
        base::Update::UpdateBotInlineQuery(Box::new(types::UpdateBotInlineQuery {
            query_id: 1,
            user_id: 1,
            query: "hi".into(),
            geo: None,
            peer_type: None,
            offset: String::new(),
        }))
    }

    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    fn dispatch(router: &Router, update: base::Update) {
        let mut tasks = Tasks::default();
        tasks.on_error = router.on_error.clone();
        router.dispatch(&Client::offline(), update, &Arc::default(), &mut tasks);
    }

    fn router_of(client: &Client) -> Arc<Router> {
        client.0.handlers.current()
    }

    #[tokio::test]
    async fn routes_each_kind_to_its_own_list() {
        let log = Log::default();
        let mut router = Router::default();
        router.message.push(1, filters::all(), record(&log, "message"));
        router.edited_message.push(2, filters::all(), record(&log, "edited"));
        router.inline_query.push(3, filters::all(), record(&log, "inline"));
        router.raw.push((4, record_raw(&log)));
        dispatch(&router, new_message(1, false));
        dispatch(&router, inline_query());
        settle().await;
        let mut seen = log.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, ["inline", "message", "raw", "raw"]);
    }

    fn record_raw(log: &Log) -> RawCallback {
        let log = log.clone();
        Arc::new(move |_, _| {
            log.lock().unwrap().push("raw");
            Box::pin(async { Ok(()) })
        })
    }

    #[tokio::test]
    async fn remove_drops_only_that_route() {
        let log = Log::default();
        let mut router = Router::default();
        router.message.push(1, filters::all(), record(&log, "first"));
        router.message.push(2, filters::all(), record(&log, "second"));
        router.raw.push((3, record_raw(&log)));
        assert!(!router.remove(UpdateKind::EditedMessage, 1));
        assert!(router.remove(UpdateKind::Message, 1));
        assert!(!router.remove(UpdateKind::Message, 1));
        dispatch(&router, new_message(1, false));
        settle().await;
        let mut seen = log.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, ["raw", "second"]);

        assert!(router.remove(UpdateKind::Raw, 3));
        log.lock().unwrap().clear();
        dispatch(&router, new_message(2, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["second"]);
    }

    #[tokio::test]
    async fn routes_ephemeral_updates_to_their_own_lists() {
        let log = Log::default();
        let mut router = Router::default();
        router.message.push(1, filters::all(), record(&log, "message"));
        router.ephemeral_message.push(2, filters::command("start"), record(&log, "ephemeral /start"));
        router.ephemeral_message.push(3, filters::all(), record(&log, "ephemeral"));
        router.edited_ephemeral_message.push(4, filters::all(), record(&log, "edited"));
        router.deleted_ephemeral_messages.push(5, filters::chat(get_channel_id(42)), record(&log, "deleted"));
        router.callback_query.push(6, filters::data("tap"), record(&log, "callback"));
        let new =
            |message| base::Update::UpdateNewEphemeralMessage(Box::new(types::UpdateNewEphemeralMessage { message }));
        dispatch(&router, new(ephemeral(false, Some(channel()))));
        dispatch(
            &router,
            base::Update::UpdateDeleteEphemeralMessages(Box::new(types::UpdateDeleteEphemeralMessages {
                peer: channel(),
                ids: vec![7],
            })),
        );
        dispatch(
            &router,
            base::Update::UpdateEphemeralBotCallbackQuery(Box::new(types::UpdateEphemeralBotCallbackQuery {
                query_id: 9,
                user_id: 1000,
                peer: None,
                msg_id: 7,
                data: b"tap".to_vec(),
                chat_instance: None,
                message: ephemeral(true, None),
            })),
        );
        settle().await;
        let mut seen = log.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, ["callback", "deleted", "ephemeral"]);
    }

    #[tokio::test]
    async fn the_bots_own_membership_goes_only_to_my_chat_member() {
        let log = Log::default();
        let mut router = Router::default();
        router.chat_member_updated.push(1, filters::all(), record(&log, "member"));
        router.my_chat_member.push(2, filters::all(), record(&log, "mine"));
        // The offline client's own id is 0.
        let member = |user_id| {
            base::Update::UpdateChatParticipant(Box::new(types::UpdateChatParticipant {
                chat_id: 5,
                date: 1,
                actor_id: 7,
                user_id,
                prev_participant: None,
                new_participant: None,
                invite: None,
                qts: 1,
            }))
        };
        dispatch(&router, member(7));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["member"]);
        dispatch(&router, member(0));
        let stopped = types::UpdateBotStopped { user_id: 7, date: 1, stopped: true, qts: 2 };
        dispatch(&router, base::Update::UpdateBotStopped(Box::new(stopped)));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["member", "mine", "mine"]);
    }

    #[tokio::test]
    async fn wallet_state_update_reaches_on_ton_wallet_state() {
        let (client, seen) = (Client::offline(), Arc::new(Mutex::new(Vec::new())));
        let sink = seen.clone();
        client.on_ton_wallet_state(filters::all(), move |state| {
            sink.lock().unwrap().push((state.is_being_created, state.wallet_address));
            async { Ok(()) }
        });
        let state = base::WalletState::WalletStateEmpty(types::WalletStateEmpty { creating: true });
        let update = base::Update::UpdateWalletState(Box::new(types::UpdateWalletState { state }));
        dispatch(&router_of(&client), update);
        settle().await;
        assert_eq!(*seen.lock().unwrap(), [(true, None)]);
    }

    #[tokio::test]
    async fn widget_data_update_reaches_handler_through_chat_filter() {
        let (client, seen) = (Client::offline(), Arc::new(Mutex::new(Vec::new())));
        let sink = seen.clone();
        client.on_widget_data_updated(filters::chat(7), move |event| {
            sink.lock().unwrap().push((event.chat.id, event.message_id, event.public_data));
            async { Ok(()) }
        });
        let router = router_of(&client);
        for peer in [8, 7] {
            let update = types::UpdateMessageWidgetData {
                peer: get_peer(peer),
                msg_id: 42,
                public_data: Some("new".into()),
                public_data_hash: Some(123),
                chat_data: None,
                pts: 1,
                pts_count: 1,
            };
            dispatch(&router, base::Update::UpdateMessageWidgetData(Box::new(update)));
        }
        settle().await;
        assert_eq!(*seen.lock().unwrap(), [(7, 42, Some("new".into()))]);
    }

    #[tokio::test]
    async fn routes_member_poll_deletion_and_status_updates() {
        let log = Log::default();
        let mut router = Router::default();
        router.chat_member_updated.push(1, filters::chat(get_channel_id(42)), record(&log, "member"));
        router.poll.push(2, filters::all(), record(&log, "poll"));
        router.poll_answer.push(3, filters::user(7), record(&log, "answer"));
        router.deleted_messages.push(4, filters::chat(get_channel_id(42)), record(&log, "deleted"));
        router.user_status.push(5, filters::all(), record(&log, "status"));
        router.callback_query.push(6, filters::data("buy"), record(&log, "business callback"));
        let channel_member = types::UpdateChannelParticipant {
            via_chatlist: false,
            channel_id: 42,
            date: 1,
            actor_id: 7,
            user_id: 7,
            prev_participant: None,
            new_participant: None,
            invite: None,
            qts: 1,
        };
        dispatch(&router, base::Update::UpdateChannelParticipant(Box::new(channel_member)));
        let stopped = types::UpdateBotStopped { user_id: 7, date: 1, stopped: true, qts: 2 };
        dispatch(&router, base::Update::UpdateBotStopped(Box::new(stopped)));
        let results = base::PollResults::PollResults(Box::new(types::PollResults {
            min: false,
            has_unread_votes: false,
            can_view_stats: false,
            results: None,
            total_voters: None,
            recent_voters: None,
            solution: None,
            solution_entities: None,
            solution_media: None,
        }));
        let poll =
            types::UpdateMessagePoll { peer: None, msg_id: None, top_msg_id: None, poll_id: 9, poll: None, results };
        dispatch(&router, base::Update::UpdateMessagePoll(Box::new(poll)));
        let vote = types::UpdateMessagePollVote {
            poll_id: 9,
            peer: get_peer(7),
            options: vec![vec![0]],
            positions: vec![0],
            qts: 3,
        };
        dispatch(&router, base::Update::UpdateMessagePollVote(Box::new(vote)));
        let deleted = types::UpdateDeleteMessages { messages: vec![1], pts: 1, pts_count: 1 };
        dispatch(&router, base::Update::UpdateDeleteMessages(Box::new(deleted)));
        let deleted = types::UpdateDeleteChannelMessages { channel_id: 42, messages: vec![1], pts: 1, pts_count: 1 };
        dispatch(&router, base::Update::UpdateDeleteChannelMessages(Box::new(deleted)));
        let status = types::UpdateUserStatus { user_id: 7, status: base::UserStatus::UserStatusEmpty };
        dispatch(&router, base::Update::UpdateUserStatus(Box::new(status)));
        let business = |message| {
            base::Update::UpdateBusinessBotCallbackQuery(Box::new(types::UpdateBusinessBotCallbackQuery {
                query_id: 1,
                user_id: 7,
                connection_id: "conn".into(),
                message,
                reply_to_message: None,
                chat_instance: 3,
                data: Some(b"buy".to_vec()),
            }))
        };
        let message = Message::bare(5, get_peer(7), 0);
        dispatch(&router, business(base::Message::Message(Box::new(message))));
        settle().await;
        let mut seen = log.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, ["answer", "business callback", "deleted", "member", "status"]);
    }

    #[tokio::test]
    async fn reaction_updates_reach_their_handlers_through_filters() {
        let (client, log) = (Client::offline(), Log::default());
        let chat = crate::utils::get_channel_id(42);
        let (reaction, other, count, late) = (log.clone(), log.clone(), log.clone(), log.clone());
        client.on_message_reaction(filters::user(8), move |_| {
            other.lock().unwrap().push("other user");
            async { Ok(()) }
        });
        client.on_message_reaction(filters::user(7) & filters::chat(chat), move |r| {
            if (r.chat.id, r.user.map(|u| u.id), r.message_id) == (chat, Some(7), 10) {
                reaction.lock().unwrap().push("reaction");
            }
            async { Ok(()) }
        });
        // Within a group the first match wins, so this one only runs if `.group(1)` reached its list.
        client
            .on_message_reaction_count(filters::all(), move |_| {
                late.lock().unwrap().push("count group 1");
                async { Ok(()) }
            })
            .group(1);
        client.on_message_reaction_count(filters::chat(chat), move |r| {
            if (r.chat.id, r.message_id) == (chat, 10) {
                count.lock().unwrap().push("count");
            }
            async { Ok(()) }
        });
        let router = router_of(&client);
        let update = types::UpdateBotMessageReaction {
            peer: get_peer(chat),
            msg_id: 10,
            date: 0,
            actor: get_peer(7),
            old_reactions: Vec::new(),
            new_reactions: Vec::new(),
            qts: 1,
        };
        dispatch(&router, base::Update::UpdateBotMessageReaction(Box::new(update)));
        let update = types::UpdateBotMessageReactions {
            peer: get_peer(chat),
            msg_id: 10,
            date: 0,
            reactions: Vec::new(),
            qts: 2,
        };
        dispatch(&router, base::Update::UpdateBotMessageReactions(Box::new(update)));
        settle().await;
        let mut seen = log.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, ["count", "count group 1", "reaction"]);
    }

    #[tokio::test]
    async fn first_match_in_a_group_wins() {
        let log = Log::default();
        let mut router = Router::default();
        router.message.push(1, filters::outgoing(), record(&log, "outgoing"));
        router.message.push(2, filters::command("start"), record(&log, "start"));
        router.message.push(3, filters::all(), record(&log, "all"));
        dispatch(&router, new_message(1, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["start"]);
    }

    #[tokio::test]
    async fn every_group_gets_a_turn_in_ascending_order() {
        let (client, log) = (Client::offline(), Log::default());
        let (late, early, first) = (log.clone(), log.clone(), log.clone());
        client
            .on_message(filters::all(), move |_| {
                late.lock().unwrap().push("group 1");
                async { Ok(()) }
            })
            .group(1);
        client.on_message(filters::command("start"), move |_| {
            first.lock().unwrap().push("group 0");
            async { Ok(()) }
        });
        client
            .on_message(filters::all(), move |_| {
                early.lock().unwrap().push("group -1");
                async { Ok(()) }
            })
            .group(-1);
        dispatch(&router_of(&client), new_message(1, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["group -1", "group 0", "group 1"]);
    }

    #[tokio::test]
    async fn stop_keeps_later_groups_out() {
        let (client, log) = (Client::offline(), Log::default());
        let (guard, rest, other) = (log.clone(), log.clone(), log.clone());
        client.on_message(filters::all(), move |_| {
            rest.lock().unwrap().push("handler");
            async { Ok(()) }
        });
        client
            .on_message(filters::outgoing(), move |_| {
                other.lock().unwrap().push("outgoing");
                async { Ok(()) }
            })
            .group(-2)
            .stop();
        client
            .on_message(filters::command("start"), move |_| {
                guard.lock().unwrap().push("guard");
                async { Ok(()) }
            })
            .stop()
            .group(-1);
        dispatch(&router_of(&client), new_message(1, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["guard"]);
    }

    #[tokio::test]
    async fn an_error_stops_later_groups_and_reaches_on_error() {
        let (client, log) = (Client::offline(), Log::default());
        let errors = Arc::new(Mutex::new(Vec::new()));
        let seen = errors.clone();
        client.on_error(move |e| {
            seen.lock().unwrap().push((e.kind, e.chat_id, e.error.to_string()));
            async {}
        });
        client.0.handlers.add(|r| {
            let (a, b) = (r.next_id(), r.next_id());
            r.message.push(a, filters::all(), fail(&log, "guard"));
            r.message.push(b, filters::all(), record(&log, "late"));
            r.configure(UpdateKind::Message, b, Some(1), false);
        });
        let failing = log.clone();
        client.on_raw_update(move |_, _| {
            failing.lock().unwrap().push("raw");
            async { Err::<(), _>(Error::Unexpected("raw")) }
        });
        dispatch(&router_of(&client), new_message(1, false));
        settle().await;
        let mut ran = log.lock().unwrap().clone();
        ran.sort_unstable();
        assert_eq!(ran, ["guard", "raw"]);
        let mut errors = errors.lock().unwrap().clone();
        errors.sort_by_key(|e| e.1);
        let guard = Error::Unexpected("guard").to_string();
        let raw = Error::Unexpected("raw").to_string();
        assert_eq!(errors, [(UpdateKind::Raw, None, raw), (UpdateKind::Message, Some(1), guard)]);
    }

    async fn second_finishes_first(ordered: bool) -> bool {
        let (client, log) = (Client::offline(), Log::default());
        let gate = Arc::new(tokio::sync::Notify::new());
        let (wait, done) = (gate.clone(), log.clone());
        client.on_message(filters::all(), move |m| {
            let (wait, done) = (wait.clone(), done.clone());
            async move {
                if m.id == 1 {
                    wait.notified().await;
                }
                done.lock().unwrap().push(if m.id == 1 { "first" } else { "second" });
                Ok(())
            }
        });
        let (router, mut tasks) = (router_of(&client), Tasks::new(ordered, None));
        for id in [1, 2] {
            router.dispatch(&client, new_message(id, false), &Arc::default(), &mut tasks);
        }
        settle().await;
        gate.notify_one();
        settle().await;
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 2);
        log[0] == "second"
    }

    #[tokio::test]
    async fn ordered_runs_a_chat_in_arrival_order() {
        assert!(second_finishes_first(false).await);
        assert!(!second_finishes_first(true).await);
    }

    fn panics(events: &std::sync::mpsc::Receiver<ClientEvent>) -> Vec<(UpdateKind, Option<i64>, Option<String>)> {
        let panicked = |e| match e {
            ClientEvent::HandlerPanicked { kind, chat_id, message } => Some((kind, chat_id, message)),
            _ => None,
        };
        events.try_iter().filter_map(panicked).collect()
    }

    #[tokio::test]
    async fn a_panicking_handler_is_reported_and_frees_its_chat_and_slot() {
        let (client, log) = (Client::offline(), Log::default());
        let done = log.clone();
        client.on_message(filters::all(), move |m| {
            let done = done.clone();
            async move {
                tokio::task::yield_now().await;
                assert!(m.id != 1, "boom");
                done.lock().unwrap().push("second");
                Ok(())
            }
        });
        // One lane and one slot: the second update runs only once the panicked handler gave both back.
        let (router, mut tasks) = (router_of(&client), Tasks::new(true, Some(Arc::new(Semaphore::new(1)))));
        let events = tasks.events.record();
        for id in [1, 2] {
            router.dispatch(&client, new_message(id, false), &Arc::default(), &mut tasks);
        }
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["second"]);
        assert_eq!(panics(&events), [(UpdateKind::Message, Some(1), Some("boom".to_owned()))]);
    }

    #[tokio::test]
    async fn a_panic_on_the_dispatcher_is_reported_and_the_update_goes_on() {
        let (client, log) = (Client::offline(), Log::default());
        client.on_raw_update(|_, _| -> std::future::Ready<crate::Result<()>> { panic!("raw boom") });
        let first = log.clone();
        client.on_raw_update(move |_, _| {
            first.lock().unwrap().push("raw");
            async { Ok(()) }
        });
        let filter = filters::create(|m: &Message| -> bool { panic!("filter {}", m.id) });
        client.on_message(filter, |_| async { Ok(()) });
        let after = log.clone();
        client
            .on_message(filters::all(), move |_| {
                after.lock().unwrap().push("message");
                async { Ok(()) }
            })
            .group(1);
        let (router, mut tasks) = (router_of(&client), Tasks::default());
        let events = tasks.events.record();
        router.dispatch(&client, new_message(1, false), &Arc::default(), &mut tasks);
        settle().await;
        let mut ran = log.lock().unwrap().clone();
        ran.sort_unstable();
        assert_eq!(ran, ["message", "raw"]);
        let raw = (UpdateKind::Raw, None, Some("raw boom".to_owned()));
        assert_eq!(panics(&events), [raw, (UpdateKind::Message, Some(1), Some("filter 1".to_owned()))]);
    }

    #[tokio::test]
    async fn the_cap_bounds_running_handlers() {
        let client = Client::offline();
        let (running, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (now, max) = (running.clone(), peak.clone());
        client.on_message(filters::all(), move |_| {
            let (now, max) = (now.clone(), max.clone());
            async move {
                max.fetch_max(now.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                tokio::task::yield_now().await;
                now.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let router = router_of(&client);
        let mut tasks = Tasks::new(false, Some(Arc::new(Semaphore::new(2))));
        for id in 0..6 {
            router.dispatch(&client, new_message(id, false), &Arc::default(), &mut tasks);
        }
        for _ in 0..10 {
            settle().await;
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        assert_eq!(running.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn closures_decide_on_the_parsed_message_in_their_group() {
        let log = Log::default();
        let mut router = Router::default();
        router.message.push(1, filters::command("start"), record(&log, "start"));
        router.message.push(2, filters::create(|m: &Message| m.text() == "/nope"), record(&log, "nope"));
        router.message.push(3, filters::private() & filters::create(|m: &Message| m.id == 1), record(&log, "one"));
        router.message.push(4, filters::all(), record(&log, "all"));
        router.configure(UpdateKind::Message, 2, Some(1), false);
        router.configure(UpdateKind::Message, 3, Some(1), false);
        router.configure(UpdateKind::Message, 4, Some(1), false);
        dispatch(&router, new_message(1, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["start", "one"]);
    }

    #[tokio::test]
    async fn service_filters_match_the_parsed_service_message() {
        let log = Log::default();
        let mut router = Router::default();
        let any = || filters::create(|_: &Message| true);
        router.message.push(1, filters::service() & any(), record(&log, "service"));
        router.message.push(2, !filters::service() & any(), record(&log, "not service"));
        router.message.push(3, filters::create(|_: &Message| false), record(&log, "closure"));
        router.message.push(4, filters::service(), record(&log, "after closure"));
        router.configure(UpdateKind::Message, 2, Some(1), false);
        router.configure(UpdateKind::Message, 3, Some(2), false);
        router.configure(UpdateKind::Message, 4, Some(2), false);
        let title = types::MessageActionChatEditTitle { title: "t".into() };
        let title = service(base::MessageAction::MessageActionChatEditTitle(Box::new(title)));
        let message = base::Message::MessageService(Box::new(title));
        let update = types::UpdateNewMessage { message, pts: 0, pts_count: 0 };
        dispatch(&router, base::Update::UpdateNewMessage(Box::new(update)));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["service", "after closure"]);
    }

    #[tokio::test]
    async fn rich_text_reaches_command_filters() {
        let log = Log::default();
        let mut router = Router::default();
        router.message.push(1, filters::command("start"), record(&log, "start"));
        let heading = base::RichText::TextPlain(Box::new(types::TextPlain { text: "/start".into() }));
        let blocks = vec![base::PageBlock::PageBlockHeading1(Box::new(types::PageBlockHeading1 { text: heading }))];
        let rich = types::RichMessage { rtl: false, part: false, blocks, photos: vec![], documents: vec![] };
        let mut raw = Message::bare(1, get_peer(1), 0);
        raw.rich_message = Some(base::RichMessage::RichMessage(Box::new(rich)));
        let parsed = Message::parse(&Client::offline(), Box::new(raw.clone()), &Entities::default());
        assert!(filters::command("start").check(&parsed));
        let update = types::UpdateNewMessage { message: base::Message::Message(Box::new(raw)), pts: 0, pts_count: 0 };
        dispatch(&router, base::Update::UpdateNewMessage(Box::new(update)));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["start"]);
    }

    #[test]
    fn message_filters_work_on_other_kinds() {
        let client = Client::offline();
        let entities = Entities::default();
        let base::Update::UpdateBotInlineQuery(raw) = inline_query() else { unreachable!() };
        let query = InlineQuery::parse(&client, &raw, &entities);
        assert_eq!((filters::text().check(&query), filters::user(1).check(&query)), (true, true));
        let base::Update::UpdateNewMessage(raw) = new_message(1, false) else { unreachable!() };
        let base::Message::Message(raw) = &raw.message else { unreachable!() };
        let message = Message::parse(&client, raw.clone(), &entities);
        assert_eq!((filters::text().check(&message), filters::private().check(&message)), (true, true));
    }

    fn spawn_listen<T: crate::handlers::Listenable>(
        client: &Client,
        criteria: impl FnOnce(crate::methods::Listen<'_, T>) -> crate::methods::Listen<'_, T> + Send + 'static,
    ) -> tokio::task::JoinHandle<crate::errors::Result<T>> {
        let client = client.clone();
        tokio::spawn(async move { criteria(client.listen::<T>()).await })
    }

    #[tokio::test]
    async fn a_listener_takes_the_message_before_every_group() {
        let (client, log) = (Client::offline(), Log::default());
        client.0.handlers.add(|r| {
            r.message.push(1, filters::all(), record(&log, "group -1"));
            r.message.push(2, filters::all(), record(&log, "group 0"));
            r.configure(UpdateKind::Message, 1, Some(-1), false);
            r.raw.push((3, record_raw(&log)));
        });
        let waiter = spawn_listen::<Message>(&client, |l| l.chat(1));
        until_waiting(&client.0.handlers.listeners.messages).await;
        dispatch_on(&client, new_message(7, false));
        settle().await;
        assert_eq!(waiter.await.unwrap().unwrap().id, 7);
        assert_eq!(*log.lock().unwrap(), ["raw"]);
    }

    #[tokio::test]
    async fn an_unmatched_message_still_reaches_handlers() {
        let (client, log) = (Client::offline(), Log::default());
        client.0.handlers.add(|r| r.message.push(1, filters::all(), record(&log, "handler")));
        let waiter = spawn_listen::<Message>(&client, |l| l.filter(filters::create(|m: &Message| m.text() == "yes")));
        until_waiting(&client.0.handlers.listeners.messages).await;
        dispatch_on(&client, new_message(7, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["handler"]);
        assert!(client.0.handlers.listeners.messages.waiting() && !waiter.is_finished());
        waiter.abort();
    }

    #[tokio::test]
    async fn listeners_see_messages_with_no_message_handlers() {
        let client = Client::offline();
        let waiter = spawn_listen::<Message>(&client, |l| l);
        until_waiting(&client.0.handlers.listeners.messages).await;
        dispatch_on(&client, new_message(7, false));
        assert_eq!(waiter.await.unwrap().unwrap().id, 7);
    }

    #[tokio::test]
    async fn every_callback_origin_reaches_callback_listeners() {
        let (client, log) = (Client::offline(), Log::default());
        client.0.handlers.add(|r| r.callback_query.push(1, filters::all(), record(&log, "handler")));
        let business = types::UpdateBusinessBotCallbackQuery {
            query_id: 1,
            user_id: 7,
            connection_id: "conn".into(),
            message: base::Message::Message(Box::new(Message::bare(5, get_peer(7), 0))),
            reply_to_message: None,
            chat_instance: 3,
            data: None,
        };
        let ephemeral = types::UpdateEphemeralBotCallbackQuery {
            query_id: 9,
            user_id: 1000,
            peer: None,
            msg_id: 7,
            data: b"tap".to_vec(),
            chat_instance: None,
            message: ephemeral(true, None),
        };
        let updates = [
            base::Update::UpdateBotCallbackQuery(Box::new(raw_query(7, 7, 2))),
            base::Update::UpdateInlineBotCallbackQuery(Box::new(raw_inline_query(7, 1))),
            base::Update::UpdateBusinessBotCallbackQuery(Box::new(business)),
            base::Update::UpdateEphemeralBotCallbackQuery(Box::new(ephemeral)),
        ];
        for (i, update) in updates.into_iter().enumerate() {
            let waiter = spawn_listen::<CallbackQuery>(&client, |l| l);
            until_waiting(&client.0.handlers.listeners.callbacks).await;
            dispatch_on(&client, update);
            assert!(waiter.await.unwrap().is_ok(), "origin {i}");
        }
        settle().await;
        assert!(log.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn edited_messages_never_reach_listeners() {
        let client = Client::offline();
        let waiter = spawn_listen::<Message>(&client, |l| l);
        until_waiting(&client.0.handlers.listeners.messages).await;
        let base::Update::UpdateNewMessage(new) = new_message(7, false) else { unreachable!() };
        let edit = types::UpdateEditMessage { message: new.message, pts: 0, pts_count: 0 };
        dispatch_on(&client, base::Update::UpdateEditMessage(Box::new(edit)));
        settle().await;
        assert!(client.0.handlers.listeners.messages.waiting() && !waiter.is_finished());
        waiter.abort();
    }

    #[tokio::test]
    async fn a_stranger_click_is_answered_and_skips_handlers() {
        let ((client, mut sent), log) = (Client::recording(), Log::default());
        client.0.handlers.add(|r| r.callback_query.push(1, filters::all(), record(&log, "handler")));
        let waiter = spawn_listen::<CallbackQuery>(&client, |l| l.user(8).message_id(2));
        until_waiting(&client.0.handlers.listeners.callbacks).await;
        dispatch_on(&client, base::Update::UpdateBotCallbackQuery(Box::new(raw_query(9, 7, 2))));
        settle().await;
        let answer = functions::messages::SetBotCallbackAnswer {
            alert: true,
            query_id: 1,
            message: Some("You are not expected to click this button.".into()),
            url: None,
            cache_time: 0,
        };
        assert_eq!(sent.try_recv().expect("no answer sent"), answer.to_bytes());
        assert!(log.lock().unwrap().is_empty());
        assert!(!waiter.is_finished());
        waiter.abort();
    }

    #[tokio::test]
    async fn a_next_step_runs_through_tasks_in_order() {
        let (client, log) = (Client::offline(), Log::default());
        let gate = Arc::new(tokio::sync::Notify::new());
        let (held, handler_log) = (gate.clone(), log.clone());
        client.on_message(filters::all(), move |_| {
            let (held, log) = (held.clone(), handler_log.clone());
            async move {
                held.notified().await;
                log.lock().unwrap().push("handler");
                Ok(())
            }
        });
        let next_log = log.clone();
        let next = move |_: Message| {
            next_log.lock().unwrap().push("next");
            async { Ok(()) }
        };
        let mut tasks = Tasks::new(true, None);
        router_of(&client).dispatch(&client, new_message(1, false), &Arc::default(), &mut tasks);
        settle().await;
        client.register_next_step_handler(next).chat(1).await.unwrap();
        router_of(&client).dispatch(&client, new_message(2, false), &Arc::default(), &mut tasks);
        settle().await;
        assert!(log.lock().unwrap().is_empty(), "the next step overtook the running handler of its chat");
        gate.notify_one();
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["handler", "next"]);
    }

    fn story_update(peer: base::Peer, item: Option<types::StoryItem>) -> base::Update {
        let story = item.map_or(base::StoryItem::StoryItemDeleted(types::StoryItemDeleted { id: 7 }), |item| {
            base::StoryItem::StoryItem(Box::new(item))
        });
        base::Update::UpdateStory(Box::new(types::UpdateStory { peer, story }))
    }

    fn story_item() -> types::StoryItem {
        types::StoryItem {
            pinned: false,
            public: true,
            close_friends: false,
            min: false,
            noforwards: false,
            edited: false,
            contacts: false,
            selected_contacts: false,
            out: false,
            id: 7,
            date: 100,
            from_id: None,
            fwd_from: None,
            expire_date: 200,
            caption: None,
            entities: None,
            media: base::MessageMedia::MessageMediaEmpty,
            media_areas: None,
            privacy: None,
            views: None,
            sent_reaction: None,
            albums: None,
            music: None,
        }
    }

    fn broadcast(id: i64) -> Entities {
        let mut buf = Vec::new();
        for word in [1u32 << 5, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        "c".to_owned().serialize(&mut buf);
        base::ChatPhoto::ChatPhotoEmpty.serialize(&mut buf);
        0i32.serialize(&mut buf);
        let channel = Box::new(types::Channel::deserialize_bare(&mut &buf[..]).unwrap());
        Entities { users: Default::default(), chats: [(get_channel_id(id), base::Chat::Channel(channel))].into() }
    }

    #[tokio::test]
    async fn stories_reach_story_handlers() {
        let log = Log::default();
        let mut router = Router::default();
        router.story.push(1, filters::all(), record(&log, "story"));
        dispatch(&router, story_update(get_peer(8), Some(story_item())));
        dispatch(&router, story_update(get_peer(8), None));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["story"]);
        assert!(router.remove(UpdateKind::Story, 1));
    }

    #[tokio::test]
    async fn story_filters_see_the_chat() {
        let log = Log::default();
        let mut router = Router::default();
        router.story.push(1, filters::channel(), record(&log, "channel"));
        let (client, entities) = (Client::offline(), Arc::new(broadcast(9)));
        for peer in [get_peer(get_channel_id(9)), get_peer(8)] {
            router.dispatch(&client, story_update(peer, Some(story_item())), &entities, &mut Tasks::default());
        }
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["channel"]);
    }

    #[tokio::test]
    async fn business_messages_carry_the_business_bit() {
        let log = Log::default();
        let mut router = Router::default();
        router.business_message.push(1, filters::business(), record(&log, "business"));
        let parsed = filters::create(|_: &Message| true) & filters::business();
        router.business_message.push(2, parsed, record(&log, "parsed"));
        router.business_message.configure(2, Some(1), false);
        router.message.push(3, filters::business(), record(&log, "plain"));
        let message = base::Message::Message(Box::new(Message::bare(1, get_peer(1), 0)));
        let update = types::UpdateBotNewBusinessMessage {
            connection_id: "conn".into(),
            message,
            reply_to_message: None,
            qts: 0,
        };
        dispatch(&router, base::Update::UpdateBotNewBusinessMessage(Box::new(update)));
        dispatch(&router, new_message(2, false));
        settle().await;
        let mut seen = log.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, ["business", "parsed"]);
    }

    #[tokio::test]
    async fn user_reaction_updates_reach_on_reactions() {
        let (client, seen) = (Client::offline(), Arc::new(Mutex::new(Vec::new())));
        let chat = get_channel_id(42);
        let sink = seen.clone();
        client.on_reactions(filters::chat(chat), move |r| {
            let counts: Vec<_> = r.reactions.reactions.iter().map(|c| c.total_count).collect();
            sink.lock().unwrap().push((r.chat.id, r.message_id, r.message_thread_id, counts));
            async { Ok(()) }
        });
        let update = |peer| {
            let reaction = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "👍".into() }));
            let count = types::ReactionCount { chosen_order: None, reaction, count: 2 };
            let reactions = types::MessageReactions {
                min: false,
                can_see_list: false,
                reactions_as_tags: false,
                results: vec![base::ReactionCount::ReactionCount(Box::new(count))],
                recent_reactions: None,
                top_reactors: None,
            };
            base::Update::UpdateMessageReactions(Box::new(types::UpdateMessageReactions {
                peer,
                msg_id: 10,
                top_msg_id: Some(3),
                saved_peer_id: None,
                reactions: base::MessageReactions::MessageReactions(Box::new(reactions)),
            }))
        };
        let router = router_of(&client);
        dispatch(&router, update(get_peer(8)));
        dispatch(&router, update(get_peer(chat)));
        settle().await;
        assert_eq!(*seen.lock().unwrap(), [(chat, 10, Some(3), vec![2])]);
    }

    #[tokio::test]
    async fn pins_in_any_chat_reach_on_pinned_messages() {
        let (client, seen) = (Client::offline(), Arc::new(Mutex::new(Vec::new())));
        let sink = seen.clone();
        client.on_pinned_messages(filters::all(), move |p| {
            sink.lock().unwrap().push((p.chat.id, p.message_ids, p.is_pinned));
            async { Ok(()) }
        });
        let router = router_of(&client);
        let pinned =
            types::UpdatePinnedMessages { pinned: true, peer: get_peer(7), messages: vec![1], pts: 1, pts_count: 1 };
        dispatch(&router, base::Update::UpdatePinnedMessages(Box::new(pinned)));
        let unpinned = types::UpdatePinnedChannelMessages {
            pinned: false,
            channel_id: 42,
            messages: vec![2, 3],
            pts: 1,
            pts_count: 2,
        };
        dispatch(&router, base::Update::UpdatePinnedChannelMessages(Box::new(unpinned)));
        settle().await;
        let mut seen = seen.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, [(get_channel_id(42), vec![2, 3], false), (7, vec![1], true)]);
    }

    #[tokio::test]
    async fn read_receipts_of_every_kind_reach_on_read_history() {
        let (client, seen) = (Client::offline(), Arc::new(Mutex::new(Vec::new())));
        let sink = seen.clone();
        client.on_read_history(filters::all(), move |r| {
            sink.lock().unwrap().push((r.chat.id, r.max_id, r.is_outgoing, r.message_thread_id, r.unread_count));
            async { Ok(()) }
        });
        let router = router_of(&client);
        let inbox = types::UpdateReadHistoryInbox {
            folder_id: None,
            peer: get_peer(7),
            top_msg_id: None,
            max_id: 5,
            still_unread_count: 2,
            pts: 1,
            pts_count: 1,
        };
        dispatch(&router, base::Update::UpdateReadHistoryInbox(Box::new(inbox)));
        let outbox = types::UpdateReadHistoryOutbox { peer: get_peer(8), max_id: 6, pts: 2, pts_count: 1 };
        dispatch(&router, base::Update::UpdateReadHistoryOutbox(Box::new(outbox)));
        let channel =
            types::UpdateReadChannelInbox { folder_id: None, channel_id: 42, max_id: 7, still_unread_count: 0, pts: 3 };
        dispatch(&router, base::Update::UpdateReadChannelInbox(Box::new(channel)));
        let channel = types::UpdateReadChannelOutbox { channel_id: 42, max_id: 8 };
        dispatch(&router, base::Update::UpdateReadChannelOutbox(Box::new(channel)));
        let thread = types::UpdateReadChannelDiscussionInbox {
            channel_id: 43,
            top_msg_id: 3,
            read_max_id: 9,
            broadcast_id: None,
            broadcast_post: None,
        };
        dispatch(&router, base::Update::UpdateReadChannelDiscussionInbox(Box::new(thread)));
        let thread = types::UpdateReadChannelDiscussionOutbox { channel_id: 43, top_msg_id: 3, read_max_id: 10 };
        dispatch(&router, base::Update::UpdateReadChannelDiscussionOutbox(Box::new(thread)));
        settle().await;
        let mut seen = seen.lock().unwrap().clone();
        seen.sort_unstable_by_key(|r| r.1);
        let (a, b) = (get_channel_id(42), get_channel_id(43));
        assert_eq!(
            seen,
            [
                (7, 5, false, None, Some(2)),
                (8, 6, true, None, None),
                (a, 7, false, None, Some(0)),
                (a, 8, true, None, None),
                (b, 9, false, Some(3), None),
                (b, 10, true, Some(3), None),
            ]
        );
    }

    #[tokio::test]
    async fn join_request_counts_reach_on_pending_join_requests() {
        let (client, seen) = (Client::offline(), Arc::new(Mutex::new(Vec::new())));
        let chat = get_channel_id(42);
        let sink = seen.clone();
        client.on_pending_join_requests(filters::chat(chat), move |r| {
            let requesters: Vec<_> = r.recent_requesters.iter().map(|u| u.id).collect();
            sink.lock().unwrap().push((r.chat.id, r.pending_count, requesters));
            async { Ok(()) }
        });
        let router = router_of(&client);
        for peer in [get_peer(-5), get_peer(chat)] {
            let update = types::UpdatePendingJoinRequests { peer, requests_pending: 2, recent_requesters: vec![7, 8] };
            dispatch(&router, base::Update::UpdatePendingJoinRequests(Box::new(update)));
        }
        settle().await;
        assert_eq!(*seen.lock().unwrap(), [(chat, 2, vec![7, 8])]);
    }

    #[tokio::test]
    async fn typing_goes_to_on_chat_action_and_stopped_drafts_to_their_own_kind() {
        let (client, log) = (Client::offline(), Log::default());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        client.on_chat_action(filters::user(7), move |a| {
            let sender = a.from_user.map(|u| u.id);
            sink.lock().unwrap().push((a.chat.id, sender, a.action, a.progress, a.message_thread_id));
            async { Ok(()) }
        });
        let stopped = log.clone();
        client.on_message_generation_stopped(filters::all(), move |_| {
            stopped.lock().unwrap().push("stopped");
            async { Ok(()) }
        });
        let router = router_of(&client);
        let user = |action| {
            base::Update::UpdateUserTyping(Box::new(types::UpdateUserTyping { user_id: 7, top_msg_id: None, action }))
        };
        dispatch(&router, user(base::SendMessageAction::SendMessageTypingAction));
        let stop = types::SendMessageStopDraftAction { random_id: 1 };
        dispatch(&router, user(base::SendMessageAction::SendMessageStopDraftAction(stop)));
        let upload = types::SendMessageUploadPhotoAction { progress: 40 };
        let channel = types::UpdateChannelUserTyping {
            channel_id: 42,
            top_msg_id: Some(3),
            from_id: get_peer(7),
            action: base::SendMessageAction::SendMessageUploadPhotoAction(upload),
        };
        dispatch(&router, base::Update::UpdateChannelUserTyping(Box::new(channel)));
        let group = types::UpdateChatUserTyping {
            chat_id: 5,
            from_id: get_peer(8),
            action: base::SendMessageAction::SendMessageTypingAction,
        };
        dispatch(&router, base::Update::UpdateChatUserTyping(Box::new(group)));
        settle().await;
        let mut seen = seen.lock().unwrap().clone();
        seen.sort_unstable_by_key(|a| a.0);
        let channel = get_channel_id(42);
        assert_eq!(
            seen,
            [
                (channel, Some(7), crate::enums::ChatAction::UploadPhoto, Some(40), Some(3)),
                (7, Some(7), crate::enums::ChatAction::Typing, None, None),
            ]
        );
        assert_eq!(*log.lock().unwrap(), ["stopped"]);
    }

    #[tokio::test]
    async fn balance_changes_reach_on_stars_balance() {
        let (client, seen) = (Client::offline(), Arc::new(Mutex::new(Vec::new())));
        let sink = seen.clone();
        client.on_stars_balance(filters::all(), move |b| {
            sink.lock().unwrap().push(b.balance);
            async { Ok(()) }
        });
        let balance = base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 5, nanos: 0 }));
        let update = base::Update::UpdateStarsBalance(Box::new(types::UpdateStarsBalance { balance }));
        dispatch(&router_of(&client), update);
        settle().await;
        assert_eq!(*seen.lock().unwrap(), [crate::types::StarAmount::new(5)]);
    }
}
