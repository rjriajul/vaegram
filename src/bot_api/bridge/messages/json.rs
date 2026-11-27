//! Parts of TL requests as the Bot API's JSON: texts with entities, points on the map, checklists, rich messages.

use serde_json::{Map, Value};

use crate::bot_api::bridge::unsupported;
use crate::bot_api::convert;
use crate::errors::{Error, Result};
use crate::raw::{base, types};

/// Sets `key` when `on`: the Bot API reads a missing flag as false.
pub(super) fn flag(out: &mut Map<String, Value>, key: &str, on: bool) {
    if on {
        out.insert(key.into(), true.into());
    }
}

/// A text, and its entities when it has any.
pub(super) fn text(text: &base::TextWithEntities, me: i64) -> (Value, Option<Value>) {
    let base::TextWithEntities::TextWithEntities(text) = text;
    let entities = (!text.entities.is_empty()).then(|| convert::entities_json(&text.entities, me));
    (text.text.clone().into(), entities)
}

/// Puts `text` under `key` and its entities, when it has any, under `entities_key`.
pub(super) fn put_text(
    out: &mut Map<String, Value>,
    (key, entities_key): (&str, &str),
    text: &str,
    entities: Option<&[base::MessageEntity]>,
    me: i64,
) {
    out.insert(key.into(), text.into());
    if let Some(list) = entities.filter(|list| !list.is_empty()) {
        out.insert(entities_key.into(), convert::entities_json(list, me));
    }
}

/// A point on the map: `latitude`, `longitude` and `horizontal_accuracy`.
pub(super) fn place(out: &mut Map<String, Value>, point: &base::InputGeoPoint) -> Result<()> {
    let base::InputGeoPoint::InputGeoPoint(point) = point else {
        return Err(Error::InvalidArgument("the location names no point"));
    };
    out.insert("latitude".into(), point.lat.into());
    out.insert("longitude".into(), point.long.into());
    if let Some(radius) = point.accuracy_radius {
        out.insert("horizontal_accuracy".into(), radius.into());
    }
    Ok(())
}

/// A live location's period, heading and alert radius, as `sendLocation` and `editMessageLiveLocation` take them.
pub(super) fn live(out: &mut Map<String, Value>, live: &types::InputMediaGeoLive) {
    if let Some(period) = live.period {
        out.insert("live_period".into(), period.into());
    }
    if let Some(heading) = live.heading {
        out.insert("heading".into(), heading.into());
    }
    if let Some(radius) = live.proximity_notification_radius {
        out.insert("proximity_alert_radius".into(), radius.into());
    }
}

/// A venue's point, title, address and place id. Telegram keeps one place id, Foursquare's or Google's.
pub(super) fn venue(out: &mut Map<String, Value>, venue: &types::InputMediaVenue) -> Result<()> {
    place(out, &venue.geo_point)?;
    out.insert("title".into(), venue.title.clone().into());
    out.insert("address".into(), venue.address.clone().into());
    let (id, kind) = match venue.provider.as_str() {
        "foursquare" => ("foursquare_id", "foursquare_type"),
        "gplaces" => ("google_place_id", "google_place_type"),
        _ => return Ok(()),
    };
    if !venue.venue_id.is_empty() {
        out.insert(id.into(), venue.venue_id.clone().into());
    }
    if !venue.venue_type.is_empty() {
        out.insert(kind.into(), venue.venue_type.clone().into());
    }
    Ok(())
}

