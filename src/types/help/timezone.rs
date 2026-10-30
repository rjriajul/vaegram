#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timezone {
    pub id: String,
    pub name: String,
    pub utc_offset_seconds: i32,
}
