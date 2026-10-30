mod get_lang_pack;
mod get_lang_pack_difference;
mod get_language;
mod get_languages;
mod get_strings;

#[cfg(test)]
mod tests {
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::{LangPackDifference, LangPackString};
    use crate::{Client, Error};

    #[tokio::test]
    async fn keeps_lang_pack_and_lang_code_apart() {
        let language = types::LangPackLanguage {
            official: true,
            rtl: false,
            beta: false,
            name: "English".into(),
            native_name: "English".into(),
            lang_code: "en".into(),
            base_lang_code: None,
            plural_code: "en".into(),
            strings_count: 2,
            translated_count: 2,
            translations_url: "https://translations.telegram.org/en/".into(),
        };
        let ok = types::LangPackString { key: "Ok".into(), value: "OK".into() };
        let files = types::LangPackStringPluralized {
            key: "Files".into(),
            zero_value: None,
            one_value: Some("%d file".into()),
            two_value: None,
            few_value: None,
            many_value: None,
            other_value: "%d files".into(),
        };
        let gone = types::LangPackStringDeleted { key: "Old".into() };
        let strings = vec![
            base::LangPackString::LangPackString(Box::new(ok)),
            base::LangPackString::LangPackStringPluralized(Box::new(files)),
            base::LangPackString::LangPackStringDeleted(Box::new(gone)),
        ];
        let diff =
            types::LangPackDifference { lang_code: "en".into(), from_version: 3, version: 4, strings: strings.clone() };
        let (client, mut sent) = Client::scripted(vec![
            Ok(base::LangPackLanguage::LangPackLanguage(Box::new(language)).to_bytes()),
            Ok(strings.to_bytes()),
            Ok(base::LangPackDifference::LangPackDifference(Box::new(diff)).to_bytes()),
        ]);

        let en = client.get_language("android", "en").await.unwrap();
        assert_eq!((en.lang_code.as_str(), en.base_lang_code, en.official), ("en", None, true));
        let strings = vec![
            LangPackString::Plain { key: "Ok".into(), value: "OK".into() },
            LangPackString::Pluralized {
                key: "Files".into(),
                zero_value: None,
                one_value: Some("%d file".into()),
                two_value: None,
                few_value: None,
                many_value: None,
                other_value: "%d files".into(),
            },
            LangPackString::Deleted { key: "Old".into() },
        ];
        let got = client.get_strings("android", "en", ["Ok"]).await.unwrap();
        assert_eq!((&got, got[2].key()), (&strings, "Old"));
        let diff = LangPackDifference { lang_code: "en".into(), from_version: 3, version: 4, strings };
        assert_eq!(client.get_lang_pack_difference("android", "en", 3).await.unwrap(), diff);

        let (lang_pack, lang_code) = (String::from("android"), String::from("en"));
        let language = functions::langpack::GetLanguage { lang_pack: lang_pack.clone(), lang_code: lang_code.clone() };
        assert_eq!(sent.recv().await.unwrap(), language.to_bytes());
        let (pack, code, keys) = (lang_pack.clone(), lang_code.clone(), vec!["Ok".into()]);
        let get_strings = functions::langpack::GetStrings { lang_pack: pack, lang_code: code, keys };
        assert_eq!(sent.recv().await.unwrap(), get_strings.to_bytes());
        let difference = functions::langpack::GetDifference { lang_pack, lang_code, from_version: 3 };
        assert_eq!(sent.recv().await.unwrap(), difference.to_bytes());
    }

    #[tokio::test]
    async fn no_keys_are_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        let none: [&str; 0] = [];
        assert!(matches!(client.get_strings("android", "en", none).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