/// A checklist as the Bot API's `InputChecklist`.
pub(super) fn checklist(todo: &base::TodoList, me: i64) -> Value {
    let base::TodoList::TodoList(todo) = todo;
    let tasks: Vec<Value> = todo
        .list
        .iter()
        .map(|base::TodoItem::TodoItem(item)| {
            let mut task = Map::new();
            task.insert("id".into(), item.id.into());
            let (text, entities) = text(&item.title, me);
            task.insert("text".into(), text);
            if let Some(entities) = entities {
                task.insert("text_entities".into(), entities);
            }
            Value::Object(task)
        })
        .collect();
    let mut out = Map::new();
    let (title, entities) = text(&todo.title, me);
    out.insert("title".into(), title);
    if let Some(entities) = entities {
        out.insert("title_entities".into(), entities);
    }
    out.insert("tasks".into(), Value::Array(tasks));
    flag(&mut out, "others_can_add_tasks", todo.others_can_append);
    flag(&mut out, "others_can_mark_tasks_as_done", todo.others_can_complete);
    Value::Object(out)
}

/// A rich message written in HTML or Markdown as the Bot API's `InputRichMessage`. Blocks and embedded files have
/// no Bot API form vaegram can build yet.
pub(super) fn rich_message(rich: &base::InputRichMessage) -> Result<Value> {
    let (key, source, rtl, noautolink, files) = match rich {
        base::InputRichMessage::InputRichMessageHTML(m) => ("html", &m.html, m.rtl, m.noautolink, m.files.as_ref()),
        base::InputRichMessage::InputRichMessageMarkdown(m) => {
            ("markdown", &m.markdown, m.rtl, m.noautolink, m.files.as_ref())
        }
        base::InputRichMessage::InputRichMessage(_) => return Err(unsupported("rich messages built from blocks")),
    };
    if files.is_some_and(|files| !files.is_empty()) {
        return Err(unsupported("files in rich messages"));
    }
    let mut out = Map::new();
    out.insert(key.into(), source.clone().into());
    flag(&mut out, "is_rtl", rtl);
    flag(&mut out, "skip_entity_detection", noautolink);
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn point(lat: f64, long: f64) -> base::InputGeoPoint {
        base::InputGeoPoint::InputGeoPoint(Box::new(types::InputGeoPoint { lat, long, accuracy_radius: Some(5) }))
    }

    #[test]
    fn a_venue_keeps_its_one_place_id() {
        let mut venue = types::InputMediaVenue {
            geo_point: point(1.5, 2.5),
            title: "Cafe".into(),
            address: "Main St".into(),
            provider: "gplaces".into(),
            venue_id: "g1".into(),
            venue_type: String::new(),
        };
        let mut out = Map::new();
        super::venue(&mut out, &venue).unwrap();
        let expected = json!({
            "latitude": 1.5, "longitude": 2.5, "horizontal_accuracy": 5, "title": "Cafe", "address": "Main St",
            "google_place_id": "g1",
        });
        assert_eq!(Value::Object(out), expected);

        venue.provider = "foursquare".into();
        venue.venue_type = "food".into();
        let mut out = Map::new();
        super::venue(&mut out, &venue).unwrap();
        assert_eq!((&out["foursquare_id"], &out["foursquare_type"]), (&json!("g1"), &json!("food")));
        assert!(out.get("google_place_id").is_none());

        let mut out = Map::new();
        assert!(place(&mut out, &base::InputGeoPoint::InputGeoPointEmpty).is_err());
    }

    #[test]
    fn only_html_and_markdown_rich_messages_have_a_bot_api_form() {
        let html = types::InputRichMessageHTML { rtl: true, noautolink: true, html: "<b>hi</b>".into(), files: None };
        let rich = base::InputRichMessage::InputRichMessageHTML(Box::new(html));
        let expected = json!({ "html": "<b>hi</b>", "is_rtl": true, "skip_entity_detection": true });
        assert_eq!(rich_message(&rich).unwrap(), expected);

        let markdown = types::InputRichMessageMarkdown {
            rtl: false,
            noautolink: false,
            markdown: "**hi**".into(),
            files: Some(Vec::new()),
        };
        let rich = base::InputRichMessage::InputRichMessageMarkdown(Box::new(markdown));
        assert_eq!(rich_message(&rich).unwrap(), json!({ "markdown": "**hi**" }));
    }
}
