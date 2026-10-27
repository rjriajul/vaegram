use vaegram::Error;
use vaegram::connection::transport::intermediate;

#[tokio::test]
async fn packets_round_trip_and_error_codes_surface() {
    let (mut client, mut server) = tokio::io::duplex(1024);

    intermediate::send(&mut client, b"abcd1234").await.unwrap();
    assert_eq!(intermediate::recv(&mut server).await.unwrap(), b"abcd1234");

    intermediate::send(&mut server, &(-404i32).to_le_bytes()).await.unwrap();
    assert!(matches!(intermediate::recv(&mut client).await, Err(Error::Transport(404))));
}
