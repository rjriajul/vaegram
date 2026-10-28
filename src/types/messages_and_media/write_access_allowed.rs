#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteAccessAllowed {
    pub from_request: bool,
    pub web_app_name: Option<String>,
    pub from_attachment_menu: bool,
}
