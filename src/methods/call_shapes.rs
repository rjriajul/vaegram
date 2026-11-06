//! Widened parameters still take the argument shapes they took before, and the futures stay `Send`.

use crate::Client;
use crate::enums::StoriesPrivacyRules;
use crate::errors::Result;
use crate::types::{InputStoryContent, Story, Upload};

fn send<T: Send>(value: T) -> T {
    value
}

// Only type-checked, never run: each `old` call compiled against the earlier `&str` / `&[&str]` signatures.
async fn shapes(client: &Client, story: &Story) -> Result<()> {
    let owned = String::from("c");
    let borrowed: &str = &owned;
    let ids: Vec<&str> = vec!["9", "PlushPepe-3"];
    let slice: &[&str] = &ids;

    // Old shapes.
    send(client.delete_business_messages(borrowed, &[1])).await?;
    send(client.delete_business_story(&owned, 1)).await?;
    send(client.get_business_account_star_balance("c")).await?;
    send(client.get_business_connection(borrowed)).await?;
    send(client.read_business_message(borrowed, 1, 1)).await?;
    send(client.remove_business_account_profile_photo(borrowed, true)).await?;
    send(client.set_business_account_bio(borrowed, "bio")).await?;
    send(client.set_business_account_name(borrowed, "First", "")).await?;
    send(client.set_business_account_username(borrowed, "name")).await?;
    send(client.transfer_business_account_stars(borrowed, 5)).await?;
    client.set_business_account_profile_photo(borrowed, Upload::bytes("a.jpg", b"x")).await?;
    send(client.hide_gift(borrowed)).await?;
    send(client.show_gift(&owned)).await?;
    send(client.drop_gift_original_details("9", 1)).await?;
    send(client.set_gift_resale_price(borrowed, None)).await?;
    send(client.craft_gift(&["9", "10"])).await?;
    send(client.add_collection_gifts("me", 1, slice)).await?;
    send(client.remove_collection_gifts("me", 1, &ids)).await?;
    send(client.reorder_collection_gifts("me", 1, &ids[..])).await?;
    send(client.create_gift_collection("me", "Best", &["9"])).await?;
    send(client.set_pinned_gifts("me", slice)).await?;
    story.edit_media(InputStoryContent::photo(Upload::bytes("a.jpg", b"x"))).await?;
    story.edit_privacy(StoriesPrivacyRules::CloseFriends).await?;

    // New shapes.
    send(client.delete_business_messages(owned.clone(), &[1])).await?;
    send(client.hide_gift(owned.clone())).await?;
    send(client.craft_gift(vec![owned.clone()])).await?;
    send(client.set_pinned_gifts("me", ids.iter().map(|id| id.to_uppercase()))).await?;
    send(client.set_pinned_gifts("me", [""; 0])).await?;
    story.edit_media(InputStoryContent::photo(Upload::bytes("a.jpg", b"x"))).caption("new").await?;
    Ok(())
}

#[test]
fn old_and_new_argument_shapes_type_check() {
    let _ = shapes;
}
