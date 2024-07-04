use pollster::FutureExt;
use zbus::export::futures_util::StreamExt;
use zbus::message::Type;
use zbus::{MatchRule, MessageStream};

/// This function exits after unlock
pub fn wait_until_unlock() {
    let connection = zbus::connection::Connection::session().block_on().unwrap();
    let match_rule = MatchRule::builder()
        .member("ActiveChanged")
        .unwrap()
        .interface("org.freedesktop.ScreenSaver")
        .unwrap()
        .path("/ScreenSaver")
        .unwrap()
        .msg_type(Type::Signal)
        .build();
    let mut stream = Box::pin(
        MessageStream::for_match_rule(match_rule, &connection, None)
            .block_on()
            .unwrap()
            .map(|message| message.unwrap().body().deserialize::<bool>().unwrap())
            .filter(|&message| async move { !message }),
    );
    stream.next().block_on();
}
