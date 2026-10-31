use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::PeerRef;

/// An invoice sent in a message, or one behind a slug, a `t.me/$<slug>` or a `t.me/invoice/<slug>` link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvoiceRef {
    Message { chat_id: PeerRef, message_id: i32 },
    Link(String),
}

impl InvoiceRef {
    pub(crate) async fn write(self, client: &Client) -> Result<base::InputInvoice> {
        Ok(match self {
            InvoiceRef::Message { chat_id, message_id } => {
                let peer = client.resolve_peer(chat_id).await?;
                base::InputInvoice::InputInvoiceMessage(Box::new(types::InputInvoiceMessage {
                    peer,
                    msg_id: message_id,
                }))
            }
            InvoiceRef::Link(link) => base::InputInvoice::InputInvoiceSlug(Box::new(types::InputInvoiceSlug {
                slug: invoice_slug(&link)?.to_owned(),
            })),
        })
    }
}

fn invoice_slug(link: &str) -> Result<&str> {
    let link = link.trim().trim_end_matches('/');
    let path = ["https://", "http://"].iter().fold(link, |l, scheme| l.strip_prefix(scheme).unwrap_or(l));
    let slug = match path.strip_prefix("t.me/").or_else(|| path.strip_prefix("telegram.me/")) {
        Some(rest) => rest.strip_prefix('$').or_else(|| rest.strip_prefix("invoice/")).unwrap_or_default(),
        None => path.strip_prefix('$').unwrap_or(path),
    };
    if slug.is_empty() || slug.contains('/') {
        return Err(Error::InvalidArgument("the invoice link has no slug"));
    }
    Ok(slug)
}

#[cfg(test)]
mod tests {
    use super::invoice_slug;
    use crate::errors::Error;

    #[test]
    fn takes_the_slug_from_links() {
        for link in ["abc", " https://t.me/$abc ", "t.me/invoice/abc/", "$abc", "http://telegram.me/invoice/abc"] {
            assert_eq!(invoice_slug(link).unwrap(), "abc", "{link}");
        }
    }

    #[test]
    fn a_link_without_a_slug_is_refused() {
        for link in ["", "  ", "t.me/$", "t.me/invoice/", "https://t.me/invoice", "t.me/a/b"] {
            assert!(matches!(invoice_slug(link), Err(Error::InvalidArgument(_))), "{link}");
        }
    }
}
