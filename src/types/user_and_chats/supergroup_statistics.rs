use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{StatisticalGraph, StatisticalValue, User};

#[derive(Debug, Clone, PartialEq)]
pub struct SupergroupStatistics {
    pub min_date: i32,
    pub max_date: i32,
    pub members: StatisticalValue,
    pub messages: StatisticalValue,
    pub viewers: StatisticalValue,
    pub posters: StatisticalValue,
    pub growth_graph: StatisticalGraph,
    pub members_graph: StatisticalGraph,
    pub new_members_by_source_graph: StatisticalGraph,
    pub languages_graph: StatisticalGraph,
    pub messages_graph: StatisticalGraph,
    pub actions_graph: StatisticalGraph,
    pub top_hours_graph: StatisticalGraph,
    pub weekdays_graph: StatisticalGraph,
    pub top_posters: Vec<TopPoster>,
    pub top_admins: Vec<TopAdmin>,
    pub top_inviters: Vec<TopInviter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopPoster {
    pub user: User,
    pub messages: i32,
    pub avg_chars: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopAdmin {
    pub user: User,
    pub deleted: i32,
    pub kicked: i32,
    pub banned: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopInviter {
    pub user: User,
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
