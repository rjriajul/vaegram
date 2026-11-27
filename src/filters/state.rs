use std::ops::{BitAnd, BitOr, Not};
use std::sync::Arc;

/// A test on a dialogue's state, for the steps of a [`DialogueRouter`](crate::dialogue::DialogueRouter). It sees
/// `None` when the conversation has no state: before it starts, after [`clear`](crate::dialogue::Dialogue::clear)
/// and once it timed out. Combine them with `&`, `|` and `!`.
///
/// Unlike a [`Filter`](super::Filter), it runs on the state the dialogue loaded for the update, not on the update.
pub struct StateFilter<S>(Arc<Check<S>>);

type Check<S> = dyn Fn(Option<&S>) -> bool + Send + Sync;

impl<S> Clone for StateFilter<S> {
    fn clone(&self) -> Self {
        StateFilter(Arc::clone(&self.0))
    }
}

impl<S> StateFilter<S> {
    /// Tests `state`, `None` standing for no state.
    pub fn check(&self, state: Option<&S>) -> bool {
        (self.0)(state)
    }
}

/// A state that `f` accepts; never the absence of one. `filters::state(|s: &Signup| matches!(s, Signup::Age { .. }))`
/// matches every `Age` state, whatever it holds.
pub fn state<S: 'static>(f: impl Fn(&S) -> bool + Send + Sync + 'static) -> StateFilter<S> {
    StateFilter(Arc::new(move |state: Option<&S>| state.is_some_and(&f)))
}

/// A state equal to `value`.
pub fn state_is<S: PartialEq + Send + Sync + 'static>(value: S) -> StateFilter<S> {
    StateFilter(Arc::new(move |state: Option<&S>| state == Some(&value)))
}

/// Any state: the conversation is under way. Put a `/cancel` step with it first, so it wins over the other steps.
pub fn any_state<S: 'static>() -> StateFilter<S> {
    StateFilter(Arc::new(|state: Option<&S>| state.is_some()))
}

/// No state: the conversation hasn't started, was cleared or timed out. The step that starts a conversation uses it.
pub fn no_state<S: 'static>() -> StateFilter<S> {
    StateFilter(Arc::new(|state: Option<&S>| state.is_none()))
}

impl<S: 'static> BitAnd for StateFilter<S> {
    type Output = StateFilter<S>;

    fn bitand(self, other: Self) -> Self {
        StateFilter(Arc::new(move |state: Option<&S>| self.check(state) && other.check(state)))
    }
}

impl<S: 'static> BitOr for StateFilter<S> {
    type Output = StateFilter<S>;

    fn bitor(self, other: Self) -> Self {
        StateFilter(Arc::new(move |state: Option<&S>| self.check(state) || other.check(state)))
    }
}

impl<S: 'static> Not for StateFilter<S> {
    type Output = StateFilter<S>;

    fn not(self) -> Self {
        StateFilter(Arc::new(move |state: Option<&S>| !self.check(state)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum Signup {
        Name,
        Age { name: String },
    }

    #[test]
    fn state_matches_only_an_accepted_state() {
        let age = state(|s: &Signup| matches!(s, Signup::Age { .. }));
        assert!(age.check(Some(&Signup::Age { name: "Ann".into() })));
        assert!(!age.check(Some(&Signup::Name)));
        assert!(!age.check(None));
    }

    #[test]
    fn state_is_compares_the_whole_value() {
        let ann = state_is(Signup::Age { name: "Ann".into() });
        assert!(ann.check(Some(&Signup::Age { name: "Ann".into() })));
        assert!(!ann.check(Some(&Signup::Age { name: "Bob".into() })));
        assert!(!ann.check(None));
    }

    #[test]
    fn any_and_no_state_split_on_presence() {
        assert!(any_state::<Signup>().check(Some(&Signup::Name)));
        assert!(!any_state::<Signup>().check(None));
        assert!(no_state::<Signup>().check(None));
        assert!(!no_state::<Signup>().check(Some(&Signup::Name)));
    }

    #[test]
    fn operators_combine_state_filters() {
        let name = state_is(Signup::Name);
        let either = name.clone() | no_state();
        assert!(either.check(None) && either.check(Some(&Signup::Name)));
        assert!(!either.check(Some(&Signup::Age { name: "Ann".into() })));
        let neither = !either;
        assert!(neither.check(Some(&Signup::Age { name: "Ann".into() })));
        let both = name & any_state();
        assert!(both.check(Some(&Signup::Name)) && !both.check(None));
    }
}
