// https://core.telegram.org/cdn

use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};

use super::transfer::{invoke_cdn_part, invoke_part};
use super::{Session, init_connection};
use crate::connection::Options;
use crate::crypto::{aes, sha256};
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};

const MAX_REUPLOADS: usize = 3;

pub(crate) struct Cdn<'a> {
    pub(crate) dc_id: i32,
    master: &'a Session,
    token: Vec<u8>,
    key: [u8; 32],
    iv: [u8; 16],
    hashes: RwLock<HashMap<i64, (usize, [u8; 32])>>,
    init: (i32, &'a Options),
    inited: AtomicBool,
}

impl<'a> Cdn<'a> {
    pub(crate) fn new(
        master: &'a Session,
        redirect: types::upload::FileCdnRedirect,
        init: (i32, &'a Options),
    ) -> Result<Self> {
        let bad = |_| Error::Unexpected("upload.fileCdnRedirect has a key or iv of the wrong size");
        let cdn = Cdn {
            dc_id: redirect.dc_id,
            master,
            token: redirect.file_token,
            key: redirect.encryption_key.try_into().map_err(bad)?,
            iv: redirect.encryption_iv.try_into().map_err(bad)?,
            hashes: RwLock::default(),
            init,
            inited: AtomicBool::new(false),
        };
        cdn.add(redirect.file_hashes);
        Ok(cdn)
    }

    pub(crate) async fn part(&self, session: &Session, offset: i64, limit: i32) -> Result<Vec<u8>> {
        let mut reuploads = 0;
        let mut bytes = loop {
            let request = functions::upload::GetCdnFile { file_token: self.token.clone(), offset, limit };
            // The server keeps initConnection with the auth key, which all of this download's CDN connections share.
            let file = if self.inited.swap(true, Ordering::AcqRel) {
                invoke_cdn_part(session, &request).await?
            } else {
                let (api_id, options) = self.init;
                let mut init = init_connection(api_id, options, request);
                // The CDN gets no private info (https://core.telegram.org/cdn): no device, no MTProxy address.
                (init.query.device_model, init.query.system_version) = ("n/a".into(), "n/a".into());
                init.query.proxy = None;
                invoke_cdn_part(session, &init).await?
            };
            match file {
                base::upload::CdnFile::CdnFile(f) => break f.bytes,
                base::upload::CdnFile::CdnFileReuploadNeeded(r) if reuploads < MAX_REUPLOADS => {
                    let reupload = functions::upload::ReuploadCdnFile {
                        file_token: self.token.clone(),
                        request_token: r.request_token,
                    };
                    self.add(invoke_part(self.master, &reupload).await?);
                    reuploads += 1;
                }
                base::upload::CdnFile::CdnFileReuploadNeeded(_) => {
                    return Err(Error::Unexpected("the CDN keeps asking for upload.reuploadCdnFile"));
                }
            }
        };
        if bytes.len() > limit as usize {
            return Err(Error::Unexpected("upload.getCdnFile returned more than the limit"));
        }
        decrypt(&mut bytes, &self.key, &self.iv, offset);

        let mut checked = 0;
        loop {
            checked += verified(&self.hashes.read().unwrap(), offset + checked as i64, &bytes[checked..])?;
            if checked == bytes.len() {
                break;
            }
            let at = offset + checked as i64;
            let request = functions::upload::GetCdnFileHashes { file_token: self.token.clone(), offset: at };
            self.add(invoke_part(self.master, &request).await?);
            if !self.hashes.read().unwrap().contains_key(&at) {
                return Err(Error::Security("the master DC has no hash for a CDN file part"));
            }
        }
        // The master DC knows hashes only for bytes that exist, so a hash right after a short part means the CDN cut
        // it. A cut at a hash boundary passes the checks above, so the master is asked when that hash isn't known yet.
        let end = offset + bytes.len() as i64;
        if bytes.len() < limit as usize {
            if !self.hashes.read().unwrap().contains_key(&end) {
                let request = functions::upload::GetCdnFileHashes { file_token: self.token.clone(), offset: end };
                self.add(invoke_part(self.master, &request).await?);
            }
            if self.hashes.read().unwrap().contains_key(&end) {
                return Err(Error::Unexpected("upload.getCdnFile returned a short part before the end"));
            }
        }
        Ok(bytes)
    }

    fn add(&self, hashes: Vec<base::FileHash>) {
        let mut known = self.hashes.write().unwrap();
        for base::FileHash::FileHash(h) in hashes {
            let types::FileHash { offset, limit, hash } = *h;
            if let (Ok(limit @ 1..), Ok(hash)) = (usize::try_from(limit), hash.try_into()) {
                known.insert(offset, (limit, hash));
            }
        }
    }
}

fn decrypt(bytes: &mut [u8], key: &[u8; 32], iv: &[u8; 16], offset: i64) {
    let mut iv = *iv;
    iv[12..].copy_from_slice(&((offset / 16) as u32).to_be_bytes());
    aes::ctr256(bytes, key, &mut iv, &mut 0);
}

fn verified(hashes: &HashMap<i64, (usize, [u8; 32])>, offset: i64, bytes: &[u8]) -> Result<usize> {
    let mut done = 0;
    while done < bytes.len() {
        let Some(&(limit, hash)) = hashes.get(&(offset + done as i64)) else { break };
        let end = bytes.len().min(done + limit);
        if sha256(&[&bytes[done..end]]) != hash {
            return Err(Error::Security("a CDN file part doesn't match its hash"));
        }
        done = end;
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{Cdn, decrypt, verified};
    use crate::connection::Options;
    use crate::crypto::{aes, sha256};
    use crate::errors::Error;
    use crate::raw::functions::upload;
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::session::Session;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    fn file_hash(offset: i64, part: &[u8]) -> base::FileHash {
        let hash = sha256(&[part]).to_vec();
        base::FileHash::FileHash(Box::new(types::FileHash { offset, limit: part.len() as i32, hash }))
    }

    #[test]
    fn decrypts_each_part_at_its_own_counter() {
        // NIST SP 800-38A F.5.5 (CTR-AES256), whose initial counter ends in fcfdfeff: the offset of that block.
        const KEY: &str = "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";
        let key: [u8; 32] = hex(KEY).try_into().unwrap();
        let iv: [u8; 16] = hex("f0f1f2f3f4f5f6f7f8f9fafb00000000").try_into().unwrap();
        let offset = 0xfcfdfeff * 16;
        let mut data = hex("601ec313775789a5b7a7f504bbf3d228f443e3ca4d62b59aca84e990cacaf5c5");
        decrypt(&mut data, &key, &iv, offset);
        assert_eq!(data, hex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51"));

        let mut whole = [7u8; 64];
        let mut halves = whole;
        decrypt(&mut whole, &key, &iv, 4096);
        let (a, b) = halves.split_at_mut(32);
        decrypt(a, &key, &iv, 4096);
        decrypt(b, &key, &iv, 4096 + 32);
        assert_eq!(whole, halves);
    }

    #[test]
    fn checks_every_part_against_its_hash() {
        let data: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
        let mut hashes = HashMap::new();
        for offset in [0, 128, 256] {
            let part = &data[offset..data.len().min(offset + 128)];
            hashes.insert(offset as i64, (128, sha256(&[part])));
        }
        assert_eq!(verified(&hashes, 0, &data).unwrap(), 300);
        assert_eq!(verified(&hashes, 128, &data[128..]).unwrap(), 172);
        assert_eq!(verified(&HashMap::new(), 0, &data).unwrap(), 0);

        hashes.remove(&256);
        assert_eq!(verified(&hashes, 0, &data).unwrap(), 256);

        let mut tampered = data.clone();
        tampered[200] ^= 1;
        assert!(matches!(verified(&hashes, 0, &tampered), Err(Error::Security(_))));
    }

    #[tokio::test]
    async fn reuploads_and_fetches_missing_hashes_from_the_master_dc() {
        let (key, iv) = ([1u8; 32], [2u8; 16]);
        let plain: Vec<u8> = (0..200u32).map(|i| (i * 7) as u8).collect();
        let (mut cipher, mut counter) = (plain.clone(), iv);
        counter[12..].fill(0);
        aes::ctr256(&mut cipher, &key, &mut counter, &mut 0);

        let (head, tail) = plain.split_at(128);
        let (first, rest) = (vec![file_hash(0, head)], vec![file_hash(128, tail)]);
        // The last ask is for a hash after the part's end: none comes back, so the part ends the file.
        let none = Vec::<base::FileHash>::new();
        let replies = vec![Ok(first.to_bytes()), Ok(rest.to_bytes()), Ok(none.to_bytes())];
        let (master, mut asked) = Session::scripted(replies);
        let needed = types::upload::CdnFileReuploadNeeded { request_token: b"again".to_vec() };
        let needed = base::upload::CdnFile::CdnFileReuploadNeeded(Box::new(needed));
        let file = base::upload::CdnFile::CdnFile(Box::new(types::upload::CdnFile { bytes: cipher }));
        let (cdn, _) = Session::scripted(vec![Ok(needed.to_bytes()), Ok(file.to_bytes())]);

        let redirect = types::upload::FileCdnRedirect {
            dc_id: 203,
            file_token: b"token".to_vec(),
            encryption_key: key.to_vec(),
            encryption_iv: iv.to_vec(),
            file_hashes: Vec::new(),
        };
        let options = Options::default();
        let part = Cdn::new(&master, redirect, (0, &options)).unwrap().part(&cdn, 0, 4096).await.unwrap();
        assert_eq!(part, plain);
        let hashes = upload::GetCdnFileHashes::CONSTRUCTOR_ID;
        let calls = [upload::ReuploadCdnFile::CONSTRUCTOR_ID, hashes, hashes];
        let asked: Vec<_> = std::iter::from_fn(|| asked.try_recv().ok()).map(|b| b[..4].to_vec()).collect();
        assert_eq!(asked, calls.map(|c| c.to_le_bytes().to_vec()));
    }

    fn redirect(file_hashes: Vec<base::FileHash>) -> types::upload::FileCdnRedirect {
        types::upload::FileCdnRedirect {
            dc_id: 203,
            file_token: b"token".to_vec(),
            encryption_key: [1u8; 32].to_vec(),
            encryption_iv: [2u8; 16].to_vec(),
            file_hashes,
        }
    }

    fn encrypted(plain: &[u8]) -> Vec<u8> {
        let (mut cipher, mut counter) = (plain.to_vec(), [2u8; 16]);
        counter[12..].fill(0);
        aes::ctr256(&mut cipher, &[1u8; 32], &mut counter, &mut 0);
        base::upload::CdnFile::CdnFile(Box::new(types::upload::CdnFile { bytes: cipher })).to_bytes()
    }

    #[tokio::test]
    async fn a_part_must_fit_its_limit_and_not_stop_before_the_end() {
        let plain: Vec<u8> = (0..256u32).map(|i| (i * 7) as u8).collect();
        let hashes = vec![file_hash(0, &plain[..128]), file_hash(128, &plain[128..]), file_hash(256, b"more")];
        let (master, _) = Session::scripted(Vec::new());
        let options = Options::default();
        let cdn = Cdn::new(&master, redirect(hashes), (0, &options)).unwrap();
        let (session, _) = Session::scripted(vec![Ok(encrypted(&plain)), Ok(encrypted(&plain))]);
        assert!(matches!(cdn.part(&session, 0, 128).await, Err(Error::Unexpected(_))));
        assert!(matches!(cdn.part(&session, 0, 4096).await, Err(Error::Unexpected(_))));
    }

    #[tokio::test]
    async fn a_part_cut_at_a_hash_boundary_is_checked_with_the_master() {
        let plain: Vec<u8> = (0..256u32).map(|i| (i * 7) as u8).collect();
        let hashes = || vec![file_hash(0, &plain[..128]), file_hash(128, &plain[128..])];
        let options = Options::default();
        let part = |more: Vec<base::FileHash>| {
            let (master, _) = Session::scripted(vec![Ok(more.to_bytes())]);
            let (session, _) = Session::scripted(vec![Ok(encrypted(&plain))]);
            let hashes = hashes();
            let options = &options;
            async move { Cdn::new(&master, redirect(hashes), (0, options)).unwrap().part(&session, 0, 4096).await }
        };
        assert!(matches!(part(vec![file_hash(256, b"more")]).await, Err(Error::Unexpected(_))));
        assert_eq!(part(Vec::new()).await.unwrap(), plain);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cdn_that_keeps_flooding_is_given_up_on() {
        let flood = || Err(Error::Rpc(crate::errors::RpcError::new(420, "FLOOD_WAIT_3600")));
        let (master, _) = Session::scripted(Vec::new());
        let options = Options::default();
        let cdn = Cdn::new(&master, redirect(Vec::new()), (0, &options)).unwrap();
        let (session, mut asked) = Session::scripted((0..6).map(|_| flood()).collect());
        let part = cdn.part(&session, 0, 4096).await;
        assert!(matches!(&part, Err(Error::Rpc(e)) if e.code == 420), "{part:?}");
        assert_eq!(std::iter::from_fn(|| asked.try_recv().ok()).count(), 6);
    }
}
