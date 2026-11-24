use std::cell::RefCell;
use std::sync::Arc;

use serde::{Serialize, Serializer};

/// A file to send: one Telegram already has, one it fetches from a URL, or bytes uploaded with the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputFile {
    /// The `file_id` of a file Telegram already has.
    FileId(String),
    /// An HTTP URL Telegram downloads the file from.
    Url(String),
    /// Bytes uploaded with the request, under this file name.
    Upload {
        /// The file name Telegram shows.
        name: String,
        /// The file's contents.
        data: Arc<[u8]>,
    },
}

impl InputFile {
    /// Bytes to upload under `name`.
    pub fn upload(name: impl Into<String>, data: impl Into<Arc<[u8]>>) -> Self {
        InputFile::Upload { name: name.into(), data: data.into() }
    }
}

/// An upload taken out of a request while it was serialized: its `attach://` key, file name and bytes.
pub(crate) type Attachment = (String, String, Arc<[u8]>);

thread_local! {
    // Serialization is synchronous, so the uploads of one request are collected on the thread serializing it.
    static ATTACHMENTS: RefCell<Option<Vec<Attachment>>> = const { RefCell::new(None) };
}

/// Serializes `value` to JSON, replacing every upload with `attach://<key>` and returning the uploads.
pub(crate) fn collect<T: Serialize>(value: &T) -> serde_json::Result<(serde_json::Value, Vec<Attachment>)> {
    ATTACHMENTS.with(|a| *a.borrow_mut() = Some(Vec::new()));
    let json = serde_json::to_value(value);
    let uploads = ATTACHMENTS.with(|a| a.borrow_mut().take()).unwrap_or_default();
    Ok((json?, uploads))
}

impl Serialize for InputFile {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            InputFile::FileId(s) | InputFile::Url(s) => serializer.serialize_str(s),
            InputFile::Upload { name, data } => {
                let key = ATTACHMENTS.with(|a| -> Result<String, S::Error> {
                    let mut a = a.borrow_mut();
                    // Outside `collect` (plain `serde_json::to_string`) there is nowhere to put the bytes.
                    let refused = || <S::Error as serde::ser::Error>::custom("an upload can't be serialized here");
                    let list = a.as_mut().ok_or_else(refused)?;
                    let key = format!("file{}", list.len());
                    list.push((key.clone(), name.clone(), data.clone()));
                    Ok(key)
                })?;
                serializer.serialize_str(&format!("attach://{key}"))
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for InputFile {
    // Only file ids and URLs come back from JSON; an upload never does.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(if s.starts_with("http://") || s.starts_with("https://") { InputFile::Url(s) } else { InputFile::FileId(s) })
    }
}

#[cfg(test)]
mod tests {
    use super::{InputFile, collect};

    #[derive(serde::Serialize)]
    struct Send {
        chat_id: i64,
        document: InputFile,
        thumbnail: Option<InputFile>,
    }

    #[test]
    fn uploads_become_attachments_and_ids_stay_strings() {
        let send = Send { chat_id: 1, document: InputFile::upload("a.txt", b"hi".to_vec()), thumbnail: None };
        let (json, uploads) = collect(&send).unwrap();
        assert_eq!(json, serde_json::json!({"chat_id": 1, "document": "attach://file0", "thumbnail": null}));
        assert_eq!(uploads.len(), 1);
        assert_eq!((uploads[0].0.as_str(), uploads[0].1.as_str(), &*uploads[0].2), ("file0", "a.txt", &b"hi"[..]));

        let by_id = Send { chat_id: 1, document: InputFile::FileId("AgAD".into()), thumbnail: None };
        let (json, uploads) = collect(&by_id).unwrap();
        assert_eq!((json["document"].as_str(), uploads.len()), (Some("AgAD"), 0));
        assert!(serde_json::to_string(&Send { thumbnail: None, ..send }).is_err(), "an upload outside collect");
    }
}
