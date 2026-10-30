use std::str::SplitWhitespace;

/// A command read from message text, borrowed from it: `/ban@MyBot 42 spam` is name `ban`, bot `MyBot`, args `42 spam`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command<'a> {
    pub name: &'a str,
    pub bot: Option<&'a str>,
    rest: &'a str,
}

impl<'a> Command<'a> {
    pub(crate) fn parse(text: &'a str, prefixes: &str) -> Option<Command<'a>> {
        let mut chars = text.chars();
        if !prefixes.contains(chars.next()?) {
            return None;
        }
        let body = chars.as_str();
        let (first, rest) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
        let (name, bot) = match first.split_once('@') {
            Some((name, bot)) => (name, Some(bot)),
            None => (first, None),
        };
        (!name.is_empty()).then_some(Command { name, bot, rest: rest.trim_start() })
    }

    pub(crate) fn is_for(&self, me: Option<&str>) -> bool {
        self.bot.is_none_or(|bot| me.is_some_and(|me| me.eq_ignore_ascii_case(bot)))
    }

    pub fn args(&self) -> SplitWhitespace<'a> {
        self.rest.split_whitespace()
    }

    pub fn arg(&self, i: usize) -> Option<&'a str> {
        self.args().nth(i)
    }

    pub fn rest(&self) -> &'a str {
        self.rest
    }
}

#[cfg(test)]
mod tests {
    use super::Command;

    fn read<'a>(text: &'a str, me: Option<&str>) -> Option<(&'a str, Vec<&'a str>)> {
        let c = Command::parse(text, "/").filter(|c| c.is_for(me))?;
        Some((c.name, c.args().collect()))
    }

    #[test]
    fn parses_commands() {
        assert_eq!(read("/start", None), Some(("start", vec![])));
        assert_eq!(read("/echo  hi there", None), Some(("echo", vec!["hi", "there"])));
        assert_eq!(read("/start@MyBot x", Some("mybot")), Some(("start", vec!["x"])));
        assert_eq!(read("/start@OtherBot", Some("mybot")), None);
        assert_eq!(read("/start@MyBot", None), None);
        assert_eq!(read("start", None), None);
        assert_eq!(read("/", None), None);
        assert_eq!(read("/ start", None), None);
        assert_eq!(read("", None), None);
    }

    #[test]
    fn keeps_the_rest_and_custom_prefixes() {
        let c = Command::parse("!ban  42 spam  ", "!.").unwrap();
        assert_eq!((c.name, c.bot, c.rest(), c.arg(1), c.arg(2)), ("ban", None, "42 spam  ", Some("spam"), None));
        assert_eq!(Command::parse(".ban", "!.").map(|c| c.name), Some("ban"));
        assert_eq!(Command::parse("/ban", "!."), None);
    }

    #[test]
    fn commands_check_the_bot_name_and_split_on_whitespace() {
        assert_eq!(read("/start@mybot", Some("MyBot")), Some(("start", vec![])));
        assert_eq!(read("/start@", Some("mybot")), None);
        assert_eq!(read("/@mybot", Some("mybot")), None);
        assert_eq!(read("/start@mybot@x", Some("mybot")), None);
        assert_eq!(read("/start@mybotx", Some("mybot")), None);
        assert_eq!(read(" /start", None), None);
        assert_eq!(read("/go\tfast\nnow ", None), Some(("go", vec!["fast", "now"])));
        assert_eq!(read("/go@mybot\nx", Some("mybot")), Some(("go", vec!["x"])));
        assert_eq!(read("/пуск арг", None), Some(("пуск", vec!["арг"])));
        assert_eq!(Command::parse("пуск", "п").map(|c| c.name), Some("уск"));
    }
}
