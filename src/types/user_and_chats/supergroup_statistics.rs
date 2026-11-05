use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{StatisticalGraph, StatisticalValue, User};

/// Statistics for a supergroup.
#[derive(Debug, Clone, PartialEq)]
pub struct SupergroupStatistics {
    /// Unix time the covered period starts.
    pub min_date: i32,
    /// Unix time the covered period ends.
    pub max_date: i32,
    /// Member count.
    pub members: StatisticalValue,
    /// Messages sent.
    pub messages: StatisticalValue,
    /// Members who viewed messages.
    pub viewers: StatisticalValue,
    /// Members who sent messages.
    pub posters: StatisticalValue,
    /// Member growth.
    pub growth_graph: StatisticalGraph,
    /// Members who joined and left.
    pub members_graph: StatisticalGraph,
    /// New members by where they joined from.
    pub new_members_by_source_graph: StatisticalGraph,
    /// Members by language.
    pub languages_graph: StatisticalGraph,
    /// Messages over time.
    pub messages_graph: StatisticalGraph,
    /// Admin actions over time.
    pub actions_graph: StatisticalGraph,
    /// Activity by hour of the day.
    pub top_hours_graph: StatisticalGraph,
    /// Activity by day of the week.
    pub weekdays_graph: StatisticalGraph,
    /// Members who sent the most messages.
    pub top_posters: Vec<TopPoster>,
    /// Admins who took the most actions.
    pub top_admins: Vec<TopAdmin>,
    /// Members who invited the most people.
    pub top_inviters: Vec<TopInviter>,
}

/// A member ranked by messages sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopPoster {
    pub user: User,
    pub messages: i32,
    /// Average length of their messages, in characters.
    pub avg_chars: i32,
}

/// An admin ranked by actions taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopAdmin {
    pub user: User,
    /// Messages deleted.
    pub deleted: i32,
    /// Members kicked.
    pub kicked: i32,
    /// Members banned.
    pub banned: i32,
}

/// A member ranked by invitations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopInviter {
    pub user: User,
    /// People invited.
    pub invitations: i32,
}

impl SupergroupStatistics {
    pub(crate) fn parse(client: &Client, raw: base::stats::MegagroupStats) -> SupergroupStatistics {
        let base::stats::MegagroupStats::MegagroupStats(s) = raw;
        let s = *s;
        let entities = &Entities::new(client, s.users, Vec::new());
        let base::StatsDateRangeDays::StatsDateRangeDays(period) = s.period;
        let value = StatisticalValue::parse;
        let graph = StatisticalGraph::parse;
        let user = |id| User::parse(id, entities);
        SupergroupStatistics {
            min_date: period.min_date,
            max_date: period.max_date,
            members: value(s.members),
            messages: value(s.messages),
            viewers: value(s.viewers),
            posters: value(s.posters),
            growth_graph: graph(s.growth_graph),
            members_graph: graph(s.members_graph),
            new_members_by_source_graph: graph(s.new_members_by_source_graph),
            languages_graph: graph(s.languages_graph),
            messages_graph: graph(s.messages_graph),
            actions_graph: graph(s.actions_graph),
            top_hours_graph: graph(s.top_hours_graph),
            weekdays_graph: graph(s.weekdays_graph),
            top_posters: s
                .top_posters
                .into_iter()
                .map(|base::StatsGroupTopPoster::StatsGroupTopPoster(p)| TopPoster {
                    user: user(p.user_id),
                    messages: p.messages,
                    avg_chars: p.avg_chars,
                })
                .collect(),
            top_admins: s
                .top_admins
                .into_iter()
                .map(|base::StatsGroupTopAdmin::StatsGroupTopAdmin(a)| TopAdmin {
                    user: user(a.user_id),
                    deleted: a.deleted,
                    kicked: a.kicked,
                    banned: a.banned,
                })
                .collect(),
            top_inviters: s
                .top_inviters
                .into_iter()
                .map(|base::StatsGroupTopInviter::StatsGroupTopInviter(i)| TopInviter {
                    user: user(i.user_id),
                    invitations: i.invitations,
                })
                .collect(),
        }
    }
}
