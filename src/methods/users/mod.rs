mod delete_profile_photos;
mod get_chat_photos;
mod get_me;
mod get_users;
mod remove_profile_photo;
mod set_passport_data_errors;
mod set_profile_photo;

pub use get_chat_photos::GetChatPhotos;
pub use set_profile_photo::SetProfilePhoto;
pub(crate) use set_profile_photo::uploaded;
